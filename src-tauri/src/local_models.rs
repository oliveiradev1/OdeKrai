use reqwest::{header, redirect::Policy, Client, StatusCode, Url};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::Mutex,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tauri::{Emitter, Manager};
use tokio_util::sync::CancellationToken;

const VERIFICATION_PROGRESS_INTERVAL_BYTES: u64 = 8 * 1024 * 1024;

#[derive(Deserialize)]
struct Manifest {
    models: Vec<Model>,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Model {
    id: String,
    name: String,
    repository: String,
    file: String,
    size_bytes: Option<u64>,
    sha256: Option<String>,
    license: Option<String>,
    download_hosts: Vec<String>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct DownloadProgress {
    model_id: String,
    received_bytes: u64,
    total_bytes: u64,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LocalModelInventory {
    model_id: String,
    name: String,
    file_name: String,
    size_bytes: u64,
    status: &'static str,
}

#[derive(Default)]
pub struct Downloads {
    active: Mutex<HashMap<String, CancellationToken>>,
    verifications: Mutex<HashMap<String, CancellationToken>>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct VerificationProgress {
    model_id: String,
    processed_bytes: u64,
    total_bytes: u64,
    status: &'static str,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LocalModelVerificationResult {
    pub model_id: String,
    pub verified: bool,
    pub size_bytes: u64,
}

/// Reads the chosen folder directly. The filesystem is the persisted inventory source;
/// this command never treats a selected UI model as a downloaded or verified file.
#[tauri::command]
pub async fn inspect_local_models(
    caller: tauri::WebviewWindow,
    state: tauri::State<'_, Downloads>,
    destination: String,
) -> Result<Vec<LocalModelInventory>, String> {
    crate::require_main(&caller)?;
    let active = state
        .active
        .lock()
        .map_err(|_| "Não foi possível consultar os downloads ativos.".to_string())?
        .clone();
    tauri::async_runtime::spawn_blocking(move || {
        let Some(folder) = resolve_inventory_folder(&PathBuf::from(destination))? else {
            return Ok(Vec::new());
        };
        let manifest: Manifest =
            serde_json::from_str(include_str!("../resources/local-models.json"))
                .map_err(|_| "O catálogo de modelos está inválido.".to_string())?;
        inventory_models(&folder, manifest.models, &active)
    })
    .await
    .map_err(|_| "Não foi possível concluir a consulta dos modelos locais.".to_string())?
}

fn resolve_inventory_folder(path: &Path) -> Result<Option<PathBuf>, String> {
    resolve_inventory_folder_result(path.canonicalize())
}

fn resolve_inventory_folder_result(
    result: std::io::Result<PathBuf>,
) -> Result<Option<PathBuf>, String> {
    match result {
        Ok(folder) => Ok(Some(folder)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
            Err(inventory_folder_error(error.kind()).into())
        }
        Err(_) => Ok(None),
    }
}

fn inventory_folder_error(kind: std::io::ErrorKind) -> &'static str {
    if kind == std::io::ErrorKind::PermissionDenied {
        "Sem permissão para acessar a pasta de modelos."
    } else {
        "A pasta de modelos não pode ser acessada."
    }
}

fn inventory_models(
    folder: &Path,
    models: Vec<Model>,
    active: &HashMap<String, CancellationToken>,
) -> Result<Vec<LocalModelInventory>, String> {
    let mut inventory = Vec::with_capacity(models.len());
    for model in models {
        if Path::new(&model.file)
            .file_name()
            .and_then(|name| name.to_str())
            != Some(model.file.as_str())
            || model.file.contains("..")
        {
            return Err("O catálogo contém um nome de arquivo inválido.".into());
        }
        let path = folder.join(&model.file);
        let metadata = fs::symlink_metadata(&path);
        let (size_bytes, status) = match metadata {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let partial = folder.join(format!("{}.part", model.file));
                match fs::symlink_metadata(partial) {
                    Err(partial_error) if partial_error.kind() == std::io::ErrorKind::NotFound => {
                        (0, "NAO_BAIXADO")
                    }
                    Err(partial_error)
                        if partial_error.kind() == std::io::ErrorKind::PermissionDenied =>
                    {
                        return Err(inventory_folder_error(partial_error.kind()).into());
                    }
                    Ok(meta) if !meta.file_type().is_file() => (meta.len(), "ERRO"),
                    Ok(meta) if meta.len() > model.size_bytes.unwrap_or_default() => {
                        (meta.len(), "ERRO")
                    }
                    Ok(meta) if active.contains_key(&model.id) => (meta.len(), "BAIXANDO"),
                    Ok(meta) => (meta.len(), "PARCIAL"),
                    Err(_) => (0, "ERRO"),
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
                return Err(inventory_folder_error(error.kind()).into());
            }
            Err(_) => (0, "ERRO"),
            Ok(metadata) if !metadata.file_type().is_file() => (metadata.len(), "ERRO"),
            Ok(metadata) => {
                let expected_size = model.size_bytes.unwrap_or_default();
                if metadata.len() != expected_size {
                    (metadata.len(), "ERRO")
                } else {
                    (metadata.len(), "BAIXADO")
                }
            }
        };
        inventory.push(LocalModelInventory {
            model_id: model.id,
            name: model.name,
            file_name: model.file,
            size_bytes,
            status,
        });
    }
    Ok(inventory)
}

fn hash_file_with_progress(
    path: &Path,
    token: &CancellationToken,
    mut progress: impl FnMut(u64, u64),
) -> Result<String, String> {
    if token.is_cancelled() {
        return Err("Verificação cancelada pelo usuário.".into());
    }
    let mut file =
        fs::File::open(path).map_err(|_| "Não foi possível ler o arquivo.".to_string())?;
    let total = file
        .metadata()
        .map_err(|_| "Não foi possível consultar o tamanho do arquivo.".to_string())?
        .len();
    let mut hash = Sha256::new();
    let mut buffer = [0; 1024 * 1024];
    let mut processed = 0;
    let mut reported = 0;
    loop {
        if token.is_cancelled() {
            return Err("Verificação cancelada pelo usuário.".into());
        }
        let count = std::io::Read::read(&mut file, &mut buffer)
            .map_err(|_| "Não foi possível verificar o arquivo.".to_string())?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
        processed += count as u64;
        if processed.saturating_sub(reported) >= VERIFICATION_PROGRESS_INTERVAL_BYTES
            || processed == total
        {
            progress(processed, total);
            reported = processed;
        }
    }
    Ok(format!("{:x}", hash.finalize()))
}

fn initialize_verification_cache(connection: &Connection) -> Result<(), String> {
    connection
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS local_model_verifications (
                model_id TEXT PRIMARY KEY NOT NULL,
                actual_sha256 TEXT NOT NULL,
                expected_sha256 TEXT NOT NULL,
                file_size_bytes INTEGER NOT NULL,
                modified_at INTEGER NOT NULL,
                verified_at INTEGER NOT NULL,
                verified INTEGER NOT NULL CHECK(verified IN (0, 1))
            );",
        )
        .map_err(|_| "Não foi possível preparar o cache de verificação dos modelos.".to_string())
}

struct VerificationCacheEntry<'a> {
    model_id: &'a str,
    actual_sha256: &'a str,
    expected_sha256: &'a str,
    file_size_bytes: u64,
    modified_at: i64,
    verified_at: i64,
    verified: bool,
}

fn store_verification_cache(
    connection: &Connection,
    entry: VerificationCacheEntry<'_>,
) -> Result<(), String> {
    connection
        .execute(
            "INSERT INTO local_model_verifications
                (model_id, actual_sha256, expected_sha256, file_size_bytes, modified_at, verified_at, verified)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(model_id) DO UPDATE SET
                actual_sha256=excluded.actual_sha256,
                expected_sha256=excluded.expected_sha256,
                file_size_bytes=excluded.file_size_bytes,
                modified_at=excluded.modified_at,
                verified_at=excluded.verified_at,
                verified=excluded.verified",
            params![
                entry.model_id,
                entry.actual_sha256,
                entry.expected_sha256,
                i64::try_from(entry.file_size_bytes).unwrap_or(i64::MAX),
                entry.modified_at,
                entry.verified_at,
                if entry.verified { 1_i64 } else { 0_i64 },
            ],
        )
        .map_err(|_| "Não foi possível salvar o resultado da verificação.".to_string())?;
    Ok(())
}

fn unix_timestamp(time: SystemTime) -> i64 {
    time.duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .min(i64::MAX as u64) as i64
}

fn verify_model_file(
    folder: &Path,
    app_data: &Path,
    model: &Model,
    token: &CancellationToken,
    app: &tauri::AppHandle,
) -> Result<LocalModelVerificationResult, String> {
    let folder = folder
        .canonicalize()
        .map_err(|_| "A pasta de modelos não existe ou não pode ser acessada.".to_string())?;
    let candidate = folder.join(&model.file);
    let metadata = fs::symlink_metadata(&candidate)
        .map_err(|_| "O arquivo do modelo não foi encontrado.".to_string())?;
    if !metadata.file_type().is_file() {
        return Err("O arquivo do modelo não é um arquivo comum.".into());
    }
    let path = candidate
        .canonicalize()
        .map_err(|_| "O arquivo do modelo não pode ser acessado.".to_string())?;
    if path.parent() != Some(folder.as_path()) {
        return Err("O arquivo do modelo está fora da pasta escolhida.".into());
    }
    let metadata = fs::metadata(&path)
        .map_err(|_| "Não foi possível consultar o arquivo do modelo.".to_string())?;
    let modified_at = metadata.modified().map(unix_timestamp).unwrap_or_default();
    let total_bytes = metadata.len();
    let _ = app.emit(
        "local-model-verification-progress",
        VerificationProgress {
            model_id: model.id.clone(),
            processed_bytes: 0,
            total_bytes,
            status: "verificando",
        },
    );
    let expected_sha256 = model
        .sha256
        .as_deref()
        .ok_or("O hash deste modelo não está definido no catálogo.")?;
    let actual_sha256 = hash_file_with_progress(&path, token, |processed_bytes, total_bytes| {
        let _ = app.emit(
            "local-model-verification-progress",
            VerificationProgress {
                model_id: model.id.clone(),
                processed_bytes,
                total_bytes,
                status: "verificando",
            },
        );
    });
    let actual_sha256 = match actual_sha256 {
        Ok(hash) => hash,
        Err(error) => {
            let status = if token.is_cancelled() {
                "cancelado"
            } else {
                "erro"
            };
            let _ = app.emit(
                "local-model-verification-progress",
                VerificationProgress {
                    model_id: model.id.clone(),
                    processed_bytes: 0,
                    total_bytes,
                    status,
                },
            );
            return Err(error);
        }
    };
    let verified = total_bytes == model.size_bytes.unwrap_or_default()
        && actual_sha256.eq_ignore_ascii_case(expected_sha256)
        && has_gguf_magic(&path);
    let cache_result = open_verification_cache(app_data).and_then(|connection| {
        store_verification_cache(
            &connection,
            VerificationCacheEntry {
                model_id: &model.id,
                actual_sha256: &actual_sha256,
                expected_sha256,
                file_size_bytes: total_bytes,
                modified_at,
                verified_at: unix_timestamp(SystemTime::now()),
                verified,
            },
        )
    });
    if let Err(error) = cache_result {
        let _ = app.emit(
            "local-model-verification-progress",
            VerificationProgress {
                model_id: model.id.clone(),
                processed_bytes: total_bytes,
                total_bytes,
                status: "erro",
            },
        );
        return Err(error);
    }
    let _ = app.emit(
        "local-model-verification-progress",
        VerificationProgress {
            model_id: model.id.clone(),
            processed_bytes: total_bytes,
            total_bytes,
            status: if verified { "concluido" } else { "erro" },
        },
    );
    Ok(LocalModelVerificationResult {
        model_id: model.id.clone(),
        verified,
        size_bytes: total_bytes,
    })
}

fn open_verification_cache(app_data: &Path) -> Result<Connection, String> {
    fs::create_dir_all(app_data)
        .map_err(|_| "Não foi possível preparar o cache de modelos locais.".to_string())?;
    let connection = Connection::open(app_data.join("local-model-verifications.sqlite3"))
        .map_err(|_| "Não foi possível abrir o cache de modelos locais.".to_string())?;
    initialize_verification_cache(&connection)?;
    Ok(connection)
}

fn has_gguf_magic(path: &Path) -> bool {
    let Ok(mut file) = fs::File::open(path) else {
        return false;
    };
    let mut magic = [0; 4];
    std::io::Read::read_exact(&mut file, &mut magic).is_ok() && &magic == b"GGUF"
}

#[tauri::command]
pub async fn verify_local_model(
    caller: tauri::WebviewWindow,
    app: tauri::AppHandle,
    state: tauri::State<'_, Downloads>,
    model_id: String,
    destination: String,
) -> Result<LocalModelVerificationResult, String> {
    crate::require_main(&caller)?;
    let model = manifest_model(&model_id)?;
    if model.sha256.is_none() || model.size_bytes.is_none() {
        return Err("Este modelo não tem hash e tamanho definidos no catálogo.".into());
    }
    let token = CancellationToken::new();
    {
        let mut verifications = state
            .verifications
            .lock()
            .map_err(|_| "Não foi possível iniciar a verificação do modelo.".to_string())?;
        if verifications.contains_key(&model_id) {
            return Err("Este modelo já está sendo verificado.".into());
        }
        verifications.insert(model_id.clone(), token.clone());
    }
    let app_data = app
        .path()
        .app_data_dir()
        .map_err(|_| "Não foi possível localizar o cache de modelos locais.".to_string());
    let result = match app_data {
        Ok(app_data) => match tauri::async_runtime::spawn_blocking(move || {
            verify_model_file(Path::new(&destination), &app_data, &model, &token, &app)
        })
        .await
        {
            Ok(result) => result,
            Err(_) => Err("Não foi possível concluir a verificação do modelo.".to_string()),
        },
        Err(error) => Err(error),
    };
    if let Ok(mut verifications) = state.verifications.lock() {
        verifications.remove(&model_id);
    }
    result
}

#[tauri::command]
pub fn cancel_local_model_verification(
    caller: tauri::WebviewWindow,
    state: tauri::State<'_, Downloads>,
    model_id: String,
) -> Result<bool, String> {
    crate::require_main(&caller)?;
    let token = state
        .verifications
        .lock()
        .map_err(|_| "Não foi possível cancelar a verificação.".to_string())?
        .get(&model_id)
        .cloned();
    if let Some(token) = token {
        token.cancel();
        Ok(true)
    } else {
        Ok(false)
    }
}

#[tauri::command]
pub async fn download_local_model(
    caller: tauri::WebviewWindow,
    app: tauri::AppHandle,
    state: tauri::State<'_, Downloads>,
    model_id: String,
    destination: String,
) -> Result<(), String> {
    crate::require_main(&caller)?;
    let model = manifest_model(&model_id)?;
    if model.size_bytes.is_none() || model.sha256.is_none() || model.license.is_none() {
        return Err("Este arquivo ainda não tem tamanho, hash e licença verificados.".into());
    }
    let folder = PathBuf::from(destination);
    fs::create_dir_all(&folder)
        .map_err(|_| "Não foi possível preparar a pasta escolhida.".to_string())?;
    let folder = folder
        .canonicalize()
        .map_err(|_| "A pasta escolhida não é válida.".to_string())?;
    let final_path = folder.join(&model.file);
    let partial_path = folder.join(format!("{}.part", model.file));
    let prompt = format!(
        "Baixar o modelo {}?\nTamanho: {} bytes\nLicença: {}\nDestino: {}\n\nO arquivo será obtido por HTTPS e validado por SHA-256.",
        model.id,
        model.size_bytes.unwrap_or_default(),
        model.license.as_deref().unwrap_or("não verificada"),
        folder.display()
    );
    if !crate::security::confirm_native(prompt).await {
        return Err("Download cancelado pelo usuário.".into());
    }
    let token = CancellationToken::new();
    {
        let mut active = state
            .active
            .lock()
            .map_err(|_| "Não foi possível iniciar o download.".to_string())?;
        if active.contains_key(&model_id) {
            return Err("Este modelo já está sendo baixado.".into());
        }
        active.insert(model_id.clone(), token.clone());
    }
    let result = download_file(&app, &model, &final_path, &partial_path, token).await;
    if let Ok(mut active) = state.active.lock() {
        active.remove(&model_id);
    }
    if result.is_ok() {
        crate::diagnostics::log_local_model_download_completed(&app);
    }
    result
}

#[tauri::command]
pub fn cancel_local_model_download(
    caller: tauri::WebviewWindow,
    state: tauri::State<'_, Downloads>,
    model_id: String,
) -> Result<bool, String> {
    crate::require_main(&caller)?;
    let token = state
        .active
        .lock()
        .map_err(|_| "Não foi possível cancelar o download.".to_string())?
        .get(&model_id)
        .cloned();
    if let Some(token) = token {
        token.cancel();
        Ok(true)
    } else {
        Ok(false)
    }
}

#[tauri::command]
pub async fn delete_local_model(
    caller: tauri::WebviewWindow,
    model_id: String,
    destination: String,
) -> Result<(), String> {
    crate::require_main(&caller)?;
    let model = manifest_model(&model_id)?;
    let folder = PathBuf::from(destination)
        .canonicalize()
        .map_err(|_| "A pasta de modelos não existe.".to_string())?;
    let path = folder.join(&model.file);
    if path.parent() != Some(folder.as_path()) {
        return Err("O destino do modelo é inválido.".into());
    }
    if !crate::security::confirm_native(format!(
        "Apagar o modelo {} de {}?",
        model.id,
        folder.display()
    ))
    .await
    {
        return Err("Exclusão cancelada pelo usuário.".into());
    }
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err("Não foi possível apagar o arquivo do modelo.".into()),
    }
}

async fn download_file(
    app: &tauri::AppHandle,
    model: &Model,
    destination: &Path,
    partial: &Path,
    token: CancellationToken,
) -> Result<(), String> {
    let size = model
        .size_bytes
        .ok_or("O tamanho do arquivo não foi verificado.")?;
    let expected_hash = model
        .sha256
        .as_deref()
        .ok_or("O hash do arquivo não foi verificado.")?;
    if destination.exists() {
        return Err("Este arquivo já existe no destino.".into());
    }
    let existing = fs::metadata(partial).map(|meta| meta.len()).unwrap_or(0);
    if existing > size {
        fs::remove_file(partial)
            .map_err(|_| "Não foi possível reiniciar o arquivo parcial.".to_string())?;
    }
    let existing = fs::metadata(partial).map(|meta| meta.len()).unwrap_or(0);
    let mut hash = Sha256::new();
    if existing > 0 {
        let mut prior = fs::File::open(partial)
            .map_err(|_| "Não foi possível retomar o arquivo parcial.".to_string())?;
        let mut buffer = [0; 1024 * 1024];
        loop {
            let count = std::io::Read::read(&mut prior, &mut buffer)
                .map_err(|_| "Não foi possível retomar o arquivo parcial.".to_string())?;
            if count == 0 {
                break;
            }
            hash.update(&buffer[..count]);
        }
    }
    let mut url = Url::parse(&format!(
        "https://huggingface.co/{}/resolve/main/{}?download=true",
        model.repository, model.file
    ))
    .map_err(|_| "O endereço do modelo é inválido.".to_string())?;
    let client = Client::builder()
        .redirect(Policy::none())
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(300))
        .build()
        .map_err(|_| "Não foi possível preparar o download.".to_string())?;
    let mut request = client.get(url.clone());
    if existing > 0 {
        request = request.header(header::RANGE, format!("bytes={existing}-"));
    }
    let mut response = request.send().await.map_err(|_| {
        "O download falhou; o arquivo parcial foi mantido para retomada.".to_string()
    })?;
    for _ in 0..5 {
        if response.status().is_redirection() {
            let location = response
                .headers()
                .get(header::LOCATION)
                .and_then(|v| v.to_str().ok())
                .ok_or("O servidor enviou um redirecionamento inválido.")?;
            url = url
                .join(location)
                .map_err(|_| "O endereço redirecionado é inválido.".to_string())?;
            if url.scheme() != "https"
                || !model
                    .download_hosts
                    .iter()
                    .any(|host| host == url.host_str().unwrap_or_default())
            {
                return Err("O servidor redirecionou para um host fora da lista permitida.".into());
            }
            let mut next = client.get(url.clone());
            if existing > 0 {
                next = next.header(header::RANGE, format!("bytes={existing}-"));
            }
            response = next.send().await.map_err(|_| {
                "O download falhou; o arquivo parcial foi mantido para retomada.".to_string()
            })?;
        } else {
            break;
        }
    }
    if response.status().is_redirection() {
        return Err("O servidor excedeu o limite de redirecionamentos.".into());
    }
    if existing > 0 && response.status() == StatusCode::OK {
        hash = Sha256::new();
        fs::write(partial, [])
            .map_err(|_| "Não foi possível reiniciar o arquivo parcial.".to_string())?;
    } else if existing > 0 && response.status() != StatusCode::PARTIAL_CONTENT {
        return Err("O servidor não aceitou a retomada do download.".into());
    } else if existing == 0 && !response.status().is_success() {
        return Err("O servidor não disponibilizou o modelo.".into());
    }
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(partial)
        .map_err(|_| "Não foi possível gravar o arquivo do modelo.".to_string())?;
    let mut received = if existing > 0 && response.status() == StatusCode::PARTIAL_CONTENT {
        existing
    } else {
        0
    };
    while let Some(chunk) = response.chunk().await.map_err(|_| {
        "O download foi interrompido; o arquivo parcial foi mantido para retomada.".to_string()
    })? {
        if token.is_cancelled() {
            return Err("Download cancelado; o arquivo parcial foi mantido para retomada.".into());
        }
        received += chunk.len() as u64;
        if received > size {
            return Err("O arquivo recebido excede o tamanho informado no manifesto.".into());
        }
        hash.update(&chunk);
        file.write_all(&chunk)
            .map_err(|_| "Não foi possível gravar o arquivo do modelo.".to_string())?;
        let _ = app.emit(
            "local-model-download-progress",
            DownloadProgress {
                model_id: model.id.clone(),
                received_bytes: received,
                total_bytes: size,
            },
        );
    }
    file.sync_all()
        .map_err(|_| "Não foi possível finalizar a gravação do modelo.".to_string())?;
    if received != size {
        return Err("O download terminou antes do tamanho esperado; tente retomar.".into());
    }
    let actual_hash = format!("{:x}", hash.finalize());
    if actual_hash != expected_hash {
        return Err("A verificação SHA-256 falhou; o arquivo parcial foi mantido.".into());
    }
    if !has_gguf_magic(partial) {
        return Err("O arquivo recebido não tem cabeçalho GGUF; o parcial foi mantido.".into());
    }
    fs::rename(partial, destination)
        .map_err(|_| "Não foi possível finalizar o arquivo do modelo.".to_string())?;
    Ok(())
}

fn manifest_model(id: &str) -> Result<Model, String> {
    let manifest: Manifest = serde_json::from_str(include_str!("../resources/local-models.json"))
        .map_err(|_| "O catálogo de modelos está inválido.".to_string())?;
    let model = manifest
        .models
        .into_iter()
        .find(|model| model.id == id)
        .ok_or("Modelo não encontrado no catálogo.")?;
    if Path::new(&model.file)
        .file_name()
        .and_then(|name| name.to_str())
        != Some(model.file.as_str())
        || model.file.contains("..")
        || model.download_hosts.is_empty()
        || model.download_hosts.iter().any(|host| {
            host.is_empty()
                || host.contains('/')
                || host.contains(':')
                || !host.chars().all(|character| {
                    character.is_ascii_alphanumeric() || character == '.' || character == '-'
                })
        })
    {
        return Err("O catálogo contém um destino ou host inválido.".into());
    }
    Ok(model)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_inventory_folder_is_an_empty_state() {
        let folder = std::env::temp_dir().join(format!(
            "odekrai-model-folder-missing-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        assert!(matches!(resolve_inventory_folder(&folder), Ok(None)));
    }

    #[test]
    fn permission_denied_for_inventory_folder_has_a_clear_message() {
        assert_eq!(
            resolve_inventory_folder_result(Err(std::io::Error::from(
                std::io::ErrorKind::PermissionDenied,
            ))),
            Err("Sem permissão para acessar a pasta de modelos.".into())
        );
    }

    #[test]
    fn manifest_download_hosts_are_https_allowlisted() {
        let model = manifest_model("qwen35-2b-q4km").unwrap();
        assert!(model.download_hosts.contains(&"huggingface.co".into()));
        assert!(model
            .download_hosts
            .iter()
            .all(|host| !host.contains('/') && !host.contains(':')));
    }
    #[test]
    fn manifest_file_names_cannot_escape_destination() {
        let model = manifest_model("qwen35-2b-q4km").unwrap();
        assert_eq!(Path::new(&model.file).components().count(), 1);
        assert!(!model.file.contains(".."));
    }

    #[test]
    fn inventory_reads_exact_files_and_detects_removed_or_similar_names() {
        let folder = std::env::temp_dir().join(format!(
            "odekrai-model-inventory-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&folder).unwrap();
        let first = Model {
            id: "first".into(),
            name: "Similar model first".into(),
            repository: "test/model".into(),
            file: "same-q4.gguf".into(),
            size_bytes: Some(4),
            sha256: Some(format!("{:x}", Sha256::digest(b"GGUF"))),
            license: Some("test".into()),
            download_hosts: vec!["example.test".into()],
        };
        let second = Model {
            id: "second".into(),
            name: "Similar model second".into(),
            file: "same-q4-other.gguf".into(),
            ..first.clone()
        };
        let first_file = first.file.clone();
        fs::write(folder.join(&first.file), b"GGUF").unwrap();
        assert_eq!(
            inventory_models(
                &folder,
                vec![first.clone(), second.clone()],
                &HashMap::new()
            )
            .unwrap()
            .iter()
            .map(|item| item.status)
            .collect::<Vec<_>>(),
            ["BAIXADO", "NAO_BAIXADO"]
        );
        fs::write(folder.join(&first.file), b"NOPE").unwrap();
        assert!(!has_gguf_magic(&folder.join(&first.file)));
        assert_eq!(
            inventory_models(
                &folder,
                vec![first.clone(), second.clone()],
                &HashMap::new()
            )
            .unwrap()[0]
                .status,
            "BAIXADO"
        );
        fs::remove_file(folder.join(first_file.clone())).unwrap();
        let partial = folder.join(format!("{first_file}.part"));
        fs::write(&partial, b"GGUF").unwrap();
        let partial_status = inventory_models(
            &folder,
            vec![first.clone(), second.clone()],
            &HashMap::new(),
        )
        .unwrap();
        assert_eq!(partial_status[0].status, "PARCIAL");
        let active = HashMap::from([(first.id.clone(), CancellationToken::new())]);
        let active_status =
            inventory_models(&folder, vec![first.clone(), second.clone()], &active).unwrap();
        assert_eq!(active_status[0].status, "BAIXANDO");
        fs::remove_file(partial).unwrap();
        assert_eq!(
            inventory_models(&folder, vec![first, second], &HashMap::new())
                .unwrap()
                .iter()
                .map(|item| item.status)
                .collect::<Vec<_>>(),
            ["NAO_BAIXADO", "NAO_BAIXADO"]
        );
        fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn inventory_does_not_read_large_model_contents() {
        let folder = std::env::temp_dir().join(format!(
            "odekrai-model-inventory-large-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&folder).unwrap();
        let size_bytes = 512 * 1024 * 1024;
        let model = Model {
            id: "large".into(),
            name: "Large model".into(),
            repository: "test/model".into(),
            file: "large.gguf".into(),
            size_bytes: Some(size_bytes),
            sha256: Some("0".repeat(64)),
            license: Some("test".into()),
            download_hosts: vec!["example.test".into()],
        };
        let path = folder.join(&model.file);
        let mut file = fs::File::create(&path).unwrap();
        file.write_all(b"NOPE").unwrap();
        file.set_len(size_bytes).unwrap();
        drop(file);

        let inventory = inventory_models(&folder, vec![model], &HashMap::new()).unwrap();
        assert_eq!(inventory[0].status, "BAIXADO");
        assert_eq!(inventory[0].size_bytes, size_bytes);
        assert_eq!(inventory[0].file_name, "large.gguf");
        let serialized = serde_json::to_value(&inventory[0]).unwrap();
        assert!(serialized.get("path").is_none());
        assert!(serialized.get("sha256").is_none());
        assert!(!has_gguf_magic(&path));

        fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn verification_hash_can_be_cancelled_between_chunks() {
        let path = std::env::temp_dir().join(format!(
            "odekrai-model-hash-cancel-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::write(&path, vec![b'x'; 9 * 1024 * 1024]).unwrap();
        let token = CancellationToken::new();
        let cancel = token.clone();
        let mut progress_events = 0;
        let result = hash_file_with_progress(&path, &token, |processed, _| {
            progress_events += 1;
            if processed > 0 {
                cancel.cancel();
            }
        });
        assert_eq!(result.unwrap_err(), "Verificação cancelada pelo usuário.");
        assert!(progress_events > 0);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn verification_hash_reports_progress_and_matches_known_sha256() {
        let path = std::env::temp_dir().join(format!(
            "odekrai-model-hash-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::write(&path, b"GGUF").unwrap();
        let mut progress = Vec::new();
        let hash = hash_file_with_progress(&path, &CancellationToken::new(), |processed, total| {
            progress.push((processed, total))
        })
        .unwrap();
        assert_eq!(hash, format!("{:x}", Sha256::digest(b"GGUF")));
        assert_eq!(progress, [(4, 4)]);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn verification_result_is_saved_in_sqlite_cache() {
        let connection = Connection::open_in_memory().unwrap();
        initialize_verification_cache(&connection).unwrap();
        store_verification_cache(
            &connection,
            VerificationCacheEntry {
                model_id: "model-a",
                actual_sha256: "actual-hash",
                expected_sha256: "expected-hash",
                file_size_bytes: 512,
                modified_at: 10,
                verified_at: 20,
                verified: false,
            },
        )
        .unwrap();
        let cache: (String, String, i64, i64) = connection
            .query_row(
                "SELECT actual_sha256, expected_sha256, file_size_bytes, verified
                 FROM local_model_verifications WHERE model_id = ?1",
                ["model-a"],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(
            cache,
            ("actual-hash".into(), "expected-hash".into(), 512, 0)
        );
    }
}
