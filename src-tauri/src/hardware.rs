use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, serde::Deserialize)]
pub struct HardwareProfile {
    pub total_ram_bytes: u64,
    pub available_ram_bytes: u64,
    pub models_free_bytes: u64,
    pub cpu_cores: usize,
    pub avx: bool,
    pub avx2: bool,
    pub avx512: bool,
    pub adapters: Vec<VideoAdapter>,
    #[serde(default)]
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, serde::Deserialize)]
pub struct VideoAdapter {
    pub name: String,
    pub dedicated_vram_bytes: u64,
}

#[tauri::command]
pub async fn get_hardware_profile(caller: tauri::WebviewWindow) -> Result<HardwareProfile, String> {
    if caller.label() != "main" {
        return Err("Comando indisponível para esta janela.".into());
    }
    tauri::async_runtime::spawn_blocking(collect_hardware_profile)
        .await
        .map_err(|_| "Não foi possível consultar o computador.".to_string())?
}

fn collect_hardware_profile() -> Result<HardwareProfile, String> {
    #[cfg(windows)]
    {
        windows_profile()
    }
    #[cfg(not(windows))]
    {
        Err("A consulta de hardware está disponível no Windows.".into())
    }
}

#[cfg(windows)]
fn windows_profile() -> Result<HardwareProfile, String> {
    use std::{ffi::OsStr, os::windows::ffi::OsStrExt};
    use windows::Win32::{
        Graphics::Dxgi::{CreateDXGIFactory1, IDXGIFactory1},
        System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX},
    };
    use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
    let mut memory = MEMORYSTATUSEX {
        dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
        ..Default::default()
    };
    let mut warnings = Vec::new();
    // SAFETY: `memory` is initialized with its required size and remains writable for the API call.
    if unsafe { GlobalMemoryStatusEx(&mut memory) }.is_err() {
        warnings.push("Não foi possível consultar a memória RAM.".to_string());
    }
    let models_path = dirs_path().join("models");
    let volume_path = if models_path.exists() {
        models_path.clone()
    } else {
        models_path.parent().unwrap_or(&models_path).to_path_buf()
    };
    let wide: Vec<u16> = OsStr::new(&volume_path)
        .encode_wide()
        .chain(Some(0))
        .collect();
    let mut free = 0u64;
    // SAFETY: `wide` is NUL-terminated and `free` is a live writable output; unused outputs are null.
    let disk_ok = unsafe {
        GetDiskFreeSpaceExW(
            wide.as_ptr(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut free,
        )
    };
    if disk_ok == 0 {
        warnings.push("Não foi possível consultar o espaço livre para modelos.".to_string());
    }
    // SAFETY: CreateDXGIFactory1 has no caller pointers; the returned COM interface owns its lifetime.
    let factory: Option<IDXGIFactory1> = unsafe { CreateDXGIFactory1().ok() };
    let mut adapters = Vec::new();
    if let Some(factory) = factory {
        let mut index = 0;
        // SAFETY: `factory` is a live COM interface; each returned adapter owns its COM reference.
        while let Ok(adapter) = unsafe { factory.EnumAdapters1(index) } {
            // SAFETY: `adapter` is a live COM interface and GetDesc1 writes into its own output.
            if let Ok(desc) = unsafe { adapter.GetDesc1() } {
                let name = String::from_utf16_lossy(&desc.Description)
                    .trim_end_matches('\0')
                    .to_string();
                if desc.Flags & 2 == 0 {
                    adapters.push(VideoAdapter {
                        name,
                        dedicated_vram_bytes: desc.DedicatedVideoMemory as u64,
                    });
                }
            }
            index += 1;
        }
    } else {
        warnings.push("Não foi possível consultar os adaptadores de vídeo.".to_string());
    }
    Ok(HardwareProfile {
        total_ram_bytes: memory.ullTotalPhys,
        available_ram_bytes: memory.ullAvailPhys,
        models_free_bytes: free,
        cpu_cores: std::thread::available_parallelism()
            .map(usize::from)
            .unwrap_or_default(),
        avx: std::is_x86_feature_detected!("avx"),
        avx2: std::is_x86_feature_detected!("avx2"),
        avx512: std::is_x86_feature_detected!("avx512f"),
        adapters,
        warnings,
    })
}

#[cfg(windows)]
fn dirs_path() -> std::path::PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("OdeKrai")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn accepts_simulated_hardware_payload_used_by_the_command() {
        let simulated = serde_json::json!({
            "total_ram_bytes": 16_000_000_000u64,
            "available_ram_bytes": 12_000_000_000u64,
            "models_free_bytes": 80_000_000_000u64,
            "cpu_cores": 8,
            "avx": true,
            "avx2": true,
            "avx512": false,
            "adapters": [{"name": "GPU de teste", "dedicated_vram_bytes": 4_000_000_000u64}]
        });
        let decoded: HardwareProfile = serde_json::from_value(simulated).unwrap();
        assert_eq!(decoded.adapters[0].name, "GPU de teste");
        assert_eq!(decoded.cpu_cores, 8);
        assert_eq!(decoded.available_ram_bytes, 12_000_000_000);
    }
}
