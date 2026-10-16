//! O renderizador de referência em CPU para a saída do egui.
//!
//! O app desenha na GPU (eframe + wgpu: Metal, Vulkan, DX12, GL ou WebGPU).
//! Este crate desenha a mesma lista de triângulos texturizados em CPU, sem
//! nenhuma dependência de placa de vídeo, e serve para:
//!
//! - **oráculo**: os testes renderizam a mesma tela pelos dois caminhos e
//!   comparam os pixels (veja `tests/oracle.rs`);
//! - **capturas sem GPU**: `ui.screenshot {"cpu": true}` no canal de
//!   controle e `lazyaicode --screenshot` em CI, servidores e contêineres.
//!
//! A imagem é dividida em tiles de 256×256 pixels, rasterizados em paralelo
//! (rayon, um tile por tarefa). Cada tile percorre só os triângulos cuja
//! caixa o toca, na ordem de pintura do egui, com a regra top-left para
//! arestas compartilhadas e mistura com alfa pré-multiplicado em espaço gama
//! (como o `egui-wgpu` faz num framebuffer não-sRGB). O resultado vira um
//! `tiny_skia::Pixmap`, que já sabe gravar PNG.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing))]

use std::collections::HashMap;

use epaint::textures::{TextureFilter, TexturesDelta};
use epaint::{ClippedPrimitive, Color32, ImageData, Primitive, TextureId};
use rayon::prelude::*;

/// Lado de um tile, em pixels.
pub const TILE: usize = 256;
/// Maior imagem aceita (lado), para não alocar o impossível.
pub const MAX_SIDE: u32 = 16_384;

struct Texture {
    w: usize,
    h: usize,
    px: Vec<Color32>,
    linear: bool,
}

/// As texturas do egui (atlas de fontes, imagens), mantidas entre quadros
/// como o renderizador de GPU mantém as dele.
#[derive(Default)]
pub struct Textures {
    map: HashMap<TextureId, Texture>,
}

impl Textures {
    pub fn new() -> Self {
        Self::default()
    }

    /// Aplica as mudanças de um quadro (`FullOutput::textures_delta`).
    pub fn apply(&mut self, delta: &TexturesDelta) {
        for (id, deltas) in &delta.set {
            for d in deltas {
                let ImageData::Color(img) = &d.image;
                let [w, h] = img.size;
                let linear = d.options.magnification == TextureFilter::Linear;
                match d.pos {
                    None => {
                        self.map.insert(*id, Texture { w, h, px: img.pixels.clone(), linear });
                    }
                    Some([x0, y0]) => {
                        let Some(t) = self.map.get_mut(id) else { continue };
                        for y in 0..h {
                            for x in 0..w {
                                let (dx, dy) = (x0 + x, y0 + y);
                                if dx < t.w && dy < t.h {
                                    if let (Some(dst), Some(src)) = (t.px.get_mut(dy * t.w + dx), img.pixels.get(y * w + x)) {
                                        *dst = *src;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        for id in &delta.free {
            self.map.remove(id);
        }
    }

    fn sample(&self, id: TextureId, u: f32, v: f32) -> [f32; 4] {
        let Some(t) = self.map.get(&id) else { return [1.0; 4] };
        if t.w == 0 || t.h == 0 {
            return [1.0; 4];
        }
        let fetch = |x: i64, y: i64| -> [f32; 4] {
            let x = x.clamp(0, t.w as i64 - 1) as usize;
            let y = y.clamp(0, t.h as i64 - 1) as usize;
            let c = t.px.get(y * t.w + x).copied().unwrap_or(Color32::TRANSPARENT);
            [f32::from(c.r()) / 255.0, f32::from(c.g()) / 255.0, f32::from(c.b()) / 255.0, f32::from(c.a()) / 255.0]
        };
        let (fx, fy) = (u * t.w as f32 - 0.5, v * t.h as f32 - 0.5);
        if !t.linear {
            return fetch((fx + 0.5).floor() as i64, (fy + 0.5).floor() as i64);
        }
        let (x0, y0) = (fx.floor(), fy.floor());
        let (tx, ty) = (fx - x0, fy - y0);
        let (x0, y0) = (x0 as i64, y0 as i64);
        let (a, b, c, d) = (fetch(x0, y0), fetch(x0 + 1, y0), fetch(x0, y0 + 1), fetch(x0 + 1, y0 + 1));
        let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;
        let mut out = [0.0; 4];
        for (o, (((a, b), c), d)) in out.iter_mut().zip(a.iter().zip(b).zip(c).zip(d)) {
            *o = lerp(lerp(*a, b, tx), lerp(c, d, tx), ty);
        }
        out
    }
}

/// Um triângulo pronto para rasterizar, em pixels.
struct Tri {
    p: [[f32; 2]; 3],
    uv: [[f32; 2]; 3],
    c: [[f32; 4]; 3],
    tex: TextureId,
    /// recorte em pixels: x0, y0, x1, y1
    clip: [f32; 4],
    bbox: [f32; 4],
}

fn rgba(c: Color32) -> [f32; 4] {
    [f32::from(c.r()) / 255.0, f32::from(c.g()) / 255.0, f32::from(c.b()) / 255.0, f32::from(c.a()) / 255.0]
}

fn collect(prims: &[ClippedPrimitive], ppp: f32) -> Vec<Tri> {
    let mut out = Vec::new();
    for cp in prims {
        let Primitive::Mesh(mesh) = &cp.primitive else { continue };
        let clip = [cp.clip_rect.min.x * ppp, cp.clip_rect.min.y * ppp, cp.clip_rect.max.x * ppp, cp.clip_rect.max.y * ppp];
        for &[i0, i1, i2] in mesh.indices.as_chunks::<3>().0 {
            let (Some(a), Some(b), Some(c)) =
                (mesh.vertices.get(i0 as usize), mesh.vertices.get(i1 as usize), mesh.vertices.get(i2 as usize))
            else {
                continue;
            };
            let p = [[a.pos.x * ppp, a.pos.y * ppp], [b.pos.x * ppp, b.pos.y * ppp], [c.pos.x * ppp, c.pos.y * ppp]];
            if p.iter().flatten().any(|v| !v.is_finite()) {
                continue;
            }
            let bbox = [
                p[0][0].min(p[1][0]).min(p[2][0]).max(clip[0]),
                p[0][1].min(p[1][1]).min(p[2][1]).max(clip[1]),
                p[0][0].max(p[1][0]).max(p[2][0]).min(clip[2]),
                p[0][1].max(p[1][1]).max(p[2][1]).min(clip[3]),
            ];
            if bbox[0] >= bbox[2] || bbox[1] >= bbox[3] {
                continue;
            }
            out.push(Tri {
                p,
                uv: [[a.uv.x, a.uv.y], [b.uv.x, b.uv.y], [c.uv.x, c.uv.y]],
                c: [rgba(a.color), rgba(b.color), rgba(c.color)],
                tex: mesh.texture_id,
                clip,
                bbox,
            });
        }
    }
    out
}

fn edge(a: [f32; 2], b: [f32; 2], p: [f32; 2]) -> f32 {
    (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0])
}

/// Aresta "top-left" (para a regra de preenchimento): com orientação
/// normalizada, a de cima (horizontal indo para a direita) ou uma da esquerda.
fn top_left(a: [f32; 2], b: [f32; 2]) -> bool {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    (dy == 0.0 && dx > 0.0) || dy < 0.0
}

/// Rasteriza um tile (`x0..x0+w`, `y0..y0+h`) em `buf` (RGBA f32, pré-multiplicado).
fn raster_tile(tris: &[Tri], tex: &Textures, x0: usize, y0: usize, w: usize, h: usize, buf: &mut [[f32; 4]]) {
    let (tx0, ty0, tx1, ty1) = (x0 as f32, y0 as f32, (x0 + w) as f32, (y0 + h) as f32);
    for t in tris {
        if t.bbox[2] <= tx0 || t.bbox[0] >= tx1 || t.bbox[3] <= ty0 || t.bbox[1] >= ty1 {
            continue;
        }
        let area = edge(t.p[0], t.p[1], t.p[2]);
        if area == 0.0 {
            continue;
        }
        // orientação horária no espaço de tela (y para baixo) para a regra top-left
        let [p0, p1, p2] = t.p;
        let [u0, u1, u2] = t.uv;
        let [c0, c1, c2] = t.c;
        let ((pa, pb, pc), (ua, ub, uc), (ca, cb, cc)) =
            if area < 0.0 { ((p0, p2, p1), (u0, u2, u1), (c0, c2, c1)) } else { ((p0, p1, p2), (u0, u1, u2), (c0, c1, c2)) };
        let area = edge(pa, pb, pc);
        let inv = 1.0 / area;
        let (bias0, bias1, bias2) = (top_left(pb, pc), top_left(pc, pa), top_left(pa, pb));
        let xs = (t.bbox[0].max(tx0).floor() as usize).max(x0);
        let ys = (t.bbox[1].max(ty0).floor() as usize).max(y0);
        let xe = (t.bbox[2].min(tx1).ceil() as usize).min(x0 + w);
        let ye = (t.bbox[3].min(ty1).ceil() as usize).min(y0 + h);
        let textured = tex.map.contains_key(&t.tex);
        for y in ys..ye {
            let py = y as f32 + 0.5;
            if py < t.clip[1] || py >= t.clip[3] {
                continue;
            }
            for x in xs..xe {
                let px = x as f32 + 0.5;
                if px < t.clip[0] || px >= t.clip[2] {
                    continue;
                }
                let p = [px, py];
                let w0 = edge(pb, pc, p);
                let w1 = edge(pc, pa, p);
                let w2 = edge(pa, pb, p);
                let inside = |w: f32, bias: bool| w > 0.0 || (w == 0.0 && bias);
                if !(inside(w0, bias0) && inside(w1, bias1) && inside(w2, bias2)) {
                    continue;
                }
                let (l0, l1, l2) = (w0 * inv, w1 * inv, w2 * inv);
                let mix = |a: f32, b: f32, c: f32| a * l0 + b * l1 + c * l2;
                let mut col = [mix(ca[0], cb[0], cc[0]), mix(ca[1], cb[1], cc[1]), mix(ca[2], cb[2], cc[2]), mix(ca[3], cb[3], cc[3])];
                if textured {
                    let s = tex.sample(t.tex, mix(ua[0], ub[0], uc[0]), mix(ua[1], ub[1], uc[1]));
                    for (cv, sv) in col.iter_mut().zip(s) {
                        *cv *= sv;
                    }
                }
                let Some(dst) = buf.get_mut((y - y0) * w + (x - x0)) else { continue };
                let ia = 1.0 - col[3].clamp(0.0, 1.0);
                for (d, cv) in dst.iter_mut().zip(col) {
                    *d = cv + *d * ia;
                }
            }
        }
    }
}

/// Desenha os primitivos num `Pixmap` de `size` pixels (`ppp` = pixels por
/// ponto), sobre o fundo `clear`.
pub fn render(prims: &[ClippedPrimitive], tex: &Textures, size: [u32; 2], ppp: f32, clear: Color32) -> Option<tiny_skia::Pixmap> {
    let [w, h] = size;
    if w == 0 || h == 0 || w > MAX_SIDE || h > MAX_SIDE || !ppp.is_finite() || ppp <= 0.0 {
        return None;
    }
    let (w, h) = (w as usize, h as usize);
    let tris = collect(prims, ppp);
    let cols = w.div_ceil(TILE);
    let rows = h.div_ceil(TILE);
    let bg = rgba(clear);
    let tiles: Vec<(usize, usize, Vec<[f32; 4]>)> = (0..cols * rows)
        .into_par_iter()
        .map(|i| {
            let (tx, ty) = (i % cols, i / cols);
            let (x0, y0) = (tx * TILE, ty * TILE);
            let (tw, th) = (TILE.min(w - x0), TILE.min(h - y0));
            let mut buf = vec![bg; tw * th];
            raster_tile(&tris, tex, x0, y0, tw, th, &mut buf);
            (x0, y0, buf)
        })
        .collect();
    let mut data = vec![0u8; w * h * 4];
    for (x0, y0, buf) in tiles {
        let tw = TILE.min(w - x0);
        for (j, px) in buf.iter().enumerate() {
            let (x, y) = (x0 + j % tw, y0 + j / tw);
            let o = (y * w + x) * 4;
            // pré-multiplicado: cada canal fica ≤ alfa
            let a = px[3].clamp(0.0, 1.0);
            if let Some(out) = data.get_mut(o..o + 4) {
                for (k, (d, v)) in out.iter_mut().zip(px).enumerate() {
                    let v = if k == 3 { a } else { v.clamp(0.0, a) };
                    *d = (v * 255.0).round() as u8;
                }
            }
        }
    }
    tiny_skia::Pixmap::from_vec(data, tiny_skia::IntSize::from_wh(w as u32, h as u32)?)
}

/// Atalho: renderiza e grava PNG.
pub fn render_png(prims: &[ClippedPrimitive], tex: &Textures, size: [u32; 2], ppp: f32, clear: Color32) -> Option<Vec<u8>> {
    render(prims, tex, size, ppp, clear)?.encode_png().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use epaint::{Mesh, Pos2, Rect};

    fn quad(color: Color32, r: Rect) -> ClippedPrimitive {
        let mut m = Mesh::default();
        m.add_colored_rect(r, color);
        ClippedPrimitive { clip_rect: Rect::EVERYTHING, primitive: Primitive::Mesh(m) }
    }

    fn px(p: &tiny_skia::Pixmap, x: u32, y: u32) -> [u8; 4] {
        let c = p.pixel(x, y).map(|c| [c.red(), c.green(), c.blue(), c.alpha()]);
        c.unwrap_or([0; 4])
    }

    #[test]
    fn fills_rects_across_tile_borders_without_seams_or_double_blend() {
        let half = Color32::from_rgba_premultiplied(0, 0, 128, 128);
        let prims = vec![quad(half, Rect::from_min_max(Pos2::new(10.0, 10.0), Pos2::new(600.0, 300.0)))];
        let p = render(&prims, &Textures::new(), [640, 320], 1.0, Color32::BLACK).unwrap();
        // dentro, inclusive sobre a fronteira dos tiles (x=256) e a diagonal da quad
        for (x, y) in [(20, 20), (255, 100), (256, 100), (500, 290), (305, 155)] {
            assert_eq!(px(&p, x, y), [0, 0, 128, 255], "({x},{y})");
        }
        assert_eq!(px(&p, 5, 5), [0, 0, 0, 255]);
        assert_eq!(px(&p, 600, 100), [0, 0, 0, 255], "borda direita exclusiva");
    }

    #[test]
    fn respects_clip_rects_and_scale() {
        let mut c = quad(Color32::WHITE, Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(100.0, 100.0)));
        c.clip_rect = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(50.0, 50.0));
        let p = render(&[c], &Textures::new(), [200, 200], 2.0, Color32::TRANSPARENT).unwrap();
        assert_eq!(px(&p, 99, 99), [255, 255, 255, 255]);
        assert_eq!(px(&p, 101, 50), [0, 0, 0, 0]);
    }

    #[test]
    fn rejects_absurd_sizes_and_garbage_geometry() {
        assert!(render(&[], &Textures::new(), [0, 10], 1.0, Color32::BLACK).is_none());
        assert!(render(&[], &Textures::new(), [MAX_SIDE + 1, 10], 1.0, Color32::BLACK).is_none());
        let mut m = Mesh::default();
        m.add_colored_rect(Rect::from_min_max(Pos2::new(f32::NAN, 0.0), Pos2::new(1e30, 1e30)), Color32::RED);
        m.indices.push(999);
        let prims = vec![ClippedPrimitive { clip_rect: Rect::EVERYTHING, primitive: Primitive::Mesh(m) }];
        assert!(render(&prims, &Textures::new(), [64, 64], 1.0, Color32::BLACK).is_some());
    }
}
