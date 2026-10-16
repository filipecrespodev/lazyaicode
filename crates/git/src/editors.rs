use std::path::{Path, PathBuf};

use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EditorInfo {
    pub id: String,
    pub name: String,
    /// Absolute path of the launcher.
    pub command: String,
}

/// (id, display name, executable names tried in order)
const KNOWN: &[(&str, &str, &[&str])] = &[
    ("code", "VS Code", &["code"]),
    ("code-insiders", "VS Code Insiders", &["code-insiders"]),
    ("cursor", "Cursor", &["cursor"]),
    ("windsurf", "Windsurf", &["windsurf"]),
    ("zed", "Zed", &["zed", "zeditor"]),
    ("subl", "Sublime Text", &["subl"]),
    ("idea", "IntelliJ IDEA", &["idea", "idea.sh"]),
    ("webstorm", "WebStorm", &["webstorm", "webstorm.sh"]),
    ("pycharm", "PyCharm", &["pycharm", "pycharm.sh"]),
    ("rustrover", "RustRover", &["rustrover", "rustrover.sh"]),
    ("goland", "GoLand", &["goland", "goland.sh"]),
    ("clion", "CLion", &["clion", "clion.sh"]),
    ("phpstorm", "PhpStorm", &["phpstorm", "phpstorm.sh"]),
    ("gnome-text-editor", "Text Editor", &["gnome-text-editor"]),
    ("gedit", "gedit", &["gedit"]),
    ("kate", "Kate", &["kate"]),
    ("xed", "Xed", &["xed"]),
    ("mousepad", "Mousepad", &["mousepad"]),
    ("notepad++", "Notepad++", &["notepad++"]),
];

fn find_executable(name: &str, dirs: &[PathBuf]) -> Option<PathBuf> {
    let candidates = [name.to_string(), format!("{name}.exe"), format!("{name}.cmd")];
    dirs.iter().flat_map(|d| candidates.iter().map(move |c| d.join(c))).find(|p| p.is_file())
}

/// Known editors whose launcher is found in `dirs`.
pub fn detect_editors(dirs: &[PathBuf]) -> Vec<EditorInfo> {
    KNOWN
        .iter()
        .filter_map(|(id, name, names)| {
            let path = names.iter().find_map(|n| find_executable(n, dirs))?;
            Some(EditorInfo { id: id.to_string(), name: name.to_string(), command: path.to_string_lossy().into_owned() })
        })
        .collect()
}

/// PATH plus the usual install spots PATH misses for GUI apps.
pub fn search_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH").map(|p| std::env::split_paths(&p).collect()).unwrap_or_default();
    let mut extra: Vec<PathBuf> =
        ["/usr/local/bin", "/opt/homebrew/bin", "/snap/bin", "/var/lib/flatpak/exports/bin"].into_iter().map(PathBuf::from).collect();
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        extra.push(home.join(".local/bin"));
        extra.push(home.join(".local/share/JetBrains/Toolbox/scripts"));
    }
    if let Some(local) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from) {
        extra.push(local.join("Programs/Microsoft VS Code/bin"));
        extra.push(local.join("Programs/cursor/resources/app/bin"));
        extra.push(local.join("Programs/Zed"));
    }
    for pf in ["ProgramFiles", "ProgramFiles(x86)"] {
        if let Some(pf) = std::env::var_os(pf).map(PathBuf::from) {
            extra.push(pf.join("Notepad++"));
            extra.push(pf.join("Sublime Text"));
        }
    }
    extra.push(PathBuf::from("/Applications/Visual Studio Code.app/Contents/Resources/app/bin"));
    extra.push(PathBuf::from("/Applications/Cursor.app/Contents/Resources/app/bin"));
    extra.push(PathBuf::from("/Applications/Sublime Text.app/Contents/SharedSupport/bin"));
    dirs.extend(extra.into_iter().filter(|d| d.is_dir()));
    dirs
}

/// VS Code-family editors open a WSL folder through their Remote-WSL
/// extension (`--remote wsl+<distro> <linux path>`), so the editor's server
/// runs next to the files instead of reading them over 9P. Windows only.
#[cfg(windows)]
fn remote_target(command: &str, path: &Path) -> Option<(String, String)> {
    let stem = Path::new(command).file_stem()?.to_string_lossy().to_lowercase();
    if !["code", "cursor", "windsurf"].contains(&stem.as_str()) {
        return None;
    }
    let (distro, linux) = lazyaicode_platform::wslpath::parse_unc(&path.to_string_lossy())?;
    Some((format!("wsl+{distro}"), linux))
}

/// Launches `command` on `path`, detached (a GUI editor outlives the app call).
pub fn open_with(command: &str, path: &Path) -> std::io::Result<()> {
    // `.cmd` launchers (VS Code on Windows) run through cmd.exe. Rust's std
    // spawns them with cmd-safe argument escaping (and refuses arguments it
    // can't escape), so no hand-rolled `cmd /C` here; paths with cmd
    // metacharacters are refused outright as a second guard.
    #[cfg(windows)]
    if command.to_lowercase().ends_with(".cmd") && path.to_string_lossy().contains(['&', '|', '<', '>', '^', '%', '!', '"']) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "caminho com caracteres especiais do cmd; abra o arquivo pelo próprio editor",
        ));
    }
    let mut cmd = std::process::Command::new(command);
    #[cfg(windows)]
    match remote_target(command, path) {
        Some((target, linux)) => cmd.args(["--remote", &target, &linux]),
        None => cmd.arg(path),
    };
    #[cfg(not(windows))]
    cmd.arg(path);
    cmd.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn().map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(dir: &Path, name: &str) {
        std::fs::write(dir.join(name), "#!/bin/sh\n").unwrap();
    }

    #[test]
    fn detects_known_editors_in_the_given_dirs_only() {
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        touch(a.path(), "code");
        touch(b.path(), "zeditor"); // alias for Zed
        touch(b.path(), "not-an-editor");

        let found = detect_editors(&[a.path().to_path_buf(), b.path().to_path_buf()]);
        let ids: Vec<_> = found.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(ids, ["code", "zed"]);
        assert_eq!(found[0].command, a.path().join("code").to_string_lossy());
        assert!(detect_editors(&[]).is_empty());
    }

    #[test]
    fn prefers_the_first_directory_on_the_path() {
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        touch(a.path(), "subl");
        touch(b.path(), "subl");
        let found = detect_editors(&[a.path().to_path_buf(), b.path().to_path_buf()]);
        assert_eq!(found.len(), 1);
        assert!(found[0].command.starts_with(&a.path().to_string_lossy().into_owned()));
    }
}
