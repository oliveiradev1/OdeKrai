use serde::Deserialize;
use tauri::{AppHandle, WebviewWindow};
use tauri_plugin_opener::OpenerExt;
use url::Url;

const CLOUD_LINKS_MANIFEST: &str = include_str!("../../src/core/cloud-links.json");

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CloudLinksManifest {
    allowed_domains: Vec<String>,
    categories: Vec<CloudLinkCategory>,
}

#[derive(Deserialize)]
struct CloudLinkCategory {
    services: Vec<CloudLinkService>,
}

#[derive(Deserialize)]
struct CloudLinkService {
    url: String,
}

pub fn validate_external_url(value: &str) -> Result<String, &'static str> {
    if value.is_empty()
        || value.len() > 2048
        || value.trim() != value
        || value.chars().any(char::is_control)
        || value.contains('\\')
        || value.chars().any(char::is_whitespace)
        || has_encoded_control(value)
    {
        return Err("O endereço do link é inválido.");
    }

    let url = Url::parse(value).map_err(|_| "O endereço do link é inválido.")?;
    match url.scheme() {
        "http" | "https"
            if url.host_str().is_some()
                && url.username().is_empty()
                && url.password().is_none()
                && !is_private_address(&url) =>
        {
            Ok(url.to_string())
        }
        "mailto" if valid_mailto_address(url.path()) => Ok(url.to_string()),
        _ => Err("Este tipo de link não pode ser aberto."),
    }
}

fn is_private_address(url: &Url) -> bool {
    let Some(host) = url.host() else { return true };
    let ip = match host {
        url::Host::Ipv4(ip) => Some(std::net::IpAddr::V4(ip)),
        url::Host::Ipv6(ip) => Some(std::net::IpAddr::V6(ip)),
        url::Host::Domain(_) => None,
    };
    ip.is_some_and(|ip| match ip {
        std::net::IpAddr::V4(ip) => {
            ip.is_private()
                || ip.is_loopback()
                || ip.is_link_local()
                || ip.is_unspecified()
                || ip.is_broadcast()
                || ip.octets()[0] == 0
                || ip.octets()[0] >= 224
        }
        std::net::IpAddr::V6(ip) => {
            ip.is_loopback()
                || ip.is_unspecified()
                || ip.is_unique_local()
                || ip.is_unicast_link_local()
                || ip
                    .to_ipv4_mapped()
                    .is_some_and(|v4| v4.is_private() || v4.is_loopback() || v4.is_link_local())
        }
    })
}

fn has_encoded_control(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.windows(3).any(|window| {
        window[0] == b'%'
            && matches!(window[1].to_ascii_lowercase(), b'0' | b'1' | b'7')
            && window[2].is_ascii_hexdigit()
            && (window[1] != b'7' || window[2].eq_ignore_ascii_case(&b'f'))
    })
}

fn valid_mailto_address(path: &str) -> bool {
    let Some((local, domain)) = path.split_once('@') else {
        return false;
    };
    !local.is_empty() && !domain.is_empty() && !domain.contains('@')
}

#[tauri::command]
pub fn open_external_link(
    app: AppHandle,
    caller: WebviewWindow,
    url: String,
) -> Result<(), String> {
    crate::require_main(&caller)?;
    let validated = validate_external_url(&url).map_err(str::to_string)?;
    let parsed = Url::parse(&validated).map_err(|_| "O endereço do link é inválido.")?;
    if let Some(host) = parsed.host_str() {
        if !crate::security::external_domain_is_trusted(&app, host)? {
            return Err("Confirme este domínio em Links externos antes de abrir o link.".into());
        }
    }
    app.opener()
        .open_url(validated, None::<&str>)
        .map_err(|_| "Não foi possível abrir o navegador padrão.".to_string())
}

fn validate_cloud_link(value: &str) -> Result<String, String> {
    let validated = validate_external_url(value).map_err(str::to_string)?;
    let parsed = Url::parse(&validated).map_err(|_| "O endereço do link é inválido.")?;
    if parsed.scheme() != "https" || parsed.port_or_known_default() != Some(443) {
        return Err("Os links da Nuvem precisam usar HTTPS sem porta alternativa.".into());
    }
    let manifest: CloudLinksManifest = serde_json::from_str(CLOUD_LINKS_MANIFEST)
        .map_err(|_| "A lista de domínios autorizados da Nuvem está inválida.".to_string())?;
    let invalid_manifest_url = manifest
        .categories
        .iter()
        .flat_map(|category| &category.services)
        .any(|service| match Url::parse(&service.url) {
            Ok(service_url) => {
                service_url.scheme() != "https"
                    || !manifest
                        .allowed_domains
                        .iter()
                        .any(|domain| service_url.host_str() == Some(domain.as_str()))
            }
            Err(_) => true,
        });
    if invalid_manifest_url {
        return Err("A lista de domínios autorizados da Nuvem está inválida.".into());
    }
    if !manifest
        .allowed_domains
        .iter()
        .any(|domain| parsed.host_str() == Some(domain.as_str()))
    {
        return Err("Este domínio não está na lista autorizada de links da Nuvem.".into());
    }
    Ok(validated)
}

#[tauri::command]
pub fn open_cloud_link(app: AppHandle, caller: WebviewWindow, url: String) -> Result<(), String> {
    crate::require_main(&caller)?;
    let validated = validate_cloud_link(&url)?;
    app.opener()
        .open_url(validated, None::<&str>)
        .map_err(|_| "Não foi possível abrir o link da Nuvem no navegador padrão.".to_string())
}

#[tauri::command]
pub fn open_update_release(
    app: AppHandle,
    caller: WebviewWindow,
    url: String,
) -> Result<(), String> {
    crate::require_main(&caller)?;
    let validated = validate_update_release_url(&url)?;
    app.opener()
        .open_url(validated, None::<&str>)
        .map_err(|_| "Não foi possível abrir o navegador padrão.".to_string())
}

fn validate_update_release_url(value: &str) -> Result<String, String> {
    let validated = validate_external_url(value).map_err(str::to_string)?;
    let parsed = Url::parse(&validated).map_err(|_| "O endereço do link é inválido.")?;
    let path_parts = parsed.path().split('/').collect::<Vec<_>>();
    let valid_tag = path_parts.last().copied().unwrap_or("");
    if parsed.scheme() != "https"
        || parsed.host_str() != Some("github.com")
        || parsed.query().is_some()
        || parsed.fragment().is_some()
        || path_parts.len() != 6
        || path_parts[..5] != ["", "oliveiradev1", "OdeKrai", "releases", "tag"]
        || semver::Version::parse(valid_tag.strip_prefix('v').unwrap_or(valid_tag)).is_err()
    {
        return Err("Este link não é uma release oficial do OdeKrai.".into());
    }
    Ok(validated)
}

#[cfg(test)]
mod tests {
    use super::{
        validate_cloud_link, validate_external_url, validate_update_release_url,
        CLOUD_LINKS_MANIFEST,
    };
    use serde::Deserialize;

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct TestManifest {
        allowed_domains: Vec<String>,
        categories: Vec<TestCategory>,
    }

    #[derive(Deserialize)]
    struct TestCategory {
        services: Vec<TestService>,
    }

    #[derive(Deserialize)]
    struct TestService {
        url: String,
    }

    #[test]
    fn cloud_links_require_https_and_an_exact_domain_from_the_embedded_list() {
        assert_eq!(
            validate_cloud_link("https://shardcloud.app/pt-br/dash/vps").unwrap(),
            "https://shardcloud.app/pt-br/dash/vps"
        );
        assert!(validate_cloud_link("http://shardcloud.app/pt-br/dash/vps")
            .unwrap_err()
            .contains("HTTPS"));
        assert!(validate_cloud_link("https://shardcloud.app.evil.example/")
            .unwrap_err()
            .contains("não está na lista autorizada"));
        assert!(validate_cloud_link("https://example.com/")
            .unwrap_err()
            .contains("não está na lista autorizada"));
    }

    #[test]
    fn cloud_manifest_urls_are_https_and_match_the_embedded_allowed_domain_list() {
        let manifest: TestManifest = serde_json::from_str(CLOUD_LINKS_MANIFEST).unwrap();
        let services = manifest
            .categories
            .iter()
            .flat_map(|category| &category.services)
            .collect::<Vec<_>>();
        assert_eq!(services.len(), 7);
        for service in services {
            let url = url::Url::parse(&service.url).unwrap();
            assert_eq!(url.scheme(), "https");
            assert!(manifest
                .allowed_domains
                .iter()
                .any(|domain| url.host_str() == Some(domain.as_str())));
        }

        let capability: serde_json::Value =
            serde_json::from_str(include_str!("../capabilities/default.json")).unwrap();
        let permission = capability["permissions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|permission| permission["identifier"] == "opener:allow-open-url")
            .unwrap();
        let scoped_urls = permission["allow"].as_array().unwrap();
        for domain in &manifest.allowed_domains {
            let expected = format!("https://{domain}/**");
            assert!(scoped_urls
                .iter()
                .any(|entry| entry["url"].as_str() == Some(expected.as_str())));
        }
        assert!(!scoped_urls
            .iter()
            .any(|entry| entry["url"].as_str() == Some("https://**")));
    }

    #[test]
    fn accepts_only_http_https_and_mailto_links_without_credentials_or_controls() {
        for url in ["https://example.com/docs", "mailto:hello@example.com"] {
            assert!(validate_external_url(url).is_ok(), "{url}");
        }

        for url in [
            "javascript:alert(1)",
            "data:text/html,hello",
            "file:///C:/secret.txt",
            "https://trusted.example@evil.example/path",
            "https://example.com/\u{0007}path",
            "https://exa\nmple.com",
            "mailto:me@example.com?subject=a%0d%0aBcc%3Dx%40evil.com",
            "https:\\\\evil.example",
            "http://127.0.0.1:8080/path",
            "http://10.0.0.1/",
            "http://192.168.1.1/",
            "http://[::1]/",
            "http://[fc00::1]/",
            "http://[::ffff:127.0.0.1]/",
            "custom:payload",
        ] {
            assert!(validate_external_url(url).is_err(), "{url:?}");
        }
    }

    #[test]
    fn release_link_requires_the_fixed_github_repository() {
        for url in [
            "https://github.com/oliveiradev1/OdeKrai/releases/tag/v0.16.0",
            "https://github.com/oliveiradev1/OdeKrai/releases/tag/v0.16.1",
        ] {
            assert!(validate_update_release_url(url).is_ok());
        }
        for url in [
            "https://github.com/other/repo/releases/tag/v1",
            "https://github.com/oliveiradev1/OdeKrai/releases/tag/v1?next=evil",
            "https://github.com.evil/oliveiradev1/OdeKrai/releases/tag/v1",
            "https://github.com/oliveiradev1/OdeKrai/releases/",
        ] {
            assert!(validate_update_release_url(url).is_err());
        }
    }
}
