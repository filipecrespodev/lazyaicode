//! Repos inside WSL are identified in the app by their Windows UNC path
//! (`\\wsl.localhost\<distro>\home\me\repo`), which is what the native folder
//! dialog, drag-and-drop and Explorer all hand over. This maps that to
//! `(distro, /linux/path)` and back.

const HOSTS: [&str; 2] = ["wsl.localhost", "wsl$"];

/// `(distro, linux path)` for a WSL UNC path; `None` for anything else.
/// Also accepts forward slashes and the `\\?\UNC\` verbatim form.
pub fn parse_unc(path: &str) -> Option<(String, String)> {
    let p = path.replace('/', "\\");
    let rest = p
        .strip_prefix("\\\\?\\UNC\\")
        .or_else(|| p.strip_prefix("\\\\"))?;
    let mut parts = rest.split('\\').filter(|s| !s.is_empty());
    let host = parts.next()?;
    if !HOSTS.iter().any(|h| h.eq_ignore_ascii_case(host)) {
        return None;
    }
    let distro = parts.next()?.to_string();
    let linux: Vec<&str> = parts.collect();
    Some((distro, format!("/{}", linux.join("/"))))
}

/// Inverse of `parse_unc`, in the `wsl.localhost` spelling.
pub fn to_unc(distro: &str, linux: &str) -> String {
    let tail = linux.trim_matches('/').replace('/', "\\");
    if tail.is_empty() {
        format!("\\\\wsl.localhost\\{distro}")
    } else {
        format!("\\\\wsl.localhost\\{distro}\\{tail}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_both_hosts_and_verbatim() {
        let want = Some(("Ubuntu".to_string(), "/home/me/repo".to_string()));
        assert_eq!(parse_unc(r"\\wsl.localhost\Ubuntu\home\me\repo"), want);
        assert_eq!(parse_unc(r"\\wsl$\Ubuntu\home\me\repo\"), want);
        assert_eq!(parse_unc(r"\\?\UNC\wsl.localhost\Ubuntu\home\me\repo"), want);
        assert_eq!(parse_unc("//wsl.localhost/Ubuntu/home/me/repo"), want);
        assert_eq!(parse_unc(r"\\WSL$\Ubuntu"), Some(("Ubuntu".into(), "/".into())));
    }

    #[test]
    fn rejects_other_paths() {
        assert_eq!(parse_unc(r"C:\Users\me\repo"), None);
        assert_eq!(parse_unc("/home/me/repo"), None);
        assert_eq!(parse_unc(r"\\server\share\repo"), None);
        assert_eq!(parse_unc(r"\\wsl.localhost"), None);
    }

    #[test]
    fn round_trips() {
        let unc = to_unc("Debian", "/srv/x y/repo");
        assert_eq!(unc, r"\\wsl.localhost\Debian\srv\x y\repo");
        assert_eq!(parse_unc(&unc), Some(("Debian".into(), "/srv/x y/repo".into())));
    }
}
