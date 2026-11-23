//! Project folders and their `<folder>.localhost` domains.
//!
//! Browsers resolve every `*.localhost` name to the loopback address on their
//! own (RFC 6761), so a project gets its own origin at
//! `http://<folder>.localhost:<web port>/` without editing the hosts file and
//! without administrator rights. Caddy maps the name back to the folder at
//! request time, so a new folder works without restarting the web server.

/// Folders whose own `public/index.php` or `public/index.html` exists are
/// served from `public/` (Laravel, Symfony, Slim, ...).
pub const PUBLIC_DIR: &str = "public";

/// The domain label for a project folder, when the folder name is a plain DNS
/// label (letters, digits, and inner hyphens, at most 63 characters).
///
/// Linux file systems are case-sensitive and browsers lowercase the host, so
/// there a folder with capital letters gets no domain.
pub fn domain_label(folder: &str) -> Option<String> {
    let bytes = folder.as_bytes();
    let valid = !bytes.is_empty()
        && bytes.len() <= 63
        && bytes
            .iter()
            .all(|b| b.is_ascii_alphanumeric() || *b == b'-')
        && bytes[0] != b'-'
        && bytes[bytes.len() - 1] != b'-';
    if !valid || (cfg!(target_os = "linux") && folder.bytes().any(|b| b.is_ascii_uppercase())) {
        return None;
    }
    Some(folder.to_ascii_lowercase())
}

/// `http://<folder>.localhost:<port>/`, when the folder can have a domain.
pub fn domain_url(folder: &str, web_port: u16) -> Option<String> {
    domain_label(folder).map(|label| format!("http://{label}.localhost:{web_port}/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_names_get_a_domain() {
        assert_eq!(
            domain_url("shop", 8080).as_deref(),
            Some("http://shop.localhost:8080/")
        );
        assert_eq!(domain_label("my-app2").as_deref(), Some("my-app2"));
        assert_eq!(
            domain_label(&"a".repeat(63)).as_deref(),
            Some(&*"a".repeat(63))
        );
    }

    #[test]
    fn names_that_are_not_dns_labels_get_none() {
        for name in [
            "",
            "my shop",
            "my_shop",
            "-shop",
            "shop-",
            "a.b",
            "ร้านค้า",
            "../etc",
            &"a".repeat(64),
        ] {
            assert_eq!(domain_label(name), None, "{name:?}");
        }
    }

    #[test]
    fn capital_letters_follow_the_file_system() {
        let label = domain_label("MyShop");
        if cfg!(target_os = "linux") {
            assert_eq!(label, None);
        } else {
            assert_eq!(label.as_deref(), Some("myshop"));
        }
    }
}
