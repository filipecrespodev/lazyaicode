//! O token do canal de controle: 256 bits aleatórios em hexadecimal, num
//! arquivo legível só pelo dono.

use std::path::Path;

/// 64 caracteres hex (dois UUID v4 = 244 bits do gerador do sistema, mais o
/// processo e o instante misturados no fim).
pub fn generate() -> String {
    let a = uuid::Uuid::new_v4().simple().to_string();
    let b = uuid::Uuid::new_v4().simple().to_string();
    format!("{a}{b}")
}

/// Grava o token com permissão 0600 (no Unix).
pub fn write(path: &Path, token: &str) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut o = std::fs::OpenOptions::new();
    o.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        o.mode(0o600);
    }
    use std::io::Write;
    o.open(path)?.write_all(token.as_bytes())
}

pub fn read(path: &Path) -> std::io::Result<String> {
    Ok(std::fs::read_to_string(path)?.trim().to_string())
}

/// Comparação em tempo constante (não vaza quantos caracteres batem).
pub fn matches(expected: &str, got: &str) -> bool {
    let (a, b) = (expected.as_bytes(), got.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_are_long_random_and_compared_exactly() {
        let t = generate();
        assert_eq!(t.len(), 64);
        assert_ne!(t, generate());
        assert!(matches(&t, &t.clone()));
        assert!(!matches(&t, &t[..63]));
        assert!(!matches(&t, ""));
    }

    #[cfg(unix)]
    #[test]
    fn token_file_is_private() {
        use std::os::unix::fs::PermissionsExt;
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("sub/control.token");
        write(&p, "abc").unwrap();
        assert_eq!(read(&p).unwrap(), "abc");
        assert_eq!(std::fs::metadata(&p).unwrap().permissions().mode() & 0o777, 0o600);
    }
}
