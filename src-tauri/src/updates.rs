use reqwest::{header, redirect::Policy, Client, StatusCode};
use rusqlite::{params, Connection, OptionalExtension};
use semver::Version;
use serde::{Deserialize, Serialize};
use std::{path::Path, time::Duration};
use tauri::{AppHandle, Manager, WebviewWindow};
use url::Url;

const OWNER: &str = "oliveiradev1";
const REPOSITORY: &str = "OdeKrai";
const CHECK_INTERVAL_SECONDS: i64 = 24 * 60 * 60;
const SUMMARY_MAX_CHARS: usize = 600;
const API_URL: &str = "https://api.github.com/repos/oliveiradev1/OdeKrai/releases/latest";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UpdateCheckResult {
    pub current_version: String,
    pub available_version: Option<String>,
    pub release_url: Option<String>,
    pub notes_summary: Option<String>,
    pub check_succeeded: bool,
}

impl UpdateCheckResult {
    fn current_only(version: &str, check_succeeded: bool) -> Self {
        Self {
            current_version: version.to_owned(),
            available_version: None,
            release_url: None,
            notes_summary: None,
            check_succeeded,
        }
    }
}

#[derive(Debug, Deserialize)]
struct ReleaseResponse {
    tag_name: String,
    html_url: String,
    body: Option<String>,
}

type CachedUpdateState = (Option<String>, Option<String>, Option<String>, bool);

fn open_database(path: &Path) -> Result<Connection, String> {
    let connection = Connection::open(path)
        .map_err(|_| "Não foi possível acessar o histórico de atualizações.".to_string())?;
    initialize_database(&connection)?;
    Ok(connection)
}

fn initialize_database(connection: &Connection) -> Result<(), String> {
    connection
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS update_check_state (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                last_attempt_at INTEGER NOT NULL DEFAULT 0,
                available_version TEXT,
                release_url TEXT,
                notes_summary TEXT,
                last_check_succeeded INTEGER NOT NULL DEFAULT 1
            );
            INSERT OR IGNORE INTO update_check_state (id, last_attempt_at) VALUES (1, 0);",
        )
        .map_err(|_| "Não foi possível preparar o histórico de atualizações.".to_string())?;
    let has_status_column: bool = connection
        .query_row(
            "SELECT EXISTS (
                SELECT 1 FROM pragma_table_info('update_check_state')
                WHERE name = 'last_check_succeeded'
            )",
            [],
            |row| row.get(0),
        )
        .map_err(|_| "Não foi possível verificar o histórico de atualizações.".to_string())?;
    if !has_status_column {
        connection
            .execute(
                "ALTER TABLE update_check_state ADD COLUMN last_check_succeeded INTEGER NOT NULL DEFAULT 1",
                [],
            )
            .map_err(|_| {
                "Não foi possível atualizar o histórico de atualizações.".to_string()
            })?;
    }
    Ok(())
}

fn reserve_check(connection: &Connection, now: i64) -> Result<bool, String> {
    connection
        .execute_batch("BEGIN IMMEDIATE")
        .map_err(|_| "Não foi possível iniciar a consulta de atualizações.".to_string())?;
    let last: i64 = connection
        .query_row(
            "SELECT last_attempt_at FROM update_check_state WHERE id = 1",
            [],
            |row| row.get(0),
        )
        .map_err(|_| "Não foi possível ler o histórico de atualizações.".to_string())?;
    if now.saturating_sub(last) < CHECK_INTERVAL_SECONDS {
        connection
            .execute_batch("COMMIT")
            .map_err(|_| "Não foi possível concluir a consulta de atualizações.".to_string())?;
        return Ok(false);
    }
    connection
        .execute(
            "UPDATE update_check_state SET last_attempt_at = ?1 WHERE id = 1",
            [now],
        )
        .map_err(|_| "Não foi possível atualizar o histórico de atualizações.".to_string())?;
    connection
        .execute_batch("COMMIT")
        .map_err(|_| "Não foi possível concluir a consulta de atualizações.".to_string())?;
    Ok(true)
}

fn cached_result(
    connection: &Connection,
    current_version: &str,
) -> Result<UpdateCheckResult, String> {
    let cached: Option<CachedUpdateState> = connection
        .query_row(
            "SELECT available_version, release_url, notes_summary, last_check_succeeded
             FROM update_check_state WHERE id = 1",
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get::<_, i64>(3)? != 0,
                ))
            },
        )
        .optional()
        .map_err(|_| "Não foi possível ler o resultado de atualizações.".to_string())?;
    let Some(cached) = cached else {
        return Ok(UpdateCheckResult::current_only(current_version, false));
    };
    if !cached.3 {
        return Ok(UpdateCheckResult::current_only(current_version, false));
    }
    let (available_version, release_url, notes_summary, check_succeeded) = cached;
    Ok(UpdateCheckResult {
        current_version: current_version.to_owned(),
        available_version,
        release_url,
        notes_summary,
        check_succeeded,
    })
}

fn save_result(connection: &Connection, result: &UpdateCheckResult) -> Result<(), String> {
    connection
        .execute(
            "UPDATE update_check_state SET available_version = ?1, release_url = ?2,
             notes_summary = ?3, last_check_succeeded = ?4 WHERE id = 1",
            params![
                result.available_version,
                result.release_url,
                result.notes_summary,
                if result.check_succeeded { 1_i64 } else { 0_i64 }
            ],
        )
        .map_err(|_| "Não foi possível salvar o resultado de atualizações.".to_string())?;
    Ok(())
}

fn save_failure(connection: &Connection) -> Result<(), String> {
    connection
        .execute(
            "UPDATE update_check_state SET last_check_succeeded = 0 WHERE id = 1",
            [],
        )
        .map_err(|_| "Não foi possível salvar o estado da consulta de atualizações.".to_string())?;
    Ok(())
}

fn make_client(version: &str) -> Result<Client, String> {
    Client::builder()
        .user_agent(format!("OdeKrai/{version}"))
        .redirect(Policy::none())
        .timeout(Duration::from_secs(8))
        .build()
        .map_err(|_| "Não foi possível preparar a consulta de atualizações.".to_string())
}

fn release_url_is_valid(raw: &str, tag: &str) -> bool {
    let Ok(url) = Url::parse(raw) else {
        return false;
    };
    url.scheme() == "https"
        && url.host_str() == Some("github.com")
        && url.username().is_empty()
        && url.password().is_none()
        && url.port().is_none()
        && url.query().is_none()
        && url.fragment().is_none()
        && (url.path() == format!("/{OWNER}/{REPOSITORY}/releases/{tag}")
            || url.path() == format!("/{OWNER}/{REPOSITORY}/releases/tag/{tag}"))
}

fn summarize_notes(body: Option<&str>) -> Option<String> {
    let normalized = body?
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    if normalized.is_empty() {
        return None;
    }
    let summary: String = normalized.chars().take(SUMMARY_MAX_CHARS).collect();
    Some(summary.trim().to_owned())
}

async fn fetch_latest(
    client: &Client,
    endpoint: &str,
    current: &str,
) -> Result<UpdateCheckResult, String> {
    let current_semver = Version::parse(current)
        .map_err(|_| "A versão atual do aplicativo é inválida.".to_string())?;
    let response = client
        .get(endpoint)
        .header(header::ACCEPT, "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2026-03-10")
        .send()
        .await
        .map_err(|_| "Falha de rede ao consultar atualizações.".to_string())?;
    if response.status() != StatusCode::OK {
        return Err(format!(
            "GitHub respondeu com HTTP {}.",
            response.status().as_u16()
        ));
    }
    let release: ReleaseResponse = response
        .json()
        .await
        .map_err(|_| "Resposta inválida ao consultar atualizações.".to_string())?;
    let available_raw = release
        .tag_name
        .strip_prefix('v')
        .unwrap_or(&release.tag_name);
    let available = Version::parse(available_raw)
        .map_err(|_| "A versão publicada não segue o formato semver.".to_string())?;
    if !release_url_is_valid(&release.html_url, &release.tag_name) {
        return Err("A URL da release não pertence à página oficial do GitHub.".to_string());
    }
    if available <= current_semver {
        return Ok(UpdateCheckResult::current_only(current, true));
    }
    Ok(UpdateCheckResult {
        current_version: current.to_owned(),
        available_version: Some(available.to_string()),
        release_url: Some(format!(
            "https://github.com/{OWNER}/{REPOSITORY}/releases/tag/{}",
            release.tag_name
        )),
        notes_summary: summarize_notes(release.body.as_deref()),
        check_succeeded: true,
    })
}

fn database_path(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|_| "Não foi possível localizar os dados locais.".to_string())?;
    std::fs::create_dir_all(&directory)
        .map_err(|_| "Não foi possível preparar os dados locais.".to_string())?;
    Ok(directory.join("updates.sqlite3"))
}

#[tauri::command]
pub async fn check_for_update(
    app: AppHandle,
    caller: WebviewWindow,
) -> Result<UpdateCheckResult, String> {
    crate::require_main(&caller)?;
    let current = env!("CARGO_PKG_VERSION");
    let path = database_path(&app)?;
    let now = chrono::Utc::now().timestamp();
    let should_check = {
        let connection = open_database(&path)?;
        reserve_check(&connection, now)?
    };
    if !should_check {
        return cached_result(&open_database(&path)?, current);
    }
    let result = match make_client(current) {
        Ok(client) => fetch_latest(&client, API_URL, current).await,
        Err(error) => Err(error),
    };
    match result {
        Ok(result) => {
            save_result(&open_database(&path)?, &result)?;
            Ok(result)
        }
        Err(_) => {
            crate::diagnostics::log_event(
                &app,
                "update_check",
                "check_for_update",
                "network_or_response_failure",
                0,
            );
            save_failure(&open_database(&path)?)?;
            cached_result(&open_database(&path)?, current)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };

    async fn mock_response(body: String, current: &str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let expected_user_agent = format!("user-agent: odekrai/{current}");
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = Vec::with_capacity(4096);
            let mut chunk = [0_u8; 1024];
            while request.len() < 4096 && !request.windows(4).any(|part| part == b"\r\n\r\n") {
                let read = stream.read(&mut chunk).await.unwrap();
                if read == 0 {
                    break;
                }
                request.extend_from_slice(&chunk[..read.min(4096 - request.len())]);
            }
            let request = String::from_utf8_lossy(&request).to_ascii_lowercase();
            assert!(request.starts_with("get /latest http/1.1"));
            assert!(
                request.contains(&expected_user_agent),
                "cabeçalhos recebidos pelo mock: {request:?}"
            );
            assert!(request.contains("accept: application/vnd.github+json"));
            assert!(request.contains("x-github-api-version: 2026-03-10"));
            assert!(!request.contains("cookie:"));
            assert!(!request.contains("authorization:"));
            let response = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            stream.write_all(response.as_bytes()).await.unwrap();
        });
        format!("http://{address}/latest")
    }

    async fn fetch_json(tag: &str, url: &str) -> Result<UpdateCheckResult, String> {
        fetch_json_for_current(tag, url, "0.15.0").await
    }

    async fn fetch_json_for_current(
        tag: &str,
        url: &str,
        current: &str,
    ) -> Result<UpdateCheckResult, String> {
        let endpoint = mock_response(
            format!(
                r###"{{"tag_name":"{tag}","html_url":"{url}","body":"## Novidades\n- Correções"}}"###
            ),
            current,
        )
        .await;
        fetch_latest(&make_client(current).unwrap(), &endpoint, current).await
    }

    async fn mock_disconnect() -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            drop(stream);
        });
        format!("http://{address}/disconnect")
    }

    #[tokio::test]
    async fn semver_detects_equal_newer_older_and_prerelease_versions() {
        for tag in ["v0.15.0", "v0.14.9"] {
            let url = format!("https://github.com/oliveiradev1/OdeKrai/releases/tag/{tag}");
            assert_eq!(fetch_json(tag, &url).await.unwrap().available_version, None);
        }
        let prerelease_url = "https://github.com/oliveiradev1/OdeKrai/releases/tag/v0.16.0-beta.1";
        assert_eq!(
            fetch_json("v0.16.0-beta.1", prerelease_url)
                .await
                .unwrap()
                .available_version
                .as_deref(),
            Some("0.16.0-beta.1")
        );
        let older_prerelease_url =
            "https://github.com/oliveiradev1/OdeKrai/releases/tag/v0.16.0-rc.1";
        assert_eq!(
            fetch_json_for_current("v0.16.0-rc.1", older_prerelease_url, "0.16.0-beta.2")
                .await
                .unwrap()
                .available_version
                .as_deref(),
            Some("0.16.0-rc.1")
        );
        assert_eq!(
            fetch_json_for_current("v0.16.0-rc.1", older_prerelease_url, "0.16.0")
                .await
                .unwrap()
                .available_version,
            None
        );
        let good_url = "https://github.com/oliveiradev1/OdeKrai/releases/v0.16.0";
        assert_eq!(
            fetch_json("v0.16.0", good_url)
                .await
                .unwrap()
                .available_version
                .as_deref(),
            Some("0.16.0")
        );
    }

    #[tokio::test]
    async fn recusa_url_de_release_fora_do_repositorio_oficial() {
        assert!(fetch_json("v0.16.0", "https://evil.example/release")
            .await
            .is_err());
    }

    #[tokio::test]
    async fn falha_de_rede_e_limite_de_24_horas_nao_precisam_da_rede_real() {
        let client = make_client("0.15.0").unwrap();
        let disconnected_endpoint = mock_disconnect().await;
        assert!(fetch_latest(&client, &disconnected_endpoint, "0.15.0")
            .await
            .is_err());
        let connection = Connection::open_in_memory().unwrap();
        initialize_database(&connection).unwrap();
        assert!(reserve_check(&connection, 100_000).unwrap());
        assert!(!reserve_check(&connection, 100_000 + CHECK_INTERVAL_SECONDS - 1).unwrap());
        assert!(reserve_check(&connection, 100_000 + CHECK_INTERVAL_SECONDS).unwrap());
        save_failure(&connection).unwrap();
        let result = cached_result(&connection, "0.15.0").unwrap();
        assert!(!result.check_succeeded);
        assert_eq!(result.available_version, None);
    }

    #[test]
    fn url_de_release_exige_https_e_caminho_fixo() {
        assert!(release_url_is_valid(
            "https://github.com/oliveiradev1/OdeKrai/releases/v1.0.0",
            "v1.0.0"
        ));
        assert!(!release_url_is_valid(
            "https://github.com.evil/oliveiradev1/OdeKrai/releases/tag/v1.0.0",
            "v1.0.0"
        ));
        assert!(!release_url_is_valid(
            "http://github.com/oliveiradev1/OdeKrai/releases/tag/v1.0.0",
            "v1.0.0"
        ));
    }

    #[test]
    fn resumo_e_limitado() {
        let summary = summarize_notes(Some(&format!("<b>{}</b>", "x".repeat(700)))).unwrap();
        assert!(summary.chars().count() <= SUMMARY_MAX_CHARS);
        assert!(summary.starts_with("<b>"));
    }

    #[test]
    fn schema_existente_recebe_coluna_de_status_sem_perder_historico() {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch(
                "CREATE TABLE update_check_state (
                    id INTEGER PRIMARY KEY CHECK (id = 1),
                    last_attempt_at INTEGER NOT NULL DEFAULT 0,
                    available_version TEXT,
                    release_url TEXT,
                    notes_summary TEXT
                );
                INSERT INTO update_check_state (id, last_attempt_at) VALUES (1, 12345);",
            )
            .unwrap();
        initialize_database(&connection).unwrap();
        let last_attempt: i64 = connection
            .query_row(
                "SELECT last_attempt_at FROM update_check_state WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(last_attempt, 12345);
        assert!(!reserve_check(&connection, 12346).unwrap());
    }
}
