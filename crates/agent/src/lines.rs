//! Leitura de linhas com teto: o stdout de um agente é entrada de fora, e
//! `lines()` do tokio cresce sem limite numa linha sem `\n`. Aqui a linha
//! grande demais é descartada (até o próximo `\n`) e reportada, sem alocar
//! além do teto.

use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, BufReader};

/// Maior linha aceita de um agente (resultados de ferramenta com imagens
/// em base64 passam de alguns MiB; 64 MiB é folga sem ser perigoso).
pub const MAX_AGENT_LINE: usize = 64 << 20;

#[derive(Debug, PartialEq, Eq)]
pub enum Line {
    Text(String),
    /// A linha passou do teto; foi descartada (`n` bytes lidos até o corte).
    TooLong(usize),
}

pub struct BoundedLines<R> {
    inner: BufReader<R>,
    buf: Vec<u8>,
    max: usize,
}

impl<R: AsyncRead + Unpin> BoundedLines<R> {
    pub fn new(r: R, max: usize) -> Self {
        Self { inner: BufReader::new(r), buf: Vec::new(), max }
    }

    /// `Ok(None)` no fim do stream.
    pub async fn next(&mut self) -> std::io::Result<Option<Line>> {
        self.buf.clear();
        let n = (&mut self.inner).take(self.max as u64 + 1).read_until(b'\n', &mut self.buf).await?;
        if n == 0 {
            return Ok(None);
        }
        if self.buf.len() > self.max && self.buf.last() != Some(&b'\n') {
            let mut skipped = self.buf.len();
            self.buf = Vec::new();
            loop {
                let mut chunk = Vec::new();
                let m = (&mut self.inner).take(64 * 1024).read_until(b'\n', &mut chunk).await?;
                skipped += m;
                if m == 0 || chunk.last() == Some(&b'\n') {
                    break;
                }
            }
            return Ok(Some(Line::TooLong(skipped)));
        }
        while matches!(self.buf.last(), Some(b'\n' | b'\r')) {
            self.buf.pop();
        }
        Ok(Some(Line::Text(String::from_utf8_lossy(&self.buf).into_owned())))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn cuts_huge_lines_and_keeps_going() {
        let mut data = b"um\r\n".to_vec();
        data.extend(vec![b'x'; 100]);
        data.extend(b"\ndois\nsem-fim");
        let mut l = BoundedLines::new(data.as_slice(), 10);
        assert_eq!(l.next().await.unwrap(), Some(Line::Text("um".into())));
        assert!(matches!(l.next().await.unwrap(), Some(Line::TooLong(n)) if n >= 100));
        assert_eq!(l.next().await.unwrap(), Some(Line::Text("dois".into())));
        assert_eq!(l.next().await.unwrap(), Some(Line::Text("sem-fim".into())));
        assert_eq!(l.next().await.unwrap(), None);
    }
}
