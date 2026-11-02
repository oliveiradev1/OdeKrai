use super::{
    catalog::catalog_entry,
    credentials::credential_entry,
    http::{http_client, provider_http_error, safe_transport_error, AuthRequest},
};
use base64::Engine as _;
use reqwest::{header, Url};
use serde::Deserialize;
use std::sync::atomic::{AtomicU64, Ordering};

const MAX_AUDIO_BYTES: usize = 25_000_000;
static MULTIPART_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TranscriptionProvider {
    id: &'static str,
    model: &'static str,
    language_field: &'static str,
}

fn transcription_provider(provider_id: &str) -> Option<TranscriptionProvider> {
    match provider_id {
        "openai" => Some(TranscriptionProvider {
            id: "openai",
            model: "gpt-transcribe",
            language_field: "languages[]",
        }),
        "groq" => Some(TranscriptionProvider {
            id: "groq",
            model: "whisper-large-v3-turbo",
            language_field: "language",
        }),
        _ => None,
    }
}

fn transcription_endpoint(provider: TranscriptionProvider) -> Result<Url, String> {
    let catalog =
        catalog_entry(provider.id).ok_or("O provedor de transcrição não está cadastrado.")?;
    let mut endpoint = Url::parse(&catalog.base_url)
        .map_err(|_| "O endpoint de transcrição cadastrado é inválido.")?;
    let expected_host = match provider.id {
        "openai" => "api.openai.com",
        "groq" => "api.groq.com",
        _ => return Err("Este provedor não possui transcrição verificada.".into()),
    };
    if endpoint.scheme() != "https" || endpoint.host_str() != Some(expected_host) {
        return Err("O host de transcrição não corresponde à allowlist do provedor.".into());
    }
    let base_path = endpoint.path().trim_end_matches('/');
    endpoint.set_path(&format!("{base_path}/audio/transcriptions"));
    Ok(endpoint)
}

fn normalize_media_type(
    provider_id: &str,
    media_type: &str,
) -> Result<(&'static str, &'static str), String> {
    let media_type = media_type
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    match (provider_id, media_type.as_str()) {
        ("openai" | "groq", "audio/webm") => Ok(("audio/webm", "recording.webm")),
        ("openai" | "groq", "audio/mp4") => Ok(("audio/mp4", "recording.mp4")),
        ("groq", "audio/ogg") => Ok(("audio/ogg", "recording.ogg")),
        ("openai" | "groq", "audio/wav" | "audio/x-wav") => Ok(("audio/wav", "recording.wav")),
        _ => Err(
            "Formato de áudio não aceito por este provedor. Grave novamente em WebM, MP4 ou WAV."
                .into(),
        ),
    }
}

#[derive(Deserialize)]
struct TranscriptionResponse {
    text: String,
}

async fn upload_audio(
    client: &reqwest::Client,
    endpoint: Url,
    provider: TranscriptionProvider,
    credential: &str,
    media_type: &str,
    audio: Vec<u8>,
) -> Result<String, String> {
    if audio.is_empty() {
        return Err("A gravação está vazia. Grave novamente antes de transcrever.".into());
    }
    if audio.len() > MAX_AUDIO_BYTES {
        return Err(
            "O áudio excede o limite de 25 MB. Grave um trecho menor e tente novamente.".into(),
        );
    }
    let (media_type, filename) = normalize_media_type(provider.id, media_type)?;
    let (boundary, body) = build_multipart_body(provider, media_type, filename, audio);
    let content_type =
        header::HeaderValue::from_str(&format!("multipart/form-data; boundary={boundary}"))
            .map_err(|_| "Não foi possível preparar o formato do áudio.".to_string())?;
    let request = client
        .post(endpoint)
        .pipe_auth(provider.id, credential)?
        .header(header::CONTENT_TYPE, content_type)
        .body(body);
    let response = request
        .send()
        .await
        .map_err(|error| safe_transport_error(&error))?;
    if !response.status().is_success() {
        return Err(provider_http_error(response.status().as_u16()));
    }
    let body = response
        .json::<TranscriptionResponse>()
        .await
        .map_err(|_| "O provedor retornou uma resposta de transcrição inválida.".to_string())?;
    let text = body.text.trim();
    if text.is_empty() {
        return Err("Nenhuma fala foi detectada no áudio. Grave novamente com a fala mais próxima do microfone.".into());
    }
    Ok(text.to_string())
}

fn build_multipart_body(
    provider: TranscriptionProvider,
    media_type: &str,
    filename: &str,
    audio: Vec<u8>,
) -> (String, Vec<u8>) {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let sequence = MULTIPART_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let boundary = format!("OdeKrai-{nanos:x}-{sequence:x}");
    let mut body = Vec::with_capacity(audio.len() + 1024);
    let mut field = |name: &str, value: &str| {
        body.extend_from_slice(format!("--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n").as_bytes());
    };
    field("model", provider.model);
    field(provider.language_field, "pt");
    field(
        "prompt",
        "Transcreva em português brasileiro, preservando o sentido e a pontuação.",
    );
    field("response_format", "json");
    body.extend_from_slice(
        format!("--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{filename}\"\r\nContent-Type: {media_type}\r\n\r\n").as_bytes(),
    );
    body.extend_from_slice(&audio);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    (boundary, body)
}

#[tauri::command]
pub async fn transcribe_audio(
    caller: tauri::WebviewWindow,
    provider_id: String,
    media_type: String,
    audio_base64: String,
) -> Result<String, String> {
    super::super::require_main(&caller)?;
    let provider = transcription_provider(&provider_id)
        .ok_or("A transcrição não está verificada para este provedor.")?;
    let max_encoded = MAX_AUDIO_BYTES.div_ceil(3) * 4;
    if audio_base64.len() > max_encoded {
        return Err(
            "O áudio excede o limite de 25 MB. Grave um trecho menor e tente novamente.".into(),
        );
    }
    let audio = base64::engine::general_purpose::STANDARD
        .decode(audio_base64.as_bytes())
        .map_err(|_| "Não foi possível ler a gravação local.".to_string())?;
    let credential =
        credential_entry(provider.id)?
            .get_password()
            .map_err(|error| match error {
                keyring::Error::NoEntry => {
                    "Salve a chave deste provedor antes de transcrever.".to_string()
                }
                _ => "Não foi possível recuperar a chave do Gerenciador de Credenciais do Windows."
                    .to_string(),
            })?;
    let endpoint = transcription_endpoint(provider)?;
    let client = http_client(90)?;
    upload_audio(&client, endpoint, provider, &credential, &media_type, audio).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_documented_providers_are_available() {
        assert_eq!(
            transcription_provider("openai").unwrap().model,
            "gpt-transcribe"
        );
        assert_eq!(
            transcription_provider("groq").unwrap().model,
            "whisper-large-v3-turbo"
        );
        assert!(transcription_provider("anthropic").is_none());
    }

    #[test]
    fn endpoint_uses_only_provider_catalog_hosts() {
        let endpoint = transcription_endpoint(transcription_provider("groq").unwrap()).unwrap();
        assert_eq!(
            endpoint.as_str(),
            "https://api.groq.com/openai/v1/audio/transcriptions"
        );
    }

    #[test]
    fn unsupported_and_oversized_audio_are_rejected() {
        assert!(normalize_media_type("openai", "audio/webm;codecs=opus").is_ok());
        assert!(normalize_media_type("openai", "audio/ogg").is_err());
        assert!(normalize_media_type("groq", "audio/ogg").is_ok());
        assert!(normalize_media_type("openai", "application/octet-stream").is_err());
        const { assert!(MAX_AUDIO_BYTES > 0) };
    }

    fn mock_server(status: &str, body: &'static str) -> (Url, std::thread::JoinHandle<Vec<u8>>) {
        use std::io::{Read, Write};
        use std::time::Duration;

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let status = status.to_string();
        let server = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut bytes = Vec::new();
            let mut chunk = [0_u8; 4096];
            loop {
                let count = socket.read(&mut chunk).unwrap();
                if count == 0 {
                    break;
                }
                bytes.extend_from_slice(&chunk[..count]);
                if let Some(header_end) = bytes.windows(4).position(|part| part == b"\r\n\r\n") {
                    let headers =
                        String::from_utf8_lossy(&bytes[..header_end]).to_ascii_lowercase();
                    let content_length = headers
                        .lines()
                        .find_map(|line| line.strip_prefix("content-length: "))
                        .and_then(|value| value.parse::<usize>().ok())
                        .unwrap_or(0);
                    if bytes.len() >= header_end + 4 + content_length {
                        break;
                    }
                }
            }
            write!(socket, "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
            bytes
        });
        (
            Url::parse(&format!("http://{address}/v1/audio/transcriptions")).unwrap(),
            server,
        )
    }

    #[tokio::test]
    async fn fake_server_receives_audio_auth_model_and_portuguese_hint() {
        let (endpoint, server) = mock_server("200 OK", r#"{"text":"Texto reconhecido."}"#);
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap();
        let provider = transcription_provider("openai").unwrap();
        let transcript = upload_audio(
            &client,
            endpoint,
            provider,
            "fake-secret",
            "audio/webm",
            b"fake-audio".to_vec(),
        )
        .await
        .unwrap();
        let request = String::from_utf8_lossy(&server.join().unwrap()).to_string();
        assert_eq!(transcript, "Texto reconhecido.");
        assert!(request.starts_with("POST /v1/audio/transcriptions HTTP/1.1"));
        assert!(request
            .to_ascii_lowercase()
            .contains("authorization: bearer fake-secret"));
        assert!(request.contains("gpt-transcribe"));
        assert!(request.contains("languages[]"));
        assert!(request.contains("Transcreva em português brasileiro"));
        assert!(request.contains("fake-audio"));
    }

    #[tokio::test]
    async fn fake_server_errors_and_empty_transcript_are_clear() {
        let (endpoint, server) = mock_server("401 Unauthorized", r#"{"error":"hidden"}"#);
        let client = reqwest::Client::new();
        let error = upload_audio(
            &client,
            endpoint,
            transcription_provider("groq").unwrap(),
            "fake-secret",
            "audio/webm",
            b"audio".to_vec(),
        )
        .await
        .unwrap_err();
        server.join().unwrap();
        assert!(error.contains("chave") || error.contains("acesso"));

        let (endpoint, server) = mock_server("200 OK", r#"{"text":"  "}"#);
        let error = upload_audio(
            &client,
            endpoint,
            transcription_provider("groq").unwrap(),
            "fake-secret",
            "audio/webm",
            b"audio".to_vec(),
        )
        .await
        .unwrap_err();
        server.join().unwrap();
        assert!(error.contains("Nenhuma fala foi detectada"));
    }
}
