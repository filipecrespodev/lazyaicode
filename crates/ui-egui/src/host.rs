//! Serviços da plataforma que a UI pede mas não implementa: escolher pasta,
//! ler imagem da área de transferência, notificar, capturar a tela. O app
//! desktop implementa com rfd/arboard; a web, com o que o navegador deixa.
//!
//! Respostas assíncronas (a pasta escolhida num diálogo) voltam como
//! mensagens na [`Inbox`], que a UI processa no próximo quadro — o mesmo
//! caminho dos pedidos `ui.*` do canal de controle.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use lazyaicode_protocol::ErrorBody;
use serde_json::Value;

/// Uma mensagem para a UI: um comando de tela e, se alguém espera, por onde
/// devolver o resultado.
pub struct UiMsg {
    pub command: String,
    pub params: Value,
    pub reply: Option<std::sync::mpsc::Sender<Result<Value, ErrorBody>>>,
}

/// Fila de mensagens para a UI, compartilhada entre threads.
#[derive(Clone, Default)]
pub struct Inbox(Arc<Mutex<VecDeque<UiMsg>>>);

impl Inbox {
    pub fn push(&self, command: &str, params: Value) {
        if let Ok(mut q) = self.0.lock() {
            q.push_back(UiMsg { command: command.into(), params, reply: None });
        }
    }

    pub fn push_msg(&self, m: UiMsg) {
        if let Ok(mut q) = self.0.lock() {
            q.push_back(m);
        }
    }

    pub fn drain(&self) -> Vec<UiMsg> {
        self.0.lock().map(|mut q| q.drain(..).collect()).unwrap_or_default()
    }
}

pub trait Host {
    /// Abre o seletor de pasta; a escolha volta como `then` na inbox, com
    /// `{"path": …}` somado a `params`.
    fn pick_folder(&self, title: &str, then: &str, params: Value);
    /// Abre o seletor de imagens; volta como `then` com `{"paths": [...]}`.
    fn pick_images(&self, then: &str, params: Value);
    /// Imagem na área de transferência (PNG), se houver.
    fn clipboard_image(&self) -> Option<Vec<u8>>;
    /// Aviso do sistema quando o agente termina com a janela sem foco.
    fn notify(&self, title: &str, body: &str);
    /// A janela tem a barra do sistema (senão a UI desenha os botões).
    fn native_decorations(&self) -> bool;
    fn is_web(&self) -> bool;
    /// Nome da plataforma para o `ui.inspect` (`linux`, `web`…).
    fn platform(&self) -> String;
    /// Agora, em ms desde 1970 (no wasm o `SystemTime` não existe).
    fn now_ms(&self) -> i64;
    /// Fuso local em ms (para "hoje"/"ontem" na barra lateral).
    fn tz_offset_ms(&self) -> i64 {
        0
    }
}

/// Início do dia local, em ms.
pub fn today_start(now_ms: i64, tz_offset_ms: i64) -> i64 {
    const DAY: i64 = 86_400_000;
    (now_ms + tz_offset_ms).div_euclid(DAY) * DAY - tz_offset_ms
}

/// Sem plataforma (testes, snapshots).
pub struct NoHost;

impl Host for NoHost {
    fn pick_folder(&self, _: &str, _: &str, _: Value) {}
    fn pick_images(&self, _: &str, _: Value) {}
    fn clipboard_image(&self) -> Option<Vec<u8>> {
        None
    }
    fn notify(&self, _: &str, _: &str) {}
    fn native_decorations(&self) -> bool {
        true
    }
    fn is_web(&self) -> bool {
        false
    }
    fn platform(&self) -> String {
        "test".into()
    }
    fn now_ms(&self) -> i64 {
        1_800_000_000_000
    }
}
