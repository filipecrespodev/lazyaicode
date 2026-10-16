//! Os pedidos `ui.*` do canal de controle: o que um agente vê da tela
//! (`ui.inspect`) e a captura (`ui.screenshot`).

use serde_json::{json, Value};

use crate::app::LazyApp;
use crate::commands::DEFERRED;

pub fn inspect(app: &LazyApp) -> Value {
    let thread = app.selected().map(|t| {
        let tr = app.data.transcripts.get(&t.thread.id);
        json!({
            "id": t.thread.id,
            "title": t.thread.title,
            "status": t.status,
            "provider": t.thread.provider,
            "model": t.thread.model,
            "isolated": t.thread.isolated,
            "cwd": t.thread.cwd,
            "items": tr.map_or(0, |tr| tr.items.len()),
            "pendingPermission": tr.and_then(|tr| tr.pending_permission()).is_some(),
            "draft": app.data.drafts.get(&t.thread.id),
            "attachments": app.data.attachments.get(&t.thread.id).map_or(0, Vec::len),
        })
    });
    let terminal = crate::terminal::current(app).filter(|_| app.state.tab == crate::state::MainTab::Terminal).map(|t| t.term.text());
    json!({
        "state": app.state,
        "workspace": app.data.active().map(|w| json!({ "id": w.id, "name": w.name, "path": w.path })),
        "thread": thread,
        "project": app.current_project().map(|p| json!({ "id": p.id, "name": p.name, "path": p.path, "isGit": p.is_git })),
        "counts": {
            "workspaces": app.data.workspaces.len(),
            "projects": app.data.projects.len(),
            "threads": app.data.threads.len(),
            "commands": app.data.commands.len(),
        },
        "toasts": app.toasts.iter().map(|t| t.text.clone()).collect::<Vec<_>>(),
        "terminal": terminal,
        "connected": app.data.connected,
        "connection": app.client.describe(),
        "platform": app.host.platform(),
        "locale": lazyaicode_i18n::locale(),
        "theme": crate::theme::resolve(&app.state.appearance.theme, app.system_dark).id,
    })
}

pub fn screenshot(app: &mut LazyApp, p: &Value) -> Result<Value, String> {
    app.screenshot_cpu = p.get("cpu").and_then(Value::as_bool).unwrap_or(false);
    Ok(Value::String(DEFERRED.into()))
}
