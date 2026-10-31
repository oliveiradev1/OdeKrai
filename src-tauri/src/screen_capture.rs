use base64::{engine::general_purpose::STANDARD, Engine as _};
use image::{imageops::FilterType, DynamicImage, ImageFormat, Limits};
use std::{
    collections::HashMap,
    io::Cursor,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};

pub(crate) const MAX_SCREENSHOT_BYTES: usize = 512 * 1024;
const MAX_SOURCE_IMAGE_BYTES: usize = 64 * 1024 * 1024;
const MAX_SCREENSHOT_PIXELS: u64 = 40_000_000;
const MAX_DIMENSION: u32 = 1280;
const MAX_APPROVAL_THUMBNAIL_BYTES: usize = 16 * 1024;

static PREPARED_AGENT_CAPTURES: OnceLock<Mutex<HashMap<String, Vec<u8>>>> = OnceLock::new();
static CAPTURE_ID_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

pub(crate) fn new_agent_capture_id() -> String {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!(
        "cap-{timestamp}-{}",
        CAPTURE_ID_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    )
}

pub(crate) struct PreparedCaptureCleanup(String);

impl PreparedCaptureCleanup {
    pub(crate) fn for_id(id: String) -> Self {
        Self(id)
    }
}

impl Drop for PreparedCaptureCleanup {
    fn drop(&mut self) {
        discard_agent_capture(&self.0);
    }
}

pub(crate) fn prepare_agent_capture(capture_id: &str) -> Result<String, String> {
    let source = capture_virtual_screen()?;
    let (jpeg, _, _) = normalize_image(&source)?;
    let thumbnail = approval_thumbnail(&jpeg)?;
    let cache = PREPARED_AGENT_CAPTURES.get_or_init(|| Mutex::new(HashMap::new()));
    cache
        .lock()
        .map_err(|_| "Não foi possível preparar a prévia da captura.".to_string())?
        .insert(capture_id.to_owned(), jpeg);
    Ok(format!(
        "data:image/jpeg;base64,{}",
        STANDARD.encode(thumbnail)
    ))
}

fn approval_thumbnail(jpeg: &[u8]) -> Result<Vec<u8>, String> {
    let image = image::load_from_memory(jpeg)
        .map_err(|_| "Não foi possível preparar a prévia reduzida da captura.")?;
    let mut thumbnail = image.resize(320, 180, FilterType::Lanczos3);
    for quality in [48, 38, 28, 18] {
        let bytes = encode_jpeg(&thumbnail, quality)?;
        if bytes.len() <= MAX_APPROVAL_THUMBNAIL_BYTES {
            return Ok(bytes);
        }
        thumbnail = thumbnail.resize(240, 135, FilterType::Lanczos3);
    }
    Err("A prévia reduzida excede o limite permitido.".into())
}

pub(crate) fn save_prepared_agent_capture(
    capture_id: &str,
    destination: &Path,
) -> Result<PathBuf, String> {
    let cache = PREPARED_AGENT_CAPTURES.get_or_init(|| Mutex::new(HashMap::new()));
    let bytes = cache
        .lock()
        .map_err(|_| "Não foi possível ler a captura aprovada.".to_string())?
        .remove(capture_id)
        .ok_or("A captura temporária expirou; tire um novo print.")?;
    use std::fs::OpenOptions;
    use std::io::Write;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .map_err(|_| "Não foi possível criar o arquivo de captura na pasta de trabalho.")?;
    if file
        .write_all(&bytes)
        .and_then(|()| file.sync_all())
        .is_err()
    {
        drop(file);
        let _ = std::fs::remove_file(destination);
        return Err("Não foi possível salvar a captura aprovada.".into());
    }
    Ok(destination.to_path_buf())
}

pub(crate) fn discard_agent_capture(capture_id: &str) {
    if let Some(cache) = PREPARED_AGENT_CAPTURES.get() {
        if let Ok(mut cache) = cache.lock() {
            cache.remove(capture_id);
        }
    }
}

pub(crate) fn normalize_image(bytes: &[u8]) -> Result<(Vec<u8>, u32, u32), String> {
    if bytes.is_empty() || bytes.len() > MAX_SOURCE_IMAGE_BYTES {
        return Err("A captura excede o limite de entrada.".into());
    }
    let format =
        image::guess_format(bytes).map_err(|_| "A captura não é uma imagem compatível.")?;
    if !matches!(
        format,
        ImageFormat::Bmp | ImageFormat::Png | ImageFormat::Jpeg | ImageFormat::WebP
    ) {
        return Err("A captura precisa estar em BMP, PNG, JPEG ou WebP.".into());
    }
    let mut limits = Limits::default();
    limits.max_image_width = Some(12_000);
    limits.max_image_height = Some(12_000);
    limits.max_alloc = Some(192 * 1024 * 1024);
    let mut decoder = image::ImageReader::with_format(Cursor::new(bytes), format);
    decoder.limits(limits);
    let image = decoder
        .decode()
        .map_err(|_| "Não foi possível processar a captura.")?;
    if u64::from(image.width()) * u64::from(image.height()) > MAX_SCREENSHOT_PIXELS {
        return Err("A captura tem resolução acima do limite permitido.".into());
    }

    let mut scaled = image.resize(MAX_DIMENSION, MAX_DIMENSION, FilterType::Lanczos3);
    for quality in [78, 68, 58, 48] {
        let bytes = encode_jpeg(&scaled, quality)?;
        if bytes.len() <= MAX_SCREENSHOT_BYTES {
            return Ok((bytes, scaled.width(), scaled.height()));
        }
        let width = (scaled.width() as f32 * 0.8).round().max(1.0) as u32;
        let height = (scaled.height() as f32 * 0.8).round().max(1.0) as u32;
        scaled = scaled.resize_exact(width, height, FilterType::Lanczos3);
    }
    let bytes = encode_jpeg(&scaled, 38)?;
    if bytes.len() > MAX_SCREENSHOT_BYTES {
        return Err("Não foi possível reduzir a captura ao limite de 512 KiB.".into());
    }
    Ok((bytes, scaled.width(), scaled.height()))
}

fn encode_jpeg(image: &DynamicImage, quality: u8) -> Result<Vec<u8>, String> {
    let mut output = Cursor::new(Vec::new());
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut output, quality)
        .encode_image(image)
        .map_err(|_| "Não foi possível preparar a captura.")?;
    Ok(output.into_inner())
}

#[cfg(windows)]
fn capture_virtual_screen() -> Result<Vec<u8>, String> {
    use std::{ffi::c_void, mem::size_of, ptr::null_mut};
    use windows_sys::Win32::{
        Graphics::Gdi::{
            BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC,
            GetDIBits, ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB,
            DIB_RGB_COLORS, SRCCOPY,
        },
        UI::WindowsAndMessaging::{
            GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN,
            SM_YVIRTUALSCREEN,
        },
    };

    let left = unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) };
    let top = unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) };
    let width = unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) };
    let height = unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) };
    if width <= 0
        || height <= 0
        || width > 12_000
        || height > 12_000
        || i64::from(width) * i64::from(height) > MAX_SCREENSHOT_PIXELS as i64
    {
        return Err("A resolução da tela não está dentro do limite permitido.".into());
    }

    unsafe {
        let screen = GetDC(null_mut());
        if screen.is_null() {
            return Err("Não foi possível acessar a tela do Windows.".into());
        }
        let memory = CreateCompatibleDC(screen);
        let bitmap = CreateCompatibleBitmap(screen, width, height);
        if memory.is_null() || bitmap.is_null() {
            if !bitmap.is_null() {
                DeleteObject(bitmap);
            }
            if !memory.is_null() {
                DeleteDC(memory);
            }
            ReleaseDC(null_mut(), screen);
            return Err("Não foi possível preparar a captura da tela.".into());
        }
        let previous = SelectObject(memory, bitmap);
        if previous.is_null()
            || BitBlt(memory, 0, 0, width, height, screen, left, top, SRCCOPY) == 0
        {
            if !previous.is_null() {
                SelectObject(memory, previous);
            }
            DeleteObject(bitmap);
            DeleteDC(memory);
            ReleaseDC(null_mut(), screen);
            return Err("O Windows não conseguiu capturar a tela.".into());
        }
        SelectObject(memory, previous);
        let mut info: BITMAPINFO = std::mem::zeroed();
        info.bmiHeader = BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            biHeight: -height,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB,
            biSizeImage: 0,
            biXPelsPerMeter: 0,
            biYPelsPerMeter: 0,
            biClrUsed: 0,
            biClrImportant: 0,
        };
        let byte_count = (width as usize)
            .checked_mul(height as usize)
            .and_then(|pixels| pixels.checked_mul(4))
            .ok_or("A captura excede o limite permitido.")?;
        let mut pixels = vec![0u8; byte_count];
        let copied = GetDIBits(
            memory,
            bitmap,
            0,
            height as u32,
            pixels.as_mut_ptr().cast::<c_void>(),
            &mut info,
            DIB_RGB_COLORS,
        );
        DeleteObject(bitmap);
        DeleteDC(memory);
        ReleaseDC(null_mut(), screen);
        if copied != height {
            return Err("Não foi possível ler os pixels da captura.".into());
        }
        // GDI returns BGRA; encode a simple BMP container for the common image normalizer.
        let mut bmp = Vec::with_capacity(54 + pixels.len());
        bmp.extend_from_slice(b"BM");
        bmp.extend_from_slice(&(54u32 + pixels.len() as u32).to_le_bytes());
        bmp.extend_from_slice(&[0; 4]);
        bmp.extend_from_slice(&54u32.to_le_bytes());
        bmp.extend_from_slice(&40u32.to_le_bytes());
        bmp.extend_from_slice(&width.to_le_bytes());
        bmp.extend_from_slice(&(-height).to_le_bytes());
        bmp.extend_from_slice(&1u16.to_le_bytes());
        bmp.extend_from_slice(&32u16.to_le_bytes());
        bmp.extend_from_slice(&0u32.to_le_bytes());
        bmp.extend_from_slice(&(pixels.len() as u32).to_le_bytes());
        bmp.extend_from_slice(&[0; 16]);
        for pixel in pixels.as_chunks_mut::<4>().0 {
            pixel[3] = 255;
        }
        bmp.extend_from_slice(&pixels);
        Ok(bmp)
    }
}

#[cfg(not(windows))]
fn capture_virtual_screen() -> Result<Vec<u8>, String> {
    Err("A captura de tela está disponível somente no Windows.".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgb};

    fn test_image(width: u32, height: u32) -> Vec<u8> {
        let image =
            DynamicImage::ImageRgb8(ImageBuffer::from_pixel(width, height, Rgb([40, 80, 120])));
        let mut bytes = Cursor::new(Vec::new());
        image.write_to(&mut bytes, ImageFormat::Png).unwrap();
        bytes.into_inner()
    }

    #[test]
    fn normalizes_image_and_strips_source_format_metadata() {
        let (jpeg, width, height) = normalize_image(&test_image(1600, 900)).unwrap();
        assert_eq!((width, height), (1280, 720));
        assert!(jpeg.len() <= MAX_SCREENSHOT_BYTES);
        assert_eq!(image::guess_format(&jpeg).unwrap(), ImageFormat::Jpeg);
    }

    #[test]
    fn rejects_invalid_and_excessively_large_image_input() {
        assert!(normalize_image(b"not an image").is_err());
        assert!(normalize_image(&vec![0; MAX_SOURCE_IMAGE_BYTES + 1]).is_err());
    }

    #[test]
    fn approved_capture_is_saved_only_to_the_requested_workspace_target() {
        let id = new_agent_capture_id();
        let root = std::env::temp_dir().join(format!("odekrai-screen-approved-{id}"));
        std::fs::create_dir_all(&root).unwrap();
        let destination = root.join("captura.jpg");
        let jpeg = normalize_image(&test_image(16, 9)).unwrap().0;
        PREPARED_AGENT_CAPTURES
            .get_or_init(|| Mutex::new(HashMap::new()))
            .lock()
            .unwrap()
            .insert(id.clone(), jpeg.clone());

        let saved = save_prepared_agent_capture(&id, &destination).unwrap();

        assert_eq!(saved, destination);
        assert_eq!(std::fs::read(&saved).unwrap(), jpeg);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejected_capture_is_discarded_without_creating_a_file() {
        let id = new_agent_capture_id();
        let root = std::env::temp_dir().join(format!("odekrai-screen-rejected-{id}"));
        std::fs::create_dir_all(&root).unwrap();
        let destination = root.join("captura.jpg");
        PREPARED_AGENT_CAPTURES
            .get_or_init(|| Mutex::new(HashMap::new()))
            .lock()
            .unwrap()
            .insert(id.clone(), b"prepared bytes".to_vec());

        drop(PreparedCaptureCleanup::for_id(id.clone()));

        assert!(!destination.exists());
        assert!(!PREPARED_AGENT_CAPTURES
            .get()
            .unwrap()
            .lock()
            .unwrap()
            .contains_key(&id));
        std::fs::remove_dir_all(root).unwrap();
    }
}
