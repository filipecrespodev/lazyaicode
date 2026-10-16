//! O terminal integrado sem IO: os bytes do PTY entram em [`Term::feed`],
//! a UI desenha a grade de [`Term::row`] e as teclas viram sequências de
//! escape com [`key_bytes`]. O mesmo código roda no app desktop e no
//! navegador (lá os bytes chegam pelo WebSocket).

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

/// Linhas guardadas acima da tela.
pub const SCROLLBACK: usize = 5000;
/// Menor tela aceita. Com 1 linha o `vt100` 0.16 estoura um contador ao
/// quebrar uma linha com caractere largo (achado pelo teste de fuzz abaixo).
pub const MIN_ROWS: u16 = 2;
pub const MIN_COLS: u16 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
    Default,
    Idx(u8),
    Rgb(u8, u8, u8),
}

impl From<vt100::Color> for Color {
    fn from(c: vt100::Color) -> Self {
        match c {
            vt100::Color::Default => Self::Default,
            vt100::Color::Idx(i) => Self::Idx(i),
            vt100::Color::Rgb(r, g, b) => Self::Rgb(r, g, b),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cell {
    pub text: String,
    pub fg: Color,
    pub bg: Color,
    pub bold: bool,
    pub dim: bool,
    pub italic: bool,
    pub underline: bool,
    pub inverse: bool,
    pub wide: bool,
}

pub struct Term {
    parser: vt100::Parser,
    /// Recebeu bytes desde o último [`Term::take_dirty`].
    dirty: bool,
    /// O processo terminou.
    pub exited: bool,
}

impl Term {
    pub fn new(rows: u16, cols: u16) -> Self {
        Self { parser: vt100::Parser::new(rows.max(MIN_ROWS), cols.max(MIN_COLS), SCROLLBACK), dirty: true, exited: false }
    }

    pub fn feed(&mut self, bytes: &[u8]) {
        self.parser.process(bytes);
        self.dirty = true;
    }

    pub fn take_dirty(&mut self) -> bool {
        std::mem::take(&mut self.dirty)
    }

    pub fn size(&self) -> (u16, u16) {
        self.parser.screen().size()
    }

    pub fn resize(&mut self, rows: u16, cols: u16) {
        self.parser.screen_mut().set_size(rows.max(MIN_ROWS), cols.max(MIN_COLS));
        self.dirty = true;
    }

    /// Rola a vista `n` linhas para cima (negativo = para baixo).
    pub fn scroll(&mut self, n: i32) {
        let cur = self.parser.screen().scrollback() as i64;
        let next = (cur + i64::from(n)).clamp(0, SCROLLBACK as i64) as usize;
        self.parser.screen_mut().set_scrollback(next);
        self.dirty = true;
    }

    pub fn scrolled(&self) -> usize {
        self.parser.screen().scrollback()
    }

    pub fn cursor(&self) -> Option<(u16, u16)> {
        let s = self.parser.screen();
        (!s.hide_cursor() && s.scrollback() == 0).then(|| s.cursor_position())
    }

    pub fn application_cursor(&self) -> bool {
        self.parser.screen().application_cursor()
    }

    pub fn bracketed_paste(&self) -> bool {
        self.parser.screen().bracketed_paste()
    }

    pub fn alternate_screen(&self) -> bool {
        self.parser.screen().alternate_screen()
    }

    /// As células de uma linha da tela.
    pub fn row(&self, row: u16) -> Vec<Cell> {
        let s = self.parser.screen();
        let (_, cols) = s.size();
        (0..cols)
            .filter_map(|c| s.cell(row, c))
            .filter(|c| !c.is_wide_continuation())
            .map(|c| Cell {
                text: if c.has_contents() { c.contents().to_string() } else { " ".to_string() },
                fg: c.fgcolor().into(),
                bg: c.bgcolor().into(),
                bold: c.bold(),
                dim: c.dim(),
                italic: c.italic(),
                underline: c.underline(),
                inverse: c.inverse(),
                wide: c.is_wide(),
            })
            .collect()
    }

    /// O texto visível (testes, copiar tudo, o `ui.inspect`).
    pub fn text(&self) -> String {
        self.parser.screen().contents()
    }

    /// Texto entre duas posições (seleção com o mouse).
    pub fn text_between(&self, start: (u16, u16), end: (u16, u16)) -> String {
        let (a, b) = if start <= end { (start, end) } else { (end, start) };
        self.parser.screen().contents_between(a.0, a.1, b.0, b.1.saturating_add(1))
    }

    /// Texto colado, com os marcadores de bracketed paste se o programa pediu.
    pub fn paste_bytes(&self, text: &str) -> Vec<u8> {
        let clean = text.replace("\r\n", "\r").replace('\n', "\r");
        if self.bracketed_paste() {
            let mut v = b"\x1b[200~".to_vec();
            v.extend(clean.replace('\x1b', "").as_bytes());
            v.extend(b"\x1b[201~");
            v
        } else {
            clean.into_bytes()
        }
    }
}

/// Cor RGB de uma cor do terminal: as 16 primeiras vêm do tema; o resto é
/// a paleta xterm de 256 cores.
pub fn rgb(c: Color, ansi16: &[[u8; 3]; 16], default: [u8; 3]) -> [u8; 3] {
    match c {
        Color::Default => default,
        Color::Rgb(r, g, b) => [r, g, b],
        Color::Idx(i) if i < 16 => ansi16.get(usize::from(i)).copied().unwrap_or(default),
        Color::Idx(i) if i < 232 => {
            let n = i - 16;
            let level = |v: u8| if v == 0 { 0 } else { 55 + v * 40 };
            [level(n / 36), level((n / 6) % 6), level(n % 6)]
        }
        Color::Idx(i) => {
            let g = 8 + (i - 232) * 10;
            [g, g, g]
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Enter,
    Tab,
    Backspace,
    Escape,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
    Insert,
    Delete,
    F(u8),
    /// Letra com Ctrl (`Ctrl+C` = `Char('c')` com `ctrl`).
    Char(char),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Mods {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
}

/// Bytes que um xterm mandaria para a tecla. Texto comum (sem Ctrl) chega
/// pelo evento de texto, não por aqui.
pub fn key_bytes(key: Key, m: Mods, app_cursor: bool) -> Option<Vec<u8>> {
    let modifier = 1 + u8::from(m.shift) + 2 * u8::from(m.alt) + 4 * u8::from(m.ctrl);
    let csi = |final_: &str, code: Option<u8>| -> Vec<u8> {
        match (modifier, code) {
            (1, None) => format!("\x1b[{final_}").into_bytes(),
            (1, Some(c)) => format!("\x1b[{c}~").into_bytes(),
            (m, None) => format!("\x1b[1;{m}{final_}").into_bytes(),
            (m, Some(c)) => format!("\x1b[{c};{m}~").into_bytes(),
        }
    };
    let arrow = |c: char| -> Vec<u8> {
        if modifier == 1 {
            if app_cursor {
                format!("\x1bO{c}").into_bytes()
            } else {
                format!("\x1b[{c}").into_bytes()
            }
        } else {
            csi(&c.to_string(), None)
        }
    };
    let out = match key {
        Key::Enter => {
            if m.alt {
                b"\x1b\r".to_vec()
            } else {
                b"\r".to_vec()
            }
        }
        Key::Tab => {
            if m.shift {
                b"\x1b[Z".to_vec()
            } else {
                b"\t".to_vec()
            }
        }
        Key::Backspace => {
            if m.ctrl {
                vec![0x08]
            } else if m.alt {
                b"\x1b\x7f".to_vec()
            } else {
                vec![0x7f]
            }
        }
        Key::Escape => vec![0x1b],
        Key::Up => arrow('A'),
        Key::Down => arrow('B'),
        Key::Right => arrow('C'),
        Key::Left => arrow('D'),
        Key::Home => arrow('H'),
        Key::End => arrow('F'),
        Key::Insert => csi("~", Some(2)),
        Key::Delete => csi("~", Some(3)),
        Key::PageUp => csi("~", Some(5)),
        Key::PageDown => csi("~", Some(6)),
        Key::F(n @ 1..=4) => {
            let c = char::from(b'P' + (n - 1));
            if modifier == 1 {
                format!("\x1bO{c}").into_bytes()
            } else {
                csi(&c.to_string(), None)
            }
        }
        Key::F(n @ 5..=12) => {
            let code = [15u8, 17, 18, 19, 20, 21, 23, 24].get(usize::from(n - 5)).copied()?;
            csi("~", Some(code))
        }
        Key::F(_) => return None,
        Key::Char(c) if m.ctrl => {
            let b = match c.to_ascii_lowercase() {
                c @ 'a'..='z' => (c as u8) - b'a' + 1,
                '@' | ' ' | '2' => 0,
                '[' | '3' => 0x1b,
                '\\' | '4' => 0x1c,
                ']' | '5' => 0x1d,
                '^' | '6' => 0x1e,
                '_' | '-' | '7' => 0x1f,
                _ => return None,
            };
            if m.alt {
                vec![0x1b, b]
            } else {
                vec![b]
            }
        }
        Key::Char(c) if m.alt => {
            let mut v = vec![0x1b];
            let mut buf = [0u8; 4];
            v.extend(c.encode_utf8(&mut buf).as_bytes());
            v
        }
        Key::Char(_) => return None,
    };
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_text_colors_and_cursor() {
        let mut t = Term::new(4, 20);
        t.feed(b"ola \x1b[31mvermelho\x1b[0m\r\nlinha 2");
        assert!(t.text().starts_with("ola vermelho\nlinha 2"));
        let row = t.row(0);
        assert_eq!(row.get(4).map(|c| c.fg), Some(Color::Idx(1)));
        assert_eq!(row.first().map(|c| c.fg), Some(Color::Default));
        assert_eq!(t.cursor(), Some((1, 7)));
        assert!(t.take_dirty() && !t.take_dirty());
    }

    #[test]
    fn keys_become_xterm_sequences() {
        let none = Mods::default();
        let ctrl = Mods { ctrl: true, ..Mods::default() };
        assert_eq!(key_bytes(Key::Char('c'), ctrl, false), Some(vec![3]));
        assert_eq!(key_bytes(Key::Up, none, false), Some(b"\x1b[A".to_vec()));
        assert_eq!(key_bytes(Key::Up, none, true), Some(b"\x1bOA".to_vec()));
        assert_eq!(key_bytes(Key::Right, ctrl, false), Some(b"\x1b[1;5C".to_vec()));
        assert_eq!(key_bytes(Key::Delete, none, false), Some(b"\x1b[3~".to_vec()));
        assert_eq!(key_bytes(Key::F(5), none, false), Some(b"\x1b[15~".to_vec()));
        assert_eq!(key_bytes(Key::Tab, Mods { shift: true, ..none }, false), Some(b"\x1b[Z".to_vec()));
        assert_eq!(key_bytes(Key::Char('x'), none, false), None);
    }

    #[test]
    fn xterm_256_palette() {
        let base = [[0u8; 3]; 16];
        assert_eq!(rgb(Color::Idx(196), &base, [0; 3]), [255, 0, 0]);
        assert_eq!(rgb(Color::Idx(232), &base, [0; 3]), [8, 8, 8]);
        assert_eq!(rgb(Color::Idx(16), &base, [0; 3]), [0, 0, 0]);
    }

    #[test]
    fn paste_respects_bracketed_mode() {
        let mut t = Term::new(2, 10);
        assert_eq!(t.paste_bytes("a\nb"), b"a\rb".to_vec());
        t.feed(b"\x1b[?2004h");
        assert_eq!(t.paste_bytes("a"), b"\x1b[200~a\x1b[201~".to_vec());
    }

    proptest::proptest! {
        #[test]
        fn never_panics_on_arbitrary_bytes(bytes in proptest::collection::vec(proptest::num::u8::ANY, 0..2000), rows in 0u16..5, cols in 0u16..5) {
            let mut t = Term::new(rows, cols);
            t.feed(&bytes);
            t.resize(cols, rows);
            for r in 0..t.size().0 { let _ = t.row(r); }
            t.scroll(10_000);
            let _ = t.text_between((3, 3), (0, 0));
        }
    }
}
