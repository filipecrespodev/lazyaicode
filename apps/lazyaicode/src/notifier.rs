//! Os avisos do sistema no desktop, fora dos quadros da janela. No Wayland,
//! uma janela minimizada, coberta ou em outra área de trabalho não recebe
//! quadros — e era dentro deles que o aviso de "terminou" saía: justo
//! quando você não estava olhando, nada avisava. Aqui um observador assina
//! os eventos do motor e decide sozinho.
//!
//! "Você está olhando" vem das janelas: a cada quadro, cada uma informa a
//! thread que mostra com foco ([`Looking`]). O aviso espera um instante e só
//! não sai se uma janela desenhou um quadro *depois* do evento mostrando a
//! thread — a janela visível desenha logo (o evento a acorda); a escondida não.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use lazyaicode_engine::Engine;
use lazyaicode_ui_egui::attention::Attention;
use serde_json::json;
use tokio::sync::broadcast;

/// Quanto o aviso espera pelo quadro de uma janela visível.
const GRACE: Duration = Duration::from_millis(400);

/// O informe de uma janela: a thread que mostra com foco, e quando.
type Report = (Option<String>, Instant);

/// O que cada janela mostra com foco, e quando informou (`janela → informe`).
#[derive(Clone, Default)]
pub struct Looking(Arc<Mutex<HashMap<u64, Report>>>);

impl Looking {
    pub fn report(&self, window: u64, thread: Option<&str>) {
        if let Ok(mut m) = self.0.lock() {
            m.insert(window, (thread.map(str::to_string), Instant::now()));
        }
    }

    pub fn forget(&self, window: u64) {
        if let Ok(mut m) = self.0.lock() {
            m.remove(&window);
        }
    }

    /// Alguma janela mostrou `thread` com foco num quadro depois de `since`.
    fn seen_since(&self, thread: &str, since: Instant) -> bool {
        self.0.lock().is_ok_and(|m| m.values().any(|(t, at)| t.as_deref() == Some(thread) && *at >= since))
    }
}

pub type Notify = Arc<dyn Fn(&str, &str) + Send + Sync>;

/// Liga o observador: roda no runtime do motor até ele fechar.
pub fn spawn(engine: &Engine, rt: &tokio::runtime::Handle, looking: Looking, notify: Notify) {
    let mut sub = engine.subscribe();
    let (engine, rt2) = (engine.clone(), rt.clone());
    rt.spawn(async move {
        let mut attn = Attention::default();
        loop {
            let e = match sub.recv().await {
                Ok(e) => e,
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => return,
            };
            let Some(n) = attn.on_event(&e.event, &e.payload) else { continue };
            let (engine, looking, notify, at) = (engine.clone(), looking.clone(), notify.clone(), Instant::now());
            rt2.spawn(async move {
                tokio::time::sleep(GRACE).await;
                if looking.seen_since(&n.thread, at) {
                    return;
                }
                let on = engine.run("settings.get", json!({})).await.ok().and_then(|v| v["settings"]["notify_on_finish"].as_bool());
                if on == Some(false) {
                    return;
                }
                let title = engine
                    .run("thread.get", json!({ "id": n.thread }))
                    .await
                    .ok()
                    .and_then(|t| t["title"].as_str().map(str::to_string))
                    .unwrap_or_else(|| lazyaicode_i18n::t("app.thread"));
                notify(&n.title(&title), &n.body);
            });
        }
    });
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;
    use lazyaicode_engine::EngineOpts;
    use lazyaicode_protocol::events;

    struct Fixture {
        rt: tokio::runtime::Runtime,
        engine: Engine,
        tid: String,
        looking: Looking,
        got: Arc<Mutex<Vec<(String, String)>>>,
        _tmp: tempfile::TempDir,
    }

    fn fixture() -> Fixture {
        let tmp = tempfile::tempdir().unwrap();
        let ws = tmp.path().join("ws");
        std::fs::create_dir_all(ws.join("app")).unwrap();
        let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
        let engine = {
            let _g = rt.enter();
            Engine::new(EngineOpts { data_dir: Some(tmp.path().join("data")), ephemeral: false })
        };
        let tid = rt.block_on(async {
            let w = engine.run("workspace.create", json!({"path": ws.to_string_lossy()})).await.unwrap();
            let p = engine.run("project.list", json!({"workspaceId": w["id"]})).await.unwrap();
            let t = engine.run("thread.create", json!({"workspaceId": w["id"], "projectId": p[0]["id"], "isolated": false})).await.unwrap();
            engine.run("thread.rename", json!({"id": t["id"], "title": "Checkout"})).await.unwrap();
            t["id"].as_str().unwrap().to_string()
        });
        let looking = Looking::default();
        let got: Arc<Mutex<Vec<(String, String)>>> = Arc::default();
        let g = got.clone();
        spawn(&engine, rt.handle(), looking.clone(), Arc::new(move |t: &str, b: &str| g.lock().unwrap().push((t.into(), b.into()))));
        Fixture { rt, engine, tid, looking, got, _tmp: tmp }
    }

    fn finish(f: &Fixture) {
        let msg = |m| json!({ "threadId": f.tid, "msg": m });
        f.engine.emit(events::AGENT_EVENT, msg(json!({"type":"assistant","message":{"content":[{"type":"text","text":"feito"}]}})));
        f.engine.emit(events::AGENT_EVENT, msg(json!({"type":"result","is_error":false})));
    }

    fn settle() {
        std::thread::sleep(GRACE + Duration::from_millis(600));
    }

    /// Nenhuma janela desenhou (todas escondidas): o aviso sai assim mesmo.
    #[test]
    fn notifies_without_any_window_frame() {
        let f = fixture();
        finish(&f);
        settle();
        assert_eq!(*f.got.lock().unwrap(), vec![("✓ Checkout".to_string(), "feito".to_string())]);
        f.rt.block_on(f.engine.shutdown());
    }

    /// A janela visível com a thread aberta desenhou depois do evento: sem aviso.
    /// Uma janela que informou antes do evento (e depois sumiu) não conta.
    #[test]
    fn a_window_looking_at_the_thread_silences_it_but_a_stale_report_does_not() {
        let f = fixture();
        f.looking.report(1, Some(&f.tid));
        std::thread::sleep(Duration::from_millis(20));
        finish(&f);
        settle();
        assert_eq!(f.got.lock().unwrap().len(), 1, "o informe antigo (janela escondida) não cala o aviso");

        finish(&f);
        std::thread::sleep(Duration::from_millis(50));
        f.looking.report(1, Some(&f.tid));
        settle();
        assert_eq!(f.got.lock().unwrap().len(), 1, "a janela olhando desenhou depois do evento");

        f.looking.forget(1);
        f.rt.block_on(f.engine.run("settings.notify-on-finish", json!({"value": false}))).unwrap();
        finish(&f);
        settle();
        assert_eq!(f.got.lock().unwrap().len(), 1, "avisos desligados nas configurações");
        f.rt.block_on(f.engine.shutdown());
    }
}
