use crate::gpu::{self, RenderParams};
use eframe::egui::{self, Color32, RichText, Vec2};

const CYAN: Color32 = Color32::from_rgb(116, 211, 226);
const MUTED: Color32 = Color32::from_rgb(131, 151, 166);

pub struct App {
    n: u32,
    l: u32,
    m: i32,
    count: u32,
    point_size: f32,
    brightness: f32,
    color_mode: u32,
    threshold: f32,
    yaw: f32,
    pitch: f32,
    zoom: f32,
    auto_rotate: bool,
    elapsed: f32,
    fps: f32,
    gpu_available: bool,
    mobile_controls: bool,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        configure_style(&cc.egui_ctx);
        let gpu_available = if let Some(state) = &cc.wgpu_render_state {
            gpu::setup(
                &state.device,
                &state.queue,
                state.target_format,
                &mut state.renderer.write().callback_resources,
            );
            true
        } else {
            false
        };
        Self {
            n: 2,
            l: 1,
            m: 0,
            count: 300_000,
            point_size: 2.0,
            brightness: 1.0,
            color_mode: 0,
            threshold: 0.01,
            yaw: 0.5,
            pitch: 0.25,
            zoom: 1.0,
            auto_rotate: true,
            elapsed: 0.0,
            fps: 60.0,
            gpu_available,
            mobile_controls: false,
        }
    }

    fn reset_camera(&mut self) {
        self.yaw = 0.5;
        self.pitch = 0.25;
        self.zoom = 1.0;
    }

    fn orbital_name(&self) -> String {
        let letter = match self.l {
            0 => "s",
            1 => "p",
            _ => "d",
        };
        format!("{}{letter}", self.n)
    }

    fn controls(&mut self, ui: &mut egui::Ui) {
        ui.add_space(6.0);
        ui.label(RichText::new("ORBITAL LAB").size(11.0).strong().color(CYAN));
        ui.add_space(3.0);
        ui.label(RichText::new("Wave field").size(25.0).strong());
        ui.label(
            RichText::new("HYDROGEN  /  PROBABILITY CLOUD")
                .size(9.0)
                .color(MUTED),
        );
        ui.add_space(16.0);
        section(ui, "01", "QUANTUM STATE");
        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            for (n, l, label) in [
                (1, 0, "1s"),
                (2, 0, "2s"),
                (2, 1, "2p"),
                (3, 0, "3s"),
                (3, 1, "3p"),
                (3, 2, "3d"),
            ] {
                if ui
                    .selectable_label(self.n == n && self.l == l, label)
                    .clicked()
                {
                    self.n = n;
                    self.l = l;
                    self.m = 0;
                }
            }
        });
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("n").color(CYAN));
            egui::ComboBox::from_id_salt("n")
                .selected_text(self.n.to_string())
                .width(46.0)
                .show_ui(ui, |ui| {
                    for n in 1..=3 {
                        ui.selectable_value(&mut self.n, n, n.to_string());
                    }
                });
            self.l = self.l.min(self.n - 1);
            ui.label(RichText::new("ℓ").color(CYAN));
            egui::ComboBox::from_id_salt("l")
                .selected_text(self.l.to_string())
                .width(46.0)
                .show_ui(ui, |ui| {
                    for l in 0..self.n {
                        ui.selectable_value(&mut self.l, l, l.to_string());
                    }
                });
            self.m = self.m.clamp(-(self.l as i32), self.l as i32);
            ui.label(RichText::new("m").color(CYAN));
            egui::ComboBox::from_id_salt("m")
                .selected_text(self.m.to_string())
                .width(46.0)
                .show_ui(ui, |ui| {
                    for m in -(self.l as i32)..=(self.l as i32) {
                        ui.selectable_value(&mut self.m, m, m.to_string());
                    }
                });
        });
        ui.add_space(17.0);
        section(ui, "02", "CLOUD");
        ui.add_space(5.0);
        ui.label(RichText::new("Particles").color(MUTED));
        ui.add(egui::Slider::new(&mut self.count, 100_000..=1_000_000).step_by(10_000.0));
        ui.label(RichText::new("Point size").color(MUTED));
        ui.add(egui::Slider::new(&mut self.point_size, 0.5..=5.0));
        ui.label(RichText::new("Brightness").color(MUTED));
        ui.add(egui::Slider::new(&mut self.brightness, 0.1..=3.0));
        ui.label(RichText::new("Density threshold").color(MUTED));
        ui.add(egui::Slider::new(&mut self.threshold, 0.0..=0.2).max_decimals(3));
        ui.add_space(15.0);
        section(ui, "03", "COLOR MAPPING");
        ui.add_space(7.0);
        ui.horizontal_wrapped(|ui| {
            ui.selectable_value(&mut self.color_mode, 0, "Phase");
            ui.selectable_value(&mut self.color_mode, 1, "Density");
            ui.selectable_value(&mut self.color_mode, 2, "Radius");
        });
        ui.add_space(17.0);
        section(ui, "04", "VIEW");
        ui.add_space(6.0);
        ui.checkbox(&mut self.auto_rotate, "Auto rotate");
        if ui.button("Reset camera").clicked() {
            self.reset_camera();
        }
        ui.add_space(15.0);
        ui.separator();
        ui.add_space(8.0);
        ui.label(
            RichText::new("DRAG TO ORBIT  ·  SCROLL / PINCH TO ZOOM")
                .size(9.0)
                .color(MUTED),
        );
        ui.add_space(8.0);
    }
}

fn section(ui: &mut egui::Ui, index: &str, title: &str) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(index).size(10.0).color(CYAN));
        ui.label(RichText::new(title).size(10.0).strong().color(MUTED));
    });
}

fn configure_style(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = Vec2::new(8.0, 7.0);
    style.spacing.button_padding = Vec2::new(10.0, 5.0);
    style.spacing.slider_width = 170.0;
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = Color32::from_rgb(10, 17, 26);
    visuals.window_fill = Color32::from_rgb(14, 24, 34);
    visuals.extreme_bg_color = Color32::from_rgb(6, 12, 20);
    visuals.override_text_color = Some(Color32::from_rgb(218, 231, 237));
    visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(16, 27, 38);
    visuals.widgets.inactive.bg_fill = Color32::from_rgb(24, 40, 52);
    visuals.widgets.hovered.bg_fill = Color32::from_rgb(39, 65, 77);
    visuals.widgets.active.bg_fill = Color32::from_rgb(45, 90, 102);
    visuals.selection.bg_fill = Color32::from_rgb(29, 87, 99);
    visuals.selection.stroke.color = CYAN;
    style.visuals = visuals;
    ctx.set_style(style);
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let dt = ctx.input(|i| i.unstable_dt).clamp(0.0, 0.1);
        self.elapsed += dt;
        if dt > 0.0 {
            self.fps += (1.0 / dt - self.fps) * (dt * 2.0).min(1.0);
        }
        if self.auto_rotate {
            self.yaw = (self.yaw + dt * 0.18).rem_euclid(std::f32::consts::TAU);
        }
        let mobile = ctx.screen_rect().width() < 700.0;
        if mobile {
            egui::TopBottomPanel::top("mobile_header").show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("WAVE FIELD").strong().color(CYAN));
                    ui.label(
                        RichText::new(format!("{}  ·  m = {}", self.orbital_name(), self.m))
                            .color(MUTED),
                    );
                    if ui
                        .button(if self.mobile_controls {
                            "Close"
                        } else {
                            "Controls"
                        })
                        .clicked()
                    {
                        self.mobile_controls = !self.mobile_controls;
                    }
                });
                if self.mobile_controls {
                    egui::ScrollArea::vertical()
                        .max_height((ctx.screen_rect().height() * 0.55).max(100.0))
                        .show(ui, |ui| self.controls(ui));
                }
            });
        } else {
            egui::SidePanel::left("controls")
                .exact_width(270.0)
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.add_space(4.0);
                            self.controls(ui);
                        });
                });
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(Color32::from_rgb(5, 10, 18)))
            .show(ctx, |ui| {
                let rect = ui.max_rect();
                let response = ui.allocate_rect(rect, egui::Sense::drag());
                if response.dragged() {
                    let delta = response.drag_delta();
                    self.yaw += delta.x * 0.008;
                    self.pitch = (self.pitch + delta.y * 0.008).clamp(-1.5, 1.5);
                }
                if response.hovered() || response.dragged() {
                    let (scroll, pinch) = ctx.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
                    self.zoom = (self.zoom * (scroll * 0.0015).exp() * pinch).clamp(0.05, 1.5);
                }
                if self.gpu_available {
                    ui.painter().add(gpu::callback(
                        rect,
                        RenderParams {
                            n: self.n,
                            l: self.l,
                            m: self.m,
                            count: self.count,
                            point_size: self.point_size,
                            brightness: self.brightness,
                            color_mode: self.color_mode,
                            threshold: self.threshold,
                            yaw: self.yaw,
                            pitch: self.pitch,
                            zoom: self.zoom,
                            time: self.elapsed,
                        },
                    ));
                } else {
                    ui.painter().text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "WebGPU is unavailable.\nUse a WebGPU-enabled browser and GPU.",
                        egui::FontId::proportional(16.0),
                        Color32::LIGHT_RED,
                    );
                }
                let painter = ui.painter();
                let top = rect.left_top() + Vec2::new(20.0, 18.0);
                painter.text(
                    top,
                    egui::Align2::LEFT_TOP,
                    "ELECTRON PROBABILITY DENSITY",
                    egui::FontId::proportional(10.0),
                    MUTED,
                );
                painter.text(
                    top + Vec2::new(0.0, 17.0),
                    egui::Align2::LEFT_TOP,
                    format!("{}  /  m = {}", self.orbital_name(), self.m),
                    egui::FontId::proportional(20.0),
                    Color32::from_rgb(218, 231, 237),
                );
                painter.text(
                    rect.left_bottom() + Vec2::new(20.0, -18.0),
                    egui::Align2::LEFT_BOTTOM,
                    format!(
                        "{:.0} FPS   ·   {:.2}M POINTS   ·   THRESHOLD {:.3}",
                        self.fps,
                        self.count as f32 / 1_000_000.0,
                        self.threshold
                    ),
                    egui::FontId::proportional(10.0),
                    MUTED,
                );
            });
        ctx.request_repaint();
    }
}
