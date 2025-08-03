use eframe::egui;
use rayon::prelude::*;
use std::time::Instant;

const RENDER_WIDTH: usize = 800;
const RENDER_HEIGHT: usize = 600;

#[derive(Clone, Copy, Debug)]
struct MandelbrotParams {
    center_x: f64,
    center_y: f64,
    zoom: f64,
    max_iter: u32,
    escape_radius: f64,
    color_offset: f64,
    color_scale: f64,
    julia_mode: bool,
    julia_c_real: f64,
    julia_c_imag: f64,
}

impl Default for MandelbrotParams {
    fn default() -> Self {
        Self {
            center_x: -0.75,
            center_y: 0.0,
            zoom: 200.0,
            max_iter: 100,
            escape_radius: 2.0,
            color_offset: 0.0,
            color_scale: 1.0,
            julia_mode: false,
            julia_c_real: -0.7,
            julia_c_imag: 0.27015,
        }
    }
}

struct MandelbrotApp {
    params: MandelbrotParams,
    buffer: Vec<u32>,
    texture: Option<egui::TextureHandle>,
    needs_redraw: bool,
    auto_zoom: bool,
    zoom_speed: f64,
    animation_time: f64,
    is_dragging: bool,
    drag_accumulator: egui::Vec2,
    last_render_time: f64,
}

impl Default for MandelbrotApp {
    fn default() -> Self {
        Self {
            params: MandelbrotParams::default(),
            buffer: vec![0; RENDER_WIDTH * RENDER_HEIGHT],
            texture: None,
            needs_redraw: true,
            auto_zoom: false,
            zoom_speed: 1.02,
            animation_time: 0.0,
            is_dragging: false,
            drag_accumulator: egui::Vec2::ZERO,
            last_render_time: 0.0,
        }
    }
}

impl eframe::App for MandelbrotApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Handle keyboard input
        self.handle_keyboard_input(ctx);

        // Auto-zoom animation
        if self.auto_zoom {
            self.params.zoom *= self.zoom_speed;
            self.animation_time += 0.016; // ~60fps
            self.needs_redraw = true;
        }

        // Side panel with controls
        egui::SidePanel::left("controls").show(ctx, |ui| {
            ui.heading("Mandelbrot Explorer");

            // Performance info at the top
            ui.label(format!("Render time: {:.1}ms", self.last_render_time));

            ui.separator();
            ui.label("🎯 View Controls");

            if ui
                .add(egui::Slider::new(&mut self.params.center_x, -2.0..=1.0).text("Center X"))
                .changed()
            {
                self.needs_redraw = true;
            }

            if ui
                .add(egui::Slider::new(&mut self.params.center_y, -1.5..=1.5).text("Center Y"))
                .changed()
            {
                self.needs_redraw = true;
            }

            if ui
                .add(
                    egui::Slider::new(&mut self.params.zoom, 50.0..=10000.0)
                        .logarithmic(true)
                        .text("Zoom"),
                )
                .changed()
            {
                self.needs_redraw = true;
            }

            ui.separator();
            ui.label("⚙️ Computation");

            if ui
                .add(egui::Slider::new(&mut self.params.max_iter, 10..=1000).text("Max Iterations"))
                .changed()
            {
                self.needs_redraw = true;
            }

            if ui
                .add(
                    egui::Slider::new(&mut self.params.escape_radius, 1.5..=10.0)
                        .text("Escape Radius"),
                )
                .changed()
            {
                self.needs_redraw = true;
            }

            ui.separator();
            ui.label("🎨 Colors");

            if ui
                .add(
                    egui::Slider::new(&mut self.params.color_offset, 0.0..=1.0)
                        .text("Color Offset"),
                )
                .changed()
            {
                self.needs_redraw = true;
            }

            if ui
                .add(egui::Slider::new(&mut self.params.color_scale, 0.1..=5.0).text("Color Scale"))
                .changed()
            {
                self.needs_redraw = true;
            }

            ui.separator();
            ui.label("🔄 Julia Set Mode");

            if ui
                .checkbox(&mut self.params.julia_mode, "Enable Julia Set")
                .changed()
            {
                self.needs_redraw = true;
            }

            if self.params.julia_mode {
                if ui
                    .add(
                        egui::Slider::new(&mut self.params.julia_c_real, -2.0..=2.0)
                            .text("Julia C (Real)"),
                    )
                    .changed()
                {
                    self.needs_redraw = true;
                }

                if ui
                    .add(
                        egui::Slider::new(&mut self.params.julia_c_imag, -2.0..=2.0)
                            .text("Julia C (Imaginary)"),
                    )
                    .changed()
                {
                    self.needs_redraw = true;
                }
            }

            ui.separator();
            ui.label("🎬 Animation");

            ui.checkbox(&mut self.auto_zoom, "Auto Zoom");

            if self.auto_zoom {
                ui.add(egui::Slider::new(&mut self.zoom_speed, 1.001..=1.1).text("Zoom Speed"));
            }

            ui.separator();

            if ui.button("📸 Reset View").clicked() {
                self.params = MandelbrotParams::default();
                self.needs_redraw = true;
            }

            ui.separator();
            ui.label("⌨️ Keyboard Controls");
            ui.label("Q/A: Iterations ±10");
            ui.label("R: Reset view");
            ui.label("Space: Print coords");
            ui.label("🖱️ Mouse drag: Pan");
            ui.label("🖱️ Scroll: Zoom");
            ui.label("🖱️ Click: Zoom to point");

            ui.separator();
            ui.label(format!("Zoom: {:.0}x", self.params.zoom / 200.0));
            ui.label(format!(
                "Center: ({:.4}, {:.4})",
                self.params.center_x, self.params.center_y
            ));
        });

        // Main render area
        egui::CentralPanel::default().show(ctx, |ui| {
            if self.needs_redraw {
                let start = Instant::now();
                self.render_fractal();
                let elapsed = start.elapsed();

                // Update texture
                let color_image = egui::ColorImage::from_rgba_unmultiplied(
                    [RENDER_WIDTH, RENDER_HEIGHT],
                    &self.buffer_to_rgba(),
                );

                if let Some(texture) = &mut self.texture {
                    texture.set(color_image, egui::TextureOptions::NEAREST);
                } else {
                    self.texture = Some(ui.ctx().load_texture(
                        "mandelbrot",
                        color_image,
                        egui::TextureOptions::NEAREST,
                    ));
                }

                self.needs_redraw = false;
                self.last_render_time = elapsed.as_millis() as f64;
            }

            // Display the fractal
            if let Some(texture) = &self.texture {
                let max_size = ui.available_size();
                let aspect_ratio = RENDER_WIDTH as f32 / RENDER_HEIGHT as f32;

                let size = if max_size.x / aspect_ratio < max_size.y {
                    egui::vec2(max_size.x, max_size.x / aspect_ratio)
                } else {
                    egui::vec2(max_size.y * aspect_ratio, max_size.y)
                };

                let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click_and_drag());
                ui.put(rect, egui::Image::new((texture.id(), size)));

                // Handle mouse interaction
                self.handle_mouse_interaction(&response, rect, size);
            }
        });

        // Request repaint for smooth animation
        if self.auto_zoom {
            ctx.request_repaint();
        }
    }
}

impl MandelbrotApp {
    fn handle_keyboard_input(&mut self, ctx: &egui::Context) {
        // Q: Increase iterations
        if ctx.input(|i| i.key_pressed(egui::Key::Q)) {
            self.params.max_iter = (self.params.max_iter + 10).min(5000);
            self.needs_redraw = true;
        }

        // A: Decrease iterations
        if ctx.input(|i| i.key_pressed(egui::Key::A)) {
            self.params.max_iter = (self.params.max_iter.saturating_sub(10)).max(10);
            self.needs_redraw = true;
        }

        // R: Reset view
        if ctx.input(|i| i.key_pressed(egui::Key::R)) {
            self.params = MandelbrotParams::default();
            self.needs_redraw = true;
        }

        // Space: Print coordinates
        if ctx.input(|i| i.key_pressed(egui::Key::Space)) {
            println!(
                "Current view - Center: ({:.6}, {:.6}), Zoom: {:.2}",
                self.params.center_x, self.params.center_y, self.params.zoom
            );
        }

        // Escape: Exit (handled by egui automatically)
    }

    fn handle_mouse_interaction(
        &mut self,
        response: &egui::Response,
        rect: egui::Rect,
        size: egui::Vec2,
    ) {
        // Handle dragging for panning with smoothing
        if response.drag_started() {
            self.is_dragging = true;
            self.drag_accumulator = egui::Vec2::ZERO;
        }

        if response.dragged() {
            self.drag_accumulator += response.drag_delta();

            // Only apply drag movement when accumulator is significant enough
            if self.drag_accumulator.length() > 2.0 {
                let scale_x = (RENDER_WIDTH as f64 / self.params.zoom) / size.x as f64;
                let scale_y = (RENDER_HEIGHT as f64 / self.params.zoom) / size.y as f64;

                self.params.center_x -= self.drag_accumulator.x as f64 * scale_x;
                self.params.center_y -= self.drag_accumulator.y as f64 * scale_y;
                self.needs_redraw = true;
                self.drag_accumulator = egui::Vec2::ZERO;
            }
        }

        if response.drag_stopped() {
            // Apply any remaining drag movement
            if self.drag_accumulator.length() > 0.1 {
                let scale_x = (RENDER_WIDTH as f64 / self.params.zoom) / size.x as f64;
                let scale_y = (RENDER_HEIGHT as f64 / self.params.zoom) / size.y as f64;

                self.params.center_x -= self.drag_accumulator.x as f64 * scale_x;
                self.params.center_y -= self.drag_accumulator.y as f64 * scale_y;
                self.needs_redraw = true;
            }
            self.is_dragging = false;
            self.drag_accumulator = egui::Vec2::ZERO;
        }

        // Handle scroll wheel for zooming
        if response.hovered() {
            let scroll_delta = response.ctx.input(|i| i.smooth_scroll_delta.y);
            if scroll_delta != 0.0 {
                let zoom_factor = if scroll_delta > 0.0 { 1.1 } else { 1.0 / 1.1 };
                self.params.zoom *= zoom_factor;
                self.needs_redraw = true;
            }
        }

        // Handle click for zoom-to-point (original behavior)
        if response.clicked() {
            if let Some(pos) = response.interact_pointer_pos() {
                let relative_pos = pos - rect.min;
                let x_ratio = relative_pos.x / size.x;
                let y_ratio = relative_pos.y / size.y;

                // Convert to complex plane coordinates
                let new_x = self.params.center_x
                    + (x_ratio as f64 - 0.5) * (RENDER_WIDTH as f64 / self.params.zoom);
                let new_y = self.params.center_y
                    + (y_ratio as f64 - 0.5) * (RENDER_HEIGHT as f64 / self.params.zoom);

                self.params.center_x = new_x;
                self.params.center_y = new_y;
                self.params.zoom *= 2.0;
                self.needs_redraw = true;
            }
        }
    }

    fn render_fractal(&mut self) {
        let escape_radius_sq = self.params.escape_radius * self.params.escape_radius;

        let params = self.params; // Copy params to avoid borrowing issues
        self.buffer
            .par_iter_mut()
            .enumerate()
            .for_each(|(i, pixel)| {
                let x = i % RENDER_WIDTH;
                let y = i / RENDER_WIDTH;

                let real = params.center_x + (x as f64 - RENDER_WIDTH as f64 / 2.0) / params.zoom;
                let imag = params.center_y + (y as f64 - RENDER_HEIGHT as f64 / 2.0) / params.zoom;

                let iterations = if params.julia_mode {
                    julia_iterations(
                        real,
                        imag,
                        params.julia_c_real,
                        params.julia_c_imag,
                        params.max_iter,
                        escape_radius_sq,
                    )
                } else {
                    mandelbrot_iterations(real, imag, params.max_iter, escape_radius_sq)
                };

                *pixel = colorize_pixel(
                    iterations,
                    params.max_iter,
                    params.color_scale,
                    params.color_offset,
                );
            });
    }

    fn buffer_to_rgba(&self) -> Vec<u8> {
        let mut rgba = Vec::with_capacity(self.buffer.len() * 4);
        for &pixel in &self.buffer {
            rgba.push(((pixel >> 16) & 0xFF) as u8); // R
            rgba.push(((pixel >> 8) & 0xFF) as u8); // G
            rgba.push((pixel & 0xFF) as u8); // B
            rgba.push(255); // A
        }
        rgba
    }
}

fn colorize_pixel(iterations: f64, max_iter: u32, color_scale: f64, color_offset: f64) -> u32 {
    if iterations >= max_iter as f64 {
        return 0x000000; // Black for points in the set
    }

    let t = ((iterations / max_iter as f64) * color_scale + color_offset).fract();

    // Enhanced color palette
    let r = (9.0 * (1.0 - t) * t * t * t * 255.0) as u8;
    let g = (15.0 * (1.0 - t) * (1.0 - t) * t * t * 255.0) as u8;
    let b = (8.5 * (1.0 - t) * (1.0 - t) * (1.0 - t) * t * 255.0) as u8;

    ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
}

fn mandelbrot_iterations(c_real: f64, c_imag: f64, max_iter: u32, escape_radius_sq: f64) -> f64 {
    let mut zr = 0.0;
    let mut zi = 0.0;
    let mut iter = 0;

    while zr * zr + zi * zi <= escape_radius_sq && iter < max_iter {
        let zr_new = zr * zr - zi * zi + c_real;
        zi = 2.0 * zr * zi + c_imag;
        zr = zr_new;
        iter += 1;
    }

    if iter >= max_iter {
        max_iter as f64
    } else {
        // Smooth coloring
        let mag = (zr * zr + zi * zi).sqrt();
        iter as f64 + 1.0 - (mag.ln() / std::f64::consts::LN_2).ln() / std::f64::consts::LN_2
    }
}

fn julia_iterations(
    z_real: f64,
    z_imag: f64,
    c_real: f64,
    c_imag: f64,
    max_iter: u32,
    escape_radius_sq: f64,
) -> f64 {
    let mut zr = z_real;
    let mut zi = z_imag;
    let mut iter = 0;

    while zr * zr + zi * zi <= escape_radius_sq && iter < max_iter {
        let zr_new = zr * zr - zi * zi + c_real;
        zi = 2.0 * zr * zi + c_imag;
        zr = zr_new;
        iter += 1;
    }

    if iter >= max_iter {
        max_iter as f64
    } else {
        let mag = (zr * zr + zi * zi).sqrt();
        iter as f64 + 1.0 - (mag.ln() / std::f64::consts::LN_2).ln() / std::f64::consts::LN_2
    }
}

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_title("Mandelbrot & Julia Set Explorer"),
        ..Default::default()
    };

    eframe::run_native(
        "Mandelbrot Explorer",
        options,
        Box::new(|_cc| Ok(Box::new(MandelbrotApp::default()))),
    )
}
