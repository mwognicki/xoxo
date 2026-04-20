use url::Url;

/// Masks the credential portion of a URL, preserving everything else.
///
/// - `scheme://token@host:port`         → `scheme://***@host:port`
/// - `scheme://user:password@host:port` → `scheme://user:***@host:port`
/// - URLs without credentials are returned unchanged.
/// - Unparseable strings are returned unchanged.
pub fn mask_url_credential(raw: &str) -> String {
    let Ok(mut url) = Url::parse(raw) else {
        return raw.to_string();
    };
    if url.password().is_some() {
        url.set_password(Some("***")).ok();
    } else if !url.username().is_empty() {
        url.set_username("***").ok();
    }
    url.to_string()
}

/// Masks an opaque secret value for safe display.
///
/// - `Some(_)` → `"***"` (value is present but hidden)
/// - `None`    → `"(not set)"` (field was not configured)
pub fn mask_secret(value: Option<&str>) -> String {
    match value {
        Some(_) => "***".into(),
        None => "(not set)".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_auth_masked() {
        assert_eq!(
            mask_url_credential("nats://mytoken@some.host:4222"),
            "nats://***@some.host:4222"
        );
    }

    #[test]
    fn user_password_auth_masked() {
        assert_eq!(
            mask_url_credential("nats://user:s3cr3t@some.host:4222"),
            "nats://user:***@some.host:4222"
        );
    }

    #[test]
    fn no_credentials_unchanged() {
        let url = "nats://some.host:4222";
        assert_eq!(mask_url_credential(url), url);
    }

    #[test]
    fn unparseable_url_unchanged() {
        let s = "not a url at all";
        assert_eq!(mask_url_credential(s), s);
    }

    #[test]
    fn secret_present_masked() {
        assert_eq!(mask_secret(Some("s3cr3t")), "***");
    }

    #[test]
    fn secret_absent_not_set() {
        assert_eq!(mask_secret(None), "(not set)");
    }
}
