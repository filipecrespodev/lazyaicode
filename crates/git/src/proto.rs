//! Wire format between the app and `lazyaicode-agent` (the backend running
//! inside a WSL distro): length-prefixed JSON frames over the agent's stdio.
//!
//! Repo commands reuse `RepoCommand` itself. Its reply channels can't travel,
//! so `Reply<T>` serializes as `null`; on the agent side, deserializing one
//! parks a waiter that turns the eventual result back into JSON.

use std::cell::RefCell;
use std::collections::HashMap;
use std::io::{self, Read, Write};

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;
use tokio::sync::oneshot;

use crate::error::GitCommandError;

/// Bumped on any incompatible change; the app reinstalls an agent that differs.
pub const PROTOCOL_VERSION: u32 = 1;

/// Largest frame we accept (diffs of huge files are the worst case).
const MAX_FRAME: usize = 512 * 1024 * 1024;

pub struct Reply<T>(oneshot::Sender<Result<T, GitCommandError>>);

impl<T> Reply<T> {
    pub fn channel() -> (Self, oneshot::Receiver<Result<T, GitCommandError>>) {
        let (tx, rx) = oneshot::channel();
        (Self(tx), rx)
    }

    pub fn send(self, result: Result<T, GitCommandError>) -> Result<(), Result<T, GitCommandError>> {
        self.0.send(result)
    }
}

/// Blocks until the command's reply arrives; returns it as JSON.
pub type Waiter = Box<dyn FnOnce() -> Value + Send>;

thread_local! {
    static WAITER: RefCell<Option<Waiter>> = const { RefCell::new(None) };
}

impl<T> Serialize for Reply<T> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_unit()
    }
}

impl<'de, T: Serialize + Send + 'static> Deserialize<'de> for Reply<T> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        <()>::deserialize(d)?;
        let (reply, rx) = Reply::<T>::channel();
        let waiter: Waiter = Box::new(move || {
            let result =
                rx.blocking_recv().unwrap_or_else(|_| Err(GitCommandError::Internal("o ator do repositório não respondeu".into())));
            serde_json::to_value(result).unwrap_or(Value::Null)
        });
        WAITER.with(|w| *w.borrow_mut() = Some(waiter));
        Ok(reply)
    }
}

/// Decodes a command received from the app, plus the waiter for its reply.
pub fn decode<C: serde::de::DeserializeOwned>(value: Value) -> Result<(C, Waiter), GitCommandError> {
    WAITER.with(|w| *w.borrow_mut() = None);
    let cmd = serde_json::from_value(value).map_err(|e| GitCommandError::Internal(format!("comando inválido: {e}")))?;
    let waiter = WAITER.with(|w| w.borrow_mut().take()).ok_or_else(|| GitCommandError::Internal("comando sem resposta".into()))?;
    Ok((cmd, waiter))
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "camelCase")]
pub enum ToAgent {
    Req {
        id: u64,
        op: String,
        args: Value,
        /// The app's `settings::SYNCED_KEYS`, so the agent sees live values.
        settings: HashMap<String, Value>,
    },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "camelCase")]
pub enum FromAgent {
    Hello { version: u32, home: String },
    Res { id: u64, result: Result<Value, GitCommandError> },
    Event { name: String, payload: Value },
}

pub fn write_frame(w: &mut impl Write, msg: &impl Serialize) -> io::Result<()> {
    let body = serde_json::to_vec(msg).map_err(io::Error::other)?;
    w.write_all(&(body.len() as u32).to_be_bytes())?;
    w.write_all(&body)?;
    w.flush()
}

/// `Ok(None)` on a clean EOF.
pub fn read_frame<T: serde::de::DeserializeOwned>(r: &mut impl Read) -> io::Result<Option<T>> {
    let mut len = [0u8; 4];
    match r.read_exact(&mut len) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e),
    }
    let len = u32::from_be_bytes(len) as usize;
    if len > MAX_FRAME {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "frame too large"));
    }
    // Cresce com o que chega, não com o que o cabeçalho promete: um
    // cabeçalho hostil de 512 MiB não aloca nada antes de mandar os dados.
    let mut body = Vec::with_capacity(len.min(1 << 20));
    r.take(len as u64).read_to_end(&mut body)?;
    if body.len() < len {
        return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "frame cut short"));
    }
    serde_json::from_slice(&body).map(Some).map_err(io::Error::other)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Serialize, Deserialize)]
    enum Cmd {
        Ping(String, Reply<usize>),
    }

    #[test]
    fn a_huge_header_without_data_fails_without_allocating_it() {
        let mut wire = ((MAX_FRAME - 1) as u32).to_be_bytes().to_vec();
        wire.extend(b"{}");
        let r: io::Result<Option<serde_json::Value>> = read_frame(&mut wire.as_slice());
        assert_eq!(r.err().map(|e| e.kind()), Some(io::ErrorKind::UnexpectedEof));
    }

    #[test]
    fn a_command_round_trips_and_its_reply_comes_back_as_json() {
        let (reply, _rx) = Reply::<usize>::channel();
        let wire = serde_json::to_value(Cmd::Ping("hi".into(), reply)).unwrap();
        assert_eq!(wire, serde_json::json!({ "Ping": ["hi", null] }));

        let (Cmd::Ping(arg, reply), waiter) = decode::<Cmd>(wire).unwrap();
        assert_eq!(arg, "hi");
        reply.send(Ok(2)).unwrap();
        assert_eq!(waiter(), serde_json::json!({ "Ok": 2 }));
    }

    #[test]
    fn frames_round_trip() {
        let mut buf = Vec::new();
        write_frame(&mut buf, &FromAgent::Hello { version: 1, home: "/h".into() }).unwrap();
        let got: FromAgent = read_frame(&mut buf.as_slice()).unwrap().unwrap();
        assert!(matches!(got, FromAgent::Hello { version: 1, .. }));
        assert!(read_frame::<FromAgent>(&mut [].as_slice()).unwrap().is_none());
    }
}
