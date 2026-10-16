//! Aparência: os temas (paletas), a densidade, a escala e as fontes. Todas
//! as cores da UI saem de [`Tokens`]; nenhuma tela usa cor fixa.

use egui::{Color32, CornerRadius, FontFamily, FontId, Stroke, TextStyle, Visuals};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Dark,
    Light,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Palette {
    pub bg: Color32,
    pub panel: Color32,
    pub elev: Color32,
    pub hover: Color32,
    pub active: Color32,
    pub input: Color32,
    pub border: Color32,
    pub border_strong: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub faint: Color32,
    pub accent: Color32,
    pub blue: Color32,
    pub amber: Color32,
    pub red: Color32,
}

pub struct ThemeDef {
    pub id: &'static str,
    /// Chave de tradução ou nome próprio.
    pub label: &'static str,
    pub mode: Mode,
    pub hex: [&'static str; 15],
}

/// As paletas do app (as mesmas da versão anterior, em Svelte).
pub const THEMES: &[ThemeDef] = &[
    ThemeDef {
        id: "dark",
        label: "appearance.themeDark",
        mode: Mode::Dark,
        hex: [
            "#16171b", "#1a1c20", "#1d1f24", "#25282e", "#2c3038", "#121317", "#2b2e35", "#3a3e47", "#dcdfe4", "#8b919c", "#5f6570",
            "#3fb47a", "#5aa7f0", "#e5b04a", "#ec6a6a",
        ],
    },
    ThemeDef {
        id: "tokyo",
        label: "Tokyo Night",
        mode: Mode::Dark,
        hex: [
            "#1a1b26", "#16161e", "#1f2335", "#24283b", "#2f354f", "#16161e", "#2a2e42", "#3b4261", "#c0caf5", "#a9b1d6", "#565f89",
            "#9ece6a", "#7aa2f7", "#e0af68", "#f7768e",
        ],
    },
    ThemeDef {
        id: "dracula",
        label: "Dracula",
        mode: Mode::Dark,
        hex: [
            "#282a36", "#21222c", "#2e303e", "#343746", "#44475a", "#21222c", "#3a3c4e", "#4d5068", "#f8f8f2", "#c4c6d4", "#6272a4",
            "#50fa7b", "#8be9fd", "#f1fa8c", "#ff5555",
        ],
    },
    ThemeDef {
        id: "one-dark",
        label: "One Dark",
        mode: Mode::Dark,
        hex: [
            "#282c34", "#21252b", "#2c313a", "#2c313a", "#3a3f4b", "#1e2227", "#333842", "#3e4451", "#abb2bf", "#9da5b4", "#5c6370",
            "#98c379", "#61afef", "#e5c07b", "#e06c75",
        ],
    },
    ThemeDef {
        id: "nord",
        label: "Nord",
        mode: Mode::Dark,
        hex: [
            "#2e3440", "#292e39", "#3b4252", "#3b4252", "#434c5e", "#272c36", "#3b4252", "#4c566a", "#eceff4", "#d8dee9", "#7b88a1",
            "#a3be8c", "#88c0d0", "#ebcb8b", "#bf616a",
        ],
    },
    ThemeDef {
        id: "gruvbox",
        label: "Gruvbox Dark",
        mode: Mode::Dark,
        hex: [
            "#282828", "#1d2021", "#32302f", "#3c3836", "#504945", "#1d2021", "#3c3836", "#504945", "#ebdbb2", "#d5c4a1", "#928374",
            "#b8bb26", "#83a598", "#fabd2f", "#fb4934",
        ],
    },
    ThemeDef {
        id: "solarized",
        label: "Solarized Dark",
        mode: Mode::Dark,
        hex: [
            "#002b36", "#00252f", "#073642", "#073642", "#0d4452", "#00212b", "#0a3a47", "#24525e", "#93a1a1", "#839496", "#586e75",
            "#859900", "#268bd2", "#b58900", "#dc322f",
        ],
    },
    ThemeDef {
        id: "catppuccin",
        label: "Catppuccin Mocha",
        mode: Mode::Dark,
        hex: [
            "#1e1e2e", "#181825", "#232334", "#2a2b3c", "#313244", "#181825", "#313244", "#45475a", "#cdd6f4", "#bac2de", "#6c7086",
            "#a6e3a1", "#89b4fa", "#f9e2af", "#f38ba8",
        ],
    },
    ThemeDef {
        id: "rose-pine",
        label: "Rosé Pine",
        mode: Mode::Dark,
        hex: [
            "#191724", "#1f1d2e", "#26233a", "#26233a", "#393552", "#16141f", "#26233a", "#403d52", "#e0def4", "#908caa", "#6e6a86",
            "#9ccfd8", "#c4a7e7", "#f6c177", "#eb6f92",
        ],
    },
    ThemeDef {
        id: "light",
        label: "appearance.themeLight",
        mode: Mode::Light,
        hex: [
            "#fbfbfc", "#f4f5f7", "#ffffff", "#eceef1", "#e3e6ea", "#ffffff", "#e1e4e8", "#cfd4da", "#1f2328", "#5d6570", "#8d949e",
            "#1f9a5b", "#0969da", "#9a6700", "#cf222e",
        ],
    },
    ThemeDef {
        id: "catppuccin-latte",
        label: "Catppuccin Latte",
        mode: Mode::Light,
        hex: [
            "#eff1f5", "#e6e9ef", "#f6f7fa", "#dce0e8", "#ccd0da", "#ffffff", "#ccd0da", "#bcc0cc", "#4c4f69", "#5c5f77", "#8c8fa1",
            "#40a02b", "#1e66f5", "#df8e1d", "#d20f39",
        ],
    },
    ThemeDef {
        id: "rose-pine-dawn",
        label: "Rosé Pine Dawn",
        mode: Mode::Light,
        hex: [
            "#faf4ed", "#f2e9e1", "#fffaf3", "#f2e9e1", "#e4dcd5", "#fffaf3", "#dfdad9", "#cecacd", "#575279", "#797593", "#9893a5",
            "#56949f", "#907aa9", "#ea9d34", "#b4637a",
        ],
    },
    ThemeDef {
        id: "solarized-light",
        label: "Solarized Light",
        mode: Mode::Light,
        hex: [
            "#fdf6e3", "#eee8d5", "#fffcf0", "#eee8d5", "#e4ddc8", "#fffcf0", "#e4ddc8", "#d3cbb4", "#586e75", "#657b83", "#93a1a1",
            "#859900", "#268bd2", "#b58900", "#dc322f",
        ],
    },
];

/// `#rrggbb` → cor; texto inválido vira magenta (visível, nunca pânico).
pub fn hex(s: &str) -> Color32 {
    let h = s.trim_start_matches('#');
    let byte = |i: usize| h.get(i..i + 2).and_then(|x| u8::from_str_radix(x, 16).ok());
    match (byte(0), byte(2), byte(4)) {
        (Some(r), Some(g), Some(b)) if h.len() == 6 => Color32::from_rgb(r, g, b),
        _ => Color32::from_rgb(255, 0, 255),
    }
}

impl ThemeDef {
    pub fn palette(&self) -> Palette {
        let c = |i: usize| self.hex.get(i).map_or(Color32::from_rgb(255, 0, 255), |h| hex(h));
        Palette {
            bg: c(0),
            panel: c(1),
            elev: c(2),
            hover: c(3),
            active: c(4),
            input: c(5),
            border: c(6),
            border_strong: c(7),
            text: c(8),
            muted: c(9),
            faint: c(10),
            accent: c(11),
            blue: c(12),
            amber: c(13),
            red: c(14),
        }
    }
}

pub fn theme_by_id(id: &str) -> Option<&'static ThemeDef> {
    THEMES.iter().find(|t| t.id == id)
}

/// "system" (ou um id desconhecido) vira o escuro ou o claro padrão.
pub fn resolve(id: &str, system_dark: bool) -> &'static ThemeDef {
    match theme_by_id(id) {
        Some(t) if id != "system" => t,
        _ => theme_by_id(if system_dark { "dark" } else { "light" }).unwrap_or(&THEMES[0]),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Density {
    Compact,
    Normal,
    Comfortable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ContentWidth {
    Narrow,
    Normal,
    Wide,
    Full,
}

impl ContentWidth {
    pub fn px(self) -> f32 {
        match self {
            Self::Narrow => 680.0,
            Self::Normal => 820.0,
            Self::Wide => 1040.0,
            Self::Full => f32::INFINITY,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Appearance {
    /// id de um tema, ou "system"
    pub theme: String,
    pub density: Density,
    /// escala da interface, em %
    pub scale: u32,
    pub content_width: ContentWidth,
    pub text_size: f32,
    pub code_size: f32,
    pub terminal_size: f32,
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            theme: "system".into(),
            density: Density::Normal,
            scale: 100,
            content_width: ContentWidth::Normal,
            text_size: 13.0,
            code_size: 12.0,
            terminal_size: 13.0,
        }
    }
}

/// Cores prontas para desenhar.
#[derive(Debug, Clone, Copy)]
pub struct Tokens {
    pub mode: Mode,
    pub p: Palette,
    pub on_accent: Color32,
    pub add_bg: Color32,
    pub add_fg: Color32,
    pub del_bg: Color32,
    pub del_fg: Color32,
    pub accent_soft: Color32,
    pub row_h: f32,
    pub radius: u8,
    pub radius_sm: u8,
    pub text_size: f32,
    pub code_size: f32,
    pub terminal_size: f32,
    pub content_w: f32,
}

fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let l = |x: u8, y: u8| (f32::from(x) * (1.0 - t) + f32::from(y) * t).round().clamp(0.0, 255.0) as u8;
    Color32::from_rgb(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()))
}

fn luminance(c: Color32) -> f32 {
    let ch = |v: u8| {
        let c = f32::from(v) / 255.0;
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * ch(c.r()) + 0.7152 * ch(c.g()) + 0.0722 * ch(c.b())
}

/// Contraste WCAG entre duas cores.
pub fn contrast(a: Color32, b: Color32) -> f32 {
    let (x, y) = (luminance(a), luminance(b));
    let (hi, lo) = if x > y { (x, y) } else { (y, x) };
    (hi + 0.05) / (lo + 0.05)
}

impl Tokens {
    pub fn new(def: &ThemeDef, a: &Appearance) -> Self {
        let p = def.palette();
        let dark = def.mode == Mode::Dark;
        let dark_text = if dark { p.bg } else { Color32::from_rgb(0x15, 0x17, 0x1b) };
        let on_accent = if contrast(p.accent, Color32::WHITE) >= contrast(p.accent, dark_text) { Color32::WHITE } else { dark_text };
        Self {
            mode: def.mode,
            p,
            on_accent,
            add_fg: if dark { mix(p.accent, Color32::WHITE, 0.25) } else { mix(p.accent, Color32::BLACK, 0.2) },
            del_fg: if dark { mix(p.red, Color32::WHITE, 0.25) } else { mix(p.red, Color32::BLACK, 0.15) },
            add_bg: mix(p.bg, p.accent, if dark { 0.14 } else { 0.11 }),
            del_bg: mix(p.bg, p.red, if dark { 0.14 } else { 0.09 }),
            accent_soft: mix(p.bg, p.accent, 0.14),
            row_h: match a.density {
                Density::Compact => 24.0,
                Density::Normal => 28.0,
                Density::Comfortable => 32.0,
            },
            radius: 8,
            radius_sm: 5,
            text_size: a.text_size.clamp(9.0, 24.0),
            code_size: a.code_size.clamp(9.0, 24.0),
            terminal_size: a.terminal_size.clamp(8.0, 28.0),
            content_w: a.content_width.px(),
        }
    }

    /// Estilo do egui inteiro a partir dos tokens.
    pub fn apply(&self, ctx: &egui::Context) {
        let p = self.p;
        let mut v = if self.mode == Mode::Dark { Visuals::dark() } else { Visuals::light() };
        v.panel_fill = p.bg;
        v.window_fill = p.elev;
        v.extreme_bg_color = p.input;
        v.faint_bg_color = p.panel;
        v.code_bg_color = p.input;
        v.override_text_color = Some(p.text);
        v.hyperlink_color = p.blue;
        v.warn_fg_color = p.amber;
        v.error_fg_color = p.red;
        v.selection.bg_fill = self.accent_soft;
        v.selection.stroke = Stroke::new(1.0, p.accent);
        v.window_stroke = Stroke::new(1.0, p.border);
        v.window_corner_radius = CornerRadius::same(self.radius);
        v.menu_corner_radius = CornerRadius::same(self.radius);
        v.window_shadow.color = Color32::from_black_alpha(if self.mode == Mode::Dark { 120 } else { 40 });
        v.popup_shadow.color = v.window_shadow.color;
        let w = &mut v.widgets;
        let r = CornerRadius::same(self.radius_sm);
        w.noninteractive.bg_fill = p.panel;
        w.noninteractive.weak_bg_fill = p.panel;
        w.noninteractive.bg_stroke = Stroke::new(1.0, p.border);
        w.noninteractive.fg_stroke = Stroke::new(1.0, p.text);
        w.noninteractive.corner_radius = r;
        w.inactive.bg_fill = p.elev;
        w.inactive.weak_bg_fill = p.elev;
        w.inactive.bg_stroke = Stroke::new(1.0, p.border);
        w.inactive.fg_stroke = Stroke::new(1.0, p.text);
        w.inactive.corner_radius = r;
        w.hovered.bg_fill = p.hover;
        w.hovered.weak_bg_fill = p.hover;
        w.hovered.bg_stroke = Stroke::new(1.0, p.border_strong);
        w.hovered.fg_stroke = Stroke::new(1.0, p.text);
        w.hovered.corner_radius = r;
        w.active.bg_fill = p.active;
        w.active.weak_bg_fill = p.active;
        w.active.bg_stroke = Stroke::new(1.0, p.accent);
        w.active.fg_stroke = Stroke::new(1.0, p.text);
        w.active.corner_radius = r;
        w.open = w.active;
        ctx.set_visuals(v);
        ctx.all_styles_mut(|s| {
            s.spacing.item_spacing = egui::vec2(6.0, 4.0);
            s.spacing.button_padding = egui::vec2(8.0, 3.0);
            s.spacing.interact_size.y = (self.row_h - 6.0).max(18.0);
            s.text_styles = [
                (TextStyle::Small, FontId::new(self.text_size - 2.0, FontFamily::Proportional)),
                (TextStyle::Body, FontId::new(self.text_size, FontFamily::Proportional)),
                (TextStyle::Button, FontId::new(self.text_size, FontFamily::Proportional)),
                (TextStyle::Heading, FontId::new(self.text_size + 4.0, FontFamily::Proportional)),
                (TextStyle::Monospace, FontId::new(self.code_size, FontFamily::Monospace)),
            ]
            .into();
        });
    }

    /// As 16 cores ANSI do terminal, derivadas do tema.
    pub fn ansi16(&self) -> [[u8; 3]; 16] {
        let p = self.p;
        let rgb = |c: Color32| [c.r(), c.g(), c.b()];
        let dark = self.mode == Mode::Dark;
        let black = if dark { mix(p.bg, Color32::BLACK, 0.3) } else { Color32::from_rgb(0x24, 0x29, 0x2e) };
        let white = if dark { p.text } else { mix(p.bg, Color32::BLACK, 0.15) };
        let magenta = mix(p.blue, p.red, 0.5);
        let cyan = mix(p.blue, p.accent, 0.5);
        let bright = |c: Color32| if dark { mix(c, Color32::WHITE, 0.2) } else { mix(c, Color32::BLACK, 0.15) };
        [
            rgb(black),
            rgb(p.red),
            rgb(p.accent),
            rgb(p.amber),
            rgb(p.blue),
            rgb(magenta),
            rgb(cyan),
            rgb(white),
            rgb(p.faint),
            rgb(bright(p.red)),
            rgb(bright(p.accent)),
            rgb(bright(p.amber)),
            rgb(bright(p.blue)),
            rgb(bright(magenta)),
            rgb(bright(cyan)),
            rgb(if dark { Color32::WHITE } else { Color32::BLACK }),
        ]
    }
}

/// Fontes: a padrão do egui, os ícones Phosphor e, no fim da fila, as
/// fontes do sistema que cobrem outros scripts (`extra`).
pub fn fonts(extra: Vec<(String, Vec<u8>, u32)>) -> egui::FontDefinitions {
    let mut f = egui::FontDefinitions::default();
    egui_phosphor::add_to_fonts(&mut f, egui_phosphor::Variant::Regular);
    for (name, bytes, index) in extra {
        let mut data = egui::FontData::from_owned(bytes);
        data.index = index;
        f.font_data.insert(name.clone(), data.into());
        for fam in [FontFamily::Proportional, FontFamily::Monospace] {
            f.families.entry(fam).or_default().push(name.clone());
        }
    }
    f
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_theme_parses_and_has_readable_text() {
        for t in THEMES {
            let p = t.palette();
            assert_ne!(p.bg, Color32::from_rgb(255, 0, 255), "{}", t.id);
            assert!(contrast(p.text, p.bg) >= 4.5, "{}: texto ilegível ({})", t.id, contrast(p.text, p.bg));
            let tk = Tokens::new(t, &Appearance::default());
            assert!(contrast(tk.on_accent, p.accent) >= 2.0, "{}: botão primário ilegível", t.id);
        }
    }

    #[test]
    fn system_follows_the_os() {
        assert_eq!(resolve("system", true).id, "dark");
        assert_eq!(resolve("system", false).id, "light");
        assert_eq!(resolve("nord", false).id, "nord");
        assert_eq!(resolve("inexistente", true).id, "dark");
        assert_eq!(hex("#zz0000"), Color32::from_rgb(255, 0, 255));
    }
}
