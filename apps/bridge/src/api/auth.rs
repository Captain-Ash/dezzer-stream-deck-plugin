//! Authentification et contrôle d'origine de l'API locale.
//!
//! Le bridge n'écoute que sur la boucle locale, mais cela ne suffit pas : n'importe quelle
//! page web ouverte dans le navigateur de l'utilisateur peut adresser `127.0.0.1`. Le token
//! est la protection réelle ; le contrôle d'origine et de `Host` bloque en plus les attaques
//! par réattribution DNS.

use axum::http::HeaderMap;
use subtle::ConstantTimeEq;

pub const HEADER_AUTHORIZATION: &str = "authorization";
pub const QUERY_TOKEN: &str = "token";

/// Comparaison à temps constant : évite de divulguer le token octet par octet.
pub fn token_matches(expected: &str, provided: &str) -> bool {
    let expected = expected.as_bytes();
    let provided = provided.as_bytes();
    if expected.len() != provided.len() {
        return false;
    }
    expected.ct_eq(provided).into()
}

/// Extrait le token d'un en-tête `Authorization: Bearer …` ou d'un paramètre `?token=`.
///
/// Le paramètre de requête n'est là que pour l'overlay et la balise `<img>` d'artwork :
/// une Browser Source OBS ne peut pas fixer d'en-tête.
pub fn extract_token(headers: &HeaderMap, query: Option<&str>) -> Option<String> {
    if let Some(value) = headers
        .get(HEADER_AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
    {
        let trimmed = value.trim();
        if let Some(token) = trimmed
            .strip_prefix("Bearer ")
            .or_else(|| trimmed.strip_prefix("bearer "))
        {
            let token = token.trim();
            if !token.is_empty() {
                return Some(token.to_string());
            }
        }
    }

    query
        .and_then(|q| {
            q.split('&').find_map(|pair| {
                let (key, value) = pair.split_once('=')?;
                (key == QUERY_TOKEN).then_some(value)
            })
        })
        .and_then(urldecode)
        .filter(|t| !t.is_empty())
}

/// Une origine absente (appel natif depuis le plugin) est acceptée ; une origine web
/// distante est refusée.
pub fn origin_allowed(origin: Option<&str>) -> bool {
    match origin {
        None => true,
        Some("null") => true,
        Some(value) => {
            let value = value.trim();
            value.starts_with("file://") || is_loopback_origin(value)
        }
    }
}

/// Protection contre la réattribution DNS : l'en-tête `Host` doit désigner la boucle locale.
pub fn host_allowed(host: Option<&str>) -> bool {
    let Some(host) = host else { return false };
    let hostname = strip_port(host.trim());
    matches!(hostname, "127.0.0.1" | "localhost" | "::1" | "[::1]")
}

fn is_loopback_origin(origin: &str) -> bool {
    let Some(rest) = origin
        .strip_prefix("http://")
        .or_else(|| origin.strip_prefix("https://"))
    else {
        return false;
    };
    host_allowed(Some(rest))
}

fn strip_port(authority: &str) -> &str {
    if let Some(end) = authority.strip_prefix('[').and_then(|r| r.find(']')) {
        return &authority[..end + 2];
    }
    match authority.rsplit_once(':') {
        Some((host, port)) if port.chars().all(|c| c.is_ascii_digit()) => host,
        _ => authority,
    }
}

fn urldecode(raw: &str) -> Option<String> {
    let bytes = raw.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' => {
                let hex = raw.get(i + 1..i + 3)?;
                out.push(u8::from_str_radix(hex, 16).ok()?);
                i += 3;
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            other => {
                out.push(other);
                i += 1;
            }
        }
    }
    String::from_utf8(out).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    fn headers(pairs: &[(&str, &str)]) -> HeaderMap {
        let mut map = HeaderMap::new();
        for (key, value) in pairs {
            map.insert(
                axum::http::HeaderName::from_bytes(key.as_bytes()).unwrap(),
                HeaderValue::from_str(value).unwrap(),
            );
        }
        map
    }

    #[test]
    fn compare_les_tokens_a_temps_constant() {
        assert!(token_matches("abcdef", "abcdef"));
        assert!(!token_matches("abcdef", "abcdeg"));
        assert!(!token_matches("abcdef", "abcde"));
        assert!(!token_matches("", "x"));
    }

    #[test]
    fn extrait_le_token_du_header_bearer() {
        let map = headers(&[("authorization", "Bearer s3cr3t")]);
        assert_eq!(extract_token(&map, None).as_deref(), Some("s3cr3t"));
    }

    #[test]
    fn extrait_le_token_de_la_query_pour_l_overlay() {
        let map = HeaderMap::new();
        assert_eq!(
            extract_token(&map, Some("theme=glass&token=s3cr3t&width=720")).as_deref(),
            Some("s3cr3t")
        );
    }

    #[test]
    fn ignore_un_header_authorization_vide_ou_mal_forme() {
        assert!(extract_token(&headers(&[("authorization", "Bearer ")]), None).is_none());
        assert!(extract_token(&headers(&[("authorization", "Basic abc")]), None).is_none());
    }

    #[test]
    fn refuse_une_origine_web_distante() {
        assert!(!origin_allowed(Some("https://evil.example")));
        assert!(!origin_allowed(Some("http://127.0.0.1.evil.example")));
        assert!(origin_allowed(None));
        assert!(origin_allowed(Some("null")));
        assert!(origin_allowed(Some("file:///C:/plugin/pi.html")));
        assert!(origin_allowed(Some("http://127.0.0.1:53211")));
        assert!(origin_allowed(Some("http://localhost:53211")));
    }

    #[test]
    fn refuse_un_host_non_loopback() {
        assert!(host_allowed(Some("127.0.0.1:53211")));
        assert!(host_allowed(Some("localhost")));
        assert!(host_allowed(Some("[::1]:53211")));
        assert!(!host_allowed(Some("attacker.example")));
        assert!(!host_allowed(Some("192.168.1.20:53211")));
        assert!(!host_allowed(None));
    }
}
