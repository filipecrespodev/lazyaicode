//! Skills, agentes e comandos encontrados no disco, com o arquivo de origem
//! de cada um — o que permite abrir, apagar e mostrar erros de carga, como a
//! página Skills do Delta. Quem decide o que está *carregado* é o CLI (via
//! `initialize`); este módulo só diz onde cada coisa mora.

use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Entry {
    /// skill | agent | command
    pub kind: String,
    /// Nome como o CLI o expõe (`plugin:nome` para itens de plugin).
    pub name: String,
    pub description: String,
    /// global | project | plugin
    pub scope: String,
    pub plugin: Option<String>,
    pub path: String,
    pub model: Option<String>,
    pub tools: Option<String>,
    pub error: Option<String>,
}

/// Frontmatter YAML mínimo: `chave: valor` (com aspas opcionais) e blocos
/// `>`/`|` indentados. Suficiente para name/description/model/tools.
pub fn frontmatter(text: &str) -> Result<HashMap<String, String>, String> {
    let text = text.trim_start_matches('\u{feff}');
    let mut lines = text.lines();
    if lines.next().map(str::trim) != Some("---") {
        return Err("sem frontmatter (bloco --- no início)".into());
    }
    let mut map = HashMap::new();
    let mut block: Option<(String, bool, Vec<String>)> = None;
    let mut closed = false;
    for line in lines {
        if line.trim() == "---" {
            closed = true;
            break;
        }
        if let Some((key, folded, buf)) = block.as_mut() {
            if line.starts_with(' ') || line.starts_with('\t') || line.trim().is_empty() {
                buf.push(line.trim().to_string());
                continue;
            }
            let joined = if *folded { buf.join(" ") } else { buf.join("\n") };
            map.insert(key.clone(), joined.trim().to_string());
            block = None;
        }
        if line.starts_with(' ') || line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        let Some((k, v)) = line.split_once(':') else { continue };
        let (k, v) = (k.trim().to_string(), v.trim());
        if v == ">" || v == ">-" || v == "|" || v == "|-" {
            block = Some((k, v.starts_with('>'), vec![]));
            continue;
        }
        let v = v
            .strip_prefix('"')
            .and_then(|x| x.strip_suffix('"'))
            .or_else(|| v.strip_prefix('\'').and_then(|x| x.strip_suffix('\'')))
            .unwrap_or(v);
        map.insert(k, v.to_string());
    }
    if let Some((key, folded, buf)) = block {
        let joined = if folded { buf.join(" ") } else { buf.join("\n") };
        map.insert(key, joined.trim().to_string());
    }
    if !closed {
        return Err("frontmatter sem o --- de fechamento".into());
    }
    Ok(map)
}

fn read_entry(kind: &str, file: &Path, fallback_name: &str, scope: &str, plugin: Option<&str>) -> Entry {
    let (meta, mut error) = match std::fs::read_to_string(file) {
        Ok(t) => match frontmatter(&t) {
            Ok(m) => (m, None),
            // comandos podem ser só o prompt, sem frontmatter
            Err(_) if kind == "command" && !t.trim_start().starts_with("---") => (HashMap::new(), None),
            Err(e) => (HashMap::new(), Some(e)),
        },
        Err(e) => (HashMap::new(), Some(e.to_string())),
    };
    let base = meta.get("name").cloned().filter(|n| !n.is_empty()).unwrap_or_else(|| fallback_name.to_string());
    let description = meta.get("description").cloned().unwrap_or_default();
    if error.is_none() && description.is_empty() && kind != "command" {
        error = Some("frontmatter sem description".into());
    }
    Entry {
        kind: kind.into(),
        name: match plugin {
            Some(p) => format!("{p}:{base}"),
            None => base,
        },
        description,
        scope: scope.into(),
        plugin: plugin.map(str::to_string),
        path: file.display().to_string(),
        model: meta.get("model").cloned(),
        tools: meta.get("tools").cloned(),
        error,
    }
}

/// `SKILL.md` em `<root>/<nome>/` e em subpastas de categoria.
fn scan_skills(root: &Path, scope: &str, plugin: Option<&str>, depth: usize, out: &mut Vec<Entry>) {
    let Ok(rd) = std::fs::read_dir(root) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if !p.is_dir() {
            continue;
        }
        let skill = p.join("SKILL.md");
        if skill.is_file() {
            let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            out.push(read_entry("skill", &skill, &name, scope, plugin));
        } else if depth > 0 {
            scan_skills(&p, scope, plugin, depth - 1, out);
        }
    }
}

/// `*.md` de agentes/comandos; subpastas de comandos viram `pasta:nome`.
fn scan_md(kind: &str, root: &Path, prefix: &str, scope: &str, plugin: Option<&str>, depth: usize, out: &mut Vec<Entry>) {
    let Ok(rd) = std::fs::read_dir(root) else { return };
    for e in rd.flatten() {
        let p = e.path();
        let stem = p.file_stem().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        if p.is_dir() && depth > 0 {
            let pre = if kind == "command" { format!("{prefix}{stem}:") } else { prefix.to_string() };
            scan_md(kind, &p, &pre, scope, plugin, depth - 1, out);
        } else if p.extension().and_then(|x| x.to_str()) == Some("md") {
            out.push(read_entry(kind, &p, &format!("{prefix}{stem}"), scope, plugin));
        }
    }
}

fn scan_base(base: &Path, scope: &str, plugin: Option<&str>, out: &mut Vec<Entry>) {
    scan_skills(&base.join("skills"), scope, plugin, 3, out);
    scan_md("agent", &base.join("agents"), "", scope, plugin, 2, out);
    scan_md("command", &base.join("commands"), "", scope, plugin, 2, out);
}

/// Pastas de instalação dos plugins da conta, com o nome curto do plugin.
fn installed_plugins(claude_dir: &Path) -> Vec<(String, PathBuf)> {
    let Ok(text) = std::fs::read_to_string(claude_dir.join("plugins/installed_plugins.json")) else {
        return vec![];
    };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else { return vec![] };
    let mut out: Vec<(String, PathBuf)> = Vec::new();
    for (id, installs) in v["plugins"].as_object().into_iter().flatten() {
        let name = id.split('@').next().unwrap_or(id).to_string();
        for i in installs.as_array().into_iter().flatten() {
            if let Some(p) = i["installPath"].as_str() {
                let p = PathBuf::from(p);
                if !out.iter().any(|(_, q)| q == &p) {
                    out.push((name.clone(), p));
                }
            }
        }
    }
    out
}

/// Tudo o que existe no disco para uma conta e (opcionalmente) um projeto.
pub fn scan(claude_dir: &Path, project: Option<&Path>) -> Vec<Entry> {
    let mut out = Vec::new();
    scan_base(claude_dir, "global", None, &mut out);
    if let Some(p) = project {
        scan_base(&p.join(".claude"), "project", None, &mut out);
        scan_skills(&p.join(".agents/skills"), "project", None, 3, &mut out);
    }
    for (name, path) in installed_plugins(claude_dir) {
        scan_base(&path, "plugin", Some(&name), &mut out);
    }
    out
}

#[derive(Debug, Clone, Serialize)]
pub struct Item {
    #[serde(flatten)]
    pub entry: Entry,
    /// O CLI carregou este item nesta pasta.
    pub loaded: bool,
    pub builtin: bool,
    pub argument_hint: Option<String>,
}

fn item(entry: Entry, loaded: bool, builtin: bool, hint: Option<String>) -> Item {
    Item { entry, loaded, builtin, argument_hint: hint.filter(|h| !h.is_empty()) }
}

fn bare(kind: &str, name: &str, description: &str, scope: &str) -> Entry {
    Entry {
        kind: kind.into(),
        name: name.into(),
        description: description.into(),
        scope: scope.into(),
        plugin: name.split_once(':').map(|(p, _)| p.to_string()),
        path: String::new(),
        model: None,
        tools: None,
        error: None,
    }
}

/// Junta o que o CLI carregou (`initialize`) com o que está no disco.
/// Itens do disco que o CLI não carregou aparecem com `loaded: false` (e o
/// erro, quando há) — exceto os de plugins, que podem estar só desativados.
pub fn merge(init: &serde_json::Value, fs: Vec<Entry>) -> (Vec<Item>, Vec<Item>) {
    let mut fs = fs;
    let mut take = |kinds: &[&str], name: &str| -> Option<Entry> {
        let i = fs.iter().position(|e| kinds.contains(&e.kind.as_str()) && e.name == name)?;
        Some(fs.remove(i))
    };
    let mut commands = Vec::new();
    for c in init["commands"].as_array().into_iter().flatten() {
        let name = c["name"].as_str().unwrap_or_default();
        let desc = c["description"].as_str().unwrap_or_default();
        let builtin = c["builtin"].as_bool().unwrap_or(false);
        let hint = c["argumentHint"].as_str().map(str::to_string);
        match take(&["skill", "command"], name) {
            Some(mut e) => {
                if e.description.is_empty() {
                    e.description = desc.to_string();
                }
                e.error = None;
                commands.push(item(e, true, builtin, hint));
            }
            None => {
                let scope = if builtin { "builtin" } else if name.contains(':') { "plugin" } else { "other" };
                let kind = if builtin { "command" } else { "skill" };
                commands.push(item(bare(kind, name, desc, scope), true, builtin, hint));
            }
        }
    }
    let mut agents = Vec::new();
    for a in init["agents"].as_array().into_iter().flatten() {
        let name = a["name"].as_str().unwrap_or_default();
        let desc = a["description"].as_str().unwrap_or_default();
        match take(&["agent"], name) {
            Some(mut e) => {
                if e.description.is_empty() {
                    e.description = desc.to_string();
                }
                e.error = None;
                agents.push(item(e, true, false, None));
            }
            None => agents.push(item(bare("agent", name, desc, "builtin"), true, true, None)),
        }
    }
    for e in fs.into_iter().filter(|e| e.scope != "plugin") {
        let mut e = e;
        if e.error.is_none() {
            e.error = Some("não carregado pelo Claude Code nesta pasta".into());
        }
        if e.kind == "agent" {
            agents.push(item(e, false, false, None));
        } else {
            commands.push(item(e, false, false, None));
        }
    }
    (commands, agents)
}

/// Status de MCP sem segredos: headers, env e argumentos ficam de fora
/// (o CLI devolve tokens de autorização na config).
pub fn redact_mcp(servers: &serde_json::Value) -> serde_json::Value {
    let list = servers.as_array().cloned().unwrap_or_default();
    serde_json::Value::Array(
        list.into_iter()
            .map(|s| {
                let c = &s["config"];
                let url = c["url"].as_str().map(|u| u.split('?').next().unwrap_or(u).to_string());
                let command = c["command"].as_str().map(|cmd| cmd.rsplit('/').next().unwrap_or(cmd).to_string());
                serde_json::json!({
                    "name": s["name"],
                    "status": s["status"],
                    "scope": s["scope"],
                    "error": s["error"],
                    "serverInfo": s["serverInfo"],
                    "transport": c["type"],
                    "url": url,
                    "command": command,
                    "tools": s["tools"].as_array().into_iter().flatten()
                        .map(|t| serde_json::json!({ "name": t["name"], "description": t["description"] }))
                        .collect::<Vec<_>>(),
                })
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_quoted_and_folded_values() {
        let m = frontmatter("---\nname: \"revisor\"\ndescription: >\n  Revisa o diff\n  e aponta bugs\nmodel: sonnet\n---\ncorpo").unwrap();
        assert_eq!(m["name"], "revisor");
        assert_eq!(m["description"], "Revisa o diff e aponta bugs");
        assert_eq!(m["model"], "sonnet");
        assert!(frontmatter("sem bloco").is_err());
        assert!(frontmatter("---\nname: x\n").is_err());
    }

    #[test]
    fn finds_global_project_nested_and_plugin_items_and_reports_broken_ones() {
        let d = tempfile::tempdir().unwrap();
        let dir = d.path().join("claude");
        let proj = d.path().join("proj");
        let w = |p: PathBuf, t: &str| {
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, t).unwrap();
        };
        w(dir.join("skills/deploy/SKILL.md"), "---\nname: deploy\ndescription: Faz deploy\n---\n");
        w(dir.join("skills/cat/lint/SKILL.md"), "---\ndescription: Lint\n---\n");
        w(dir.join("agents/revisor.md"), "---\nname: revisor\ndescription: Revisa\ntools: Read, Grep\n---\n");
        w(dir.join("commands/git/pr.md"), "Abre um PR");
        w(proj.join(".claude/skills/quebrado/SKILL.md"), "sem frontmatter");
        let plugin = d.path().join("cache/meu-plugin/1.0");
        w(plugin.join("agents/qa.md"), "---\nname: qa\ndescription: QA\n---\n");
        w(
            dir.join("plugins/installed_plugins.json"),
            &serde_json::json!({ "plugins": { "meu-plugin@mkt": [{ "installPath": plugin }] } }).to_string(),
        );

        let mut all = scan(&dir, Some(&proj));
        all.sort_by(|a, b| a.name.cmp(&b.name));
        let names: Vec<_> = all.iter().map(|e| (e.kind.as_str(), e.name.as_str(), e.scope.as_str())).collect();
        assert_eq!(
            names,
            vec![
                ("skill", "deploy", "global"),
                ("command", "git:pr", "global"),
                ("skill", "lint", "global"),
                ("agent", "meu-plugin:qa", "plugin"),
                ("skill", "quebrado", "project"),
                ("agent", "revisor", "global"),
            ]
        );
        assert!(all.iter().find(|e| e.name == "quebrado").unwrap().error.is_some());
        assert_eq!(all.iter().find(|e| e.name == "revisor").unwrap().tools.as_deref(), Some("Read, Grep"));
        // comandos não exigem frontmatter
        assert!(all.iter().find(|e| e.name == "git:pr").unwrap().error.is_none());
    }

    #[test]
    fn merge_marks_what_the_cli_loaded_and_keeps_broken_files_visible() {
        let fs = vec![
            Entry { kind: "skill".into(), name: "deploy".into(), description: "".into(), scope: "global".into(), plugin: None, path: "/s/deploy/SKILL.md".into(), model: None, tools: None, error: None },
            Entry { kind: "skill".into(), name: "quebrado".into(), description: "".into(), scope: "project".into(), plugin: None, path: "/p/SKILL.md".into(), model: None, tools: None, error: Some("sem frontmatter".into()) },
            Entry { kind: "agent".into(), name: "p:qa".into(), description: "QA".into(), scope: "plugin".into(), plugin: Some("p".into()), path: "/x/qa.md".into(), model: None, tools: None, error: None },
            Entry { kind: "agent".into(), name: "p:off".into(), description: "".into(), scope: "plugin".into(), plugin: Some("p".into()), path: "/x/off.md".into(), model: None, tools: None, error: None },
        ];
        let init = serde_json::json!({
            "commands": [
                { "name": "compact", "description": "Compacta", "builtin": true },
                { "name": "deploy", "description": "Faz deploy", "argumentHint": "<env>" }
            ],
            "agents": [{ "name": "Explore", "description": "Busca" }, { "name": "p:qa", "description": "QA" }]
        });
        let (cmds, agents) = merge(&init, fs);
        let deploy = cmds.iter().find(|c| c.entry.name == "deploy").unwrap();
        assert!(deploy.loaded && !deploy.builtin);
        assert_eq!(deploy.entry.description, "Faz deploy");
        assert_eq!(deploy.argument_hint.as_deref(), Some("<env>"));
        assert!(cmds.iter().find(|c| c.entry.name == "compact").unwrap().builtin);
        let broken = cmds.iter().find(|c| c.entry.name == "quebrado").unwrap();
        assert!(!broken.loaded && broken.entry.error.as_deref() == Some("sem frontmatter"));
        assert!(agents.iter().find(|a| a.entry.name == "Explore").unwrap().builtin);
        assert_eq!(agents.iter().find(|a| a.entry.name == "p:qa").unwrap().entry.path, "/x/qa.md");
        // agente de plugin que o CLI não carregou (plugin desativado) não aparece
        assert!(agents.iter().all(|a| a.entry.name != "p:off"));
    }

    #[test]
    fn redaction_drops_headers_env_and_url_queries() {
        let raw = serde_json::json!([{
            "name": "Trevo", "status": "connected", "scope": "user",
            "config": { "type": "sse", "url": "https://x/sse?token=abc", "headers": { "Authorization": "Bearer segredo" } },
            "tools": [{ "name": "grep", "description": "busca", "inputSchema": {} }]
        }, {
            "name": "local", "status": "failed",
            "config": { "type": "stdio", "command": "/usr/bin/node", "args": ["--key=segredo"], "env": { "TOKEN": "segredo" } }
        }]);
        let out = redact_mcp(&raw).to_string();
        assert!(!out.contains("segredo") && !out.contains("token=abc"), "{out}");
        assert!(out.contains("https://x/sse") && out.contains("\"command\":\"node\""));
    }
}
