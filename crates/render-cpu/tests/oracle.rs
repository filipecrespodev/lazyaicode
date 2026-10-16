//! O compositor em CPU contra o da GPU: a mesma tela do egui renderizada
//! pelo wgpu (via `egui_kittest`) e por este crate deve dar a mesma imagem,
//! a menos de pequenas diferenças de filtragem e arredondamento.
//!
//! Sem adaptador gráfico (CI sem GPU), o lado wgpu não roda e o teste só
//! confere o lado CPU.

use egui::{Color32, Pos2, Rect, Stroke, Vec2};

const SIZE: Vec2 = Vec2::new(480.0, 320.0);

fn scene(ui: &mut egui::Ui) {
    egui::CentralPanel::default().show(ui, |ui| {
        ui.heading("LazyAICode");
        ui.label("O compositor em CPU é o oráculo do wgpu.");
        let _ = ui.button("Botão");
        ui.horizontal(|ui| {
            let _ = ui.selectable_label(true, "selecionado");
            let _ = ui.checkbox(&mut true.clone(), "marcado");
        });
        let p = ui.painter();
        p.rect_filled(Rect::from_min_size(Pos2::new(300.0, 30.0), Vec2::new(120.0, 60.0)), 8.0, Color32::from_rgb(63, 180, 122));
        p.circle_filled(Pos2::new(360.0, 180.0), 40.0, Color32::from_rgba_unmultiplied(90, 167, 240, 200));
        p.line_segment([Pos2::new(20.0, 280.0), Pos2::new(460.0, 220.0)], Stroke::new(3.0, Color32::from_rgb(236, 106, 106)));
    });
}

/// Os pixels pelo caminho da CPU: um `Context` próprio com a mesma entrada.
fn cpu_image() -> image::RgbaImage {
    let ctx = egui::Context::default();
    let mut tex = lazyaicode_render_cpu::Textures::new();
    let mut prims = Vec::new();
    for i in 0..4 {
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SIZE)),
            time: Some(f64::from(i) * 0.1),
            ..Default::default()
        };
        // o mesmo quadro que o harness do kittest põe em volta da UI
        let mut out = ctx.run_ui(input, |ui| {
            egui::Frame::central_panel(ui.style()).outer_margin(8.0).inner_margin(0.0).show(ui, scene);
        });
        tex.apply(&out.textures_delta);
        out.textures_delta.clear();
        prims = ctx.tessellate(out.shapes, 1.0);
    }
    let pm = lazyaicode_render_cpu::render(&prims, &tex, [SIZE.x as u32, SIZE.y as u32], 1.0, Color32::BLACK).expect("render");
    // pré-multiplicado → direto (a imagem é opaca)
    image::RgbaImage::from_raw(pm.width(), pm.height(), pm.data().to_vec()).expect("imagem")
}

#[test]
fn cpu_renders_the_scene() {
    let img = cpu_image();
    assert_eq!(img.dimensions(), (480, 320));
    // o retângulo verde está onde foi desenhado
    let p = img.get_pixel(370, 70);
    assert!(p[1] > 150 && p[0] < 100, "{p:?}");
    // há texto (pixels claros) na área do título
    let bright = (10..200).flat_map(|x| (8..40).map(move |y| (x, y))).filter(|&(x, y)| img.get_pixel(x, y)[0] > 120).count();
    assert!(bright > 50, "{bright}");
}

#[test]
fn cpu_matches_wgpu() {
    let mut harness = egui_kittest::Harness::builder().with_size(SIZE).wgpu().build_ui(scene);
    harness.run();
    let gpu = match harness.render() {
        Ok(img) => img,
        Err(e) => {
            eprintln!("sem GPU, só o lado CPU foi conferido: {e}");
            return;
        }
    };
    let cpu = cpu_image();
    assert_eq!(gpu.dimensions(), cpu.dimensions());
    let (mut sum, mut far) = (0u64, 0u64);
    for (a, b) in gpu.pixels().zip(cpu.pixels()) {
        let d = (0..3).map(|k| i32::from(a[k]).abs_diff(i32::from(b[k]))).max().unwrap_or(0);
        sum += u64::from(d);
        if d > 48 {
            far += 1;
        }
    }
    let n = u64::from(gpu.width() * gpu.height());
    let mean = sum as f64 / n as f64;
    let far_pct = far as f64 * 100.0 / n as f64;
    if std::env::var_os("ORACLE_DUMP").is_some() {
        gpu.save("oracle-gpu.png").ok();
        cpu.save("oracle-cpu.png").ok();
    }
    assert!(mean < 0.5, "diferença média {mean:.2} (ORACLE_DUMP=1 grava as duas imagens)");
    assert!(far_pct < 0.1, "{far_pct:.2}% dos pixels muito diferentes");
}
