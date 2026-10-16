use serde::Serialize;

pub use lazyaicode_git::editors::{self, EditorInfo};
pub use lazyaicode_git::error::GitCommandError;
pub use lazyaicode_git::state::AppState;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GlobalGitIdentity {
    pub name: Option<String>,
    pub email: Option<String>,
}

/// Reads the machine's global git identity (`user.name`/`user.email`) — used
/// to seed the "Default" commit profile the first time the app runs.
pub async fn get_global_git_identity() -> Result<GlobalGitIdentity, GitCommandError> {
    let cfg = git2::Config::open_default()?;
    Ok(GlobalGitIdentity { name: cfg.get_string("user.name").ok(), email: cfg.get_string("user.email").ok() })
}

/// Editors found on this machine, for the default-editor setting.
pub async fn list_installed_editors() -> Vec<EditorInfo> {
    editors::detect_editors(&editors::search_dirs())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemFonts {
    families: Vec<String>,
    /// What the system's default text / code fonts resolve to, for the "Default" label.
    default_text: String,
    default_code: String,
}

/// One family per line (`fc-list : family` gives "Family A,Family B" aliases
/// per line: the first is kept), sorted case-insensitively, without duplicates.
fn parse_font_families(text: &str) -> Vec<String> {
    let mut families: Vec<String> =
        text.lines().filter_map(|l| l.split(',').next().map(|f| f.trim().to_string())).filter(|f| !f.is_empty()).collect();
    families.sort_by_key(|f| f.to_lowercase());
    families.dedup();
    families
}

/// Font families installed on this machine, for the Appearance font pickers:
/// `fc-list` on Linux/macOS, WPF's font list through PowerShell on Windows.
pub async fn list_system_fonts() -> Result<SystemFonts, String> {
    use std::process::Command;
    // GUI launches often miss the shell's PATH (fontconfig under Homebrew, ...).
    let run = |programs: &[&str], args: &[&str]| -> Result<String, String> {
        let mut last = String::new();
        for p in programs {
            let mut cmd = Command::new(p);
            cmd.args(args);
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                cmd.creation_flags(0x0800_0000);
            }
            match cmd.output() {
                Ok(o) if o.status.success() => return Ok(String::from_utf8_lossy(&o.stdout).into_owned()),
                Ok(o) => last = format!("{p}: {}", String::from_utf8_lossy(&o.stderr).trim()),
                Err(e) => last = format!("{p}: {e}"),
            }
        }
        Err(last)
    };
    #[cfg(windows)]
    let (text, default_text, default_code) = (
        run(
            &["powershell"],
            &[
                "-NoProfile",
                "-Command",
                "Add-Type -AssemblyName PresentationCore; [Windows.Media.Fonts]::SystemFontFamilies | ForEach-Object { $_.Source }",
            ],
        )?,
        "Segoe UI".to_string(),
        "Consolas".to_string(),
    );
    #[cfg(not(windows))]
    let (text, default_text, default_code) = {
        let fc = ["fc-list", "/usr/bin/fc-list", "/usr/local/bin/fc-list", "/opt/homebrew/bin/fc-list"];
        let m = ["fc-match", "/usr/bin/fc-match", "/usr/local/bin/fc-match", "/opt/homebrew/bin/fc-match"];
        let first = |s: String| s.lines().next().unwrap_or("").split(',').next().unwrap_or("").trim().to_string();
        (
            run(&fc, &[":", "family"]).map_err(|e| format!("fontconfig is not available ({e})"))?,
            run(&m, &["sans-serif", "-f", "%{family}"]).map(first).unwrap_or_default(),
            run(&m, &["monospace", "-f", "%{family}"]).map(first).unwrap_or_default(),
        )
    };
    Ok(SystemFonts { families: parse_font_families(&text), default_text, default_code })
}

/// Absolute path of `rel_path` inside the repo's working directory.
pub async fn repo_file_path(repo_id: String, rel_path: String, state: &AppState) -> Result<String, GitCommandError> {
    let root = state.repo_path(&repo_id)?;
    Ok(std::path::Path::new(&root).join(rel_path).to_string_lossy().into_owned())
}

/// Opens `path` with the editor launcher `command` (from `list_installed_editors`).
pub async fn open_in_editor(command: String, path: String) -> Result<(), GitCommandError> {
    editors::open_with(&command, std::path::Path::new(&path))
        .map_err(|e| GitCommandError::Internal(format!("Não foi possível abrir {command}: {e}")))
}

#[cfg(test)]
mod tests {
    use super::parse_font_families;

    #[test]
    fn font_families_keep_first_alias_sorted_and_unique() {
        let out = "DejaVu Sans,DejaVu Sans Condensed\nnoto sans\n\nDejaVu Sans\n  Fira Code \n";
        assert_eq!(parse_font_families(out), vec!["DejaVu Sans", "Fira Code", "noto sans"]);
    }
}
