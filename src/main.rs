use minifb::{Key, MouseButton, MouseMode, Window, WindowOptions};
use rayon::prelude::*;
use std::time::Instant;

const WIDTH: usize = 800;
const HEIGHT: usize = 600;

#[derive(Clone, Copy, Debug)]
struct View {
    center_x: f64,
    center_y: f64,
    scale: f64, // pixels per unit in complex plane
    max_iter: u32,
}

impl Default for View {
    fn default() -> Self {
        View {
            center_x: -0.75,
            center_y: 0.0,
            scale: 200.0,
            max_iter: 100,
        }
    }
}

struct App {
    window: Window,
    buffer: Vec<u32>,
    view: View,
    mouse_down: bool,
    last_mouse_pos: (f32, f32),
    needs_redraw: bool,
    is_dragging: bool,
    low_quality_buffer: Vec<u32>,
    high_quality_pending: bool,
}

impl App {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let mut window = Window::new(
            "Mandelbrot Explorer - Click/drag to pan, scroll to zoom, Q/A to adjust iterations",
            WIDTH,
            HEIGHT,
            WindowOptions::default(),
        )?;

        // Limit to max ~60 fps
        window.set_target_fps(60);

        Ok(App {
            window,
            buffer: vec![0; WIDTH * HEIGHT],
            view: View::default(),
            mouse_down: false,
            last_mouse_pos: (0.0, 0.0),
            needs_redraw: true,
            is_dragging: false,
            low_quality_buffer: vec![0; WIDTH * HEIGHT],
            high_quality_pending: false,
        })
    }

    fn handle_input(&mut self) {
        // Mouse input
        if let Some((x, y)) = self.window.get_mouse_pos(MouseMode::Clamp) {
            let current_pos = (x, y);

            if self.window.get_mouse_down(MouseButton::Left) {
                if !self.mouse_down {
                    self.mouse_down = true;
                    self.is_dragging = true;
                    self.last_mouse_pos = current_pos;
                } else {
                    // Pan based on mouse movement
                    let dx = current_pos.0 - self.last_mouse_pos.0;
                    let dy = current_pos.1 - self.last_mouse_pos.1;

                    self.view.center_x -= dx as f64 / self.view.scale;
                    self.view.center_y -= dy as f64 / self.view.scale;

                    self.last_mouse_pos = current_pos;
                    self.needs_redraw = true;
                }
            } else {
                if self.mouse_down {
                    // Just finished dragging - trigger high quality render
                    self.high_quality_pending = true;
                }
                self.mouse_down = false;
                self.is_dragging = false;
            }
        }

        // Scroll wheel for zooming
        if let Some(scroll) = self.window.get_scroll_wheel() {
            let zoom_factor = if scroll.1 > 0.0 { 1.2 } else { 1.0 / 1.2 };
            self.view.scale *= zoom_factor;
            self.needs_redraw = true;
            self.high_quality_pending = true; // Trigger high quality after zoom
        }

        // Keyboard input
        self.window.get_keys().iter().for_each(|key| match key {
            Key::Q => {
                self.view.max_iter = (self.view.max_iter + 10).min(2000);
                self.needs_redraw = true;
            }
            Key::A => {
                self.view.max_iter = (self.view.max_iter.saturating_sub(10)).max(10);
                self.needs_redraw = true;
            }
            Key::R => {
                self.view = View::default();
                self.needs_redraw = true;
            }
            Key::Space => {
                println!(
                    "Current view: center=({:.6}, {:.6}), scale={:.2}, iters={}",
                    self.view.center_x, self.view.center_y, self.view.scale, self.view.max_iter
                );
            }
            _ => {}
        });
    }

    fn render_mandelbrot_fast(&mut self) {
        // Fast, low-quality render for real-time interaction
        let start = Instant::now();
        let skip = 2; // Always use 2x2 blocks, never skip more
        let adaptive_iters = self.calculate_adaptive_iterations();
        let reduced_iters = (adaptive_iters * 3 / 4).max(80); // Keep 75% of iterations, minimum 80

        // Create a temporary buffer for parallel work
        let mut temp_buffer = vec![[0u32; 3]; (HEIGHT / skip + 1) * (WIDTH / skip + 1)];

        temp_buffer
            .par_iter_mut()
            .enumerate()
            .for_each(|(i, pixel_data)| {
                let block_x = (i % (WIDTH / skip + 1)) * skip;
                let block_y = (i / (WIDTH / skip + 1)) * skip;

                if block_y < HEIGHT && block_x < WIDTH {
                    let y0 = self.view.center_y
                        + (block_y as f64 - HEIGHT as f64 / 2.0) / self.view.scale;
                    let x0 = self.view.center_x
                        + (block_x as f64 - WIDTH as f64 / 2.0) / self.view.scale;

                    let val = mandelbrot_smooth(x0, y0, reduced_iters);
                    let [r, g, b] = colorize(val, reduced_iters);
                    let color = ((r as u32) << 16) | ((g as u32) << 8) | (b as u32);

                    pixel_data[0] = color;
                    pixel_data[1] = block_x as u32;
                    pixel_data[2] = block_y as u32;
                }
            });

        // Copy to main buffer
        for pixel_data in temp_buffer {
            let color = pixel_data[0];
            let block_x = pixel_data[1] as usize;
            let block_y = pixel_data[2] as usize;

            if block_y < HEIGHT && block_x < WIDTH {
                for dy in 0..skip.min(HEIGHT - block_y) {
                    for dx in 0..skip.min(WIDTH - block_x) {
                        let idx = (block_y + dy) * WIDTH + block_x + dx;
                        if idx < self.low_quality_buffer.len() {
                            self.low_quality_buffer[idx] = color;
                        }
                    }
                }
            }
        }

        let elapsed = start.elapsed();
        self.window.set_title(&format!(
            "Mandelbrot Explorer - {:.1}ms (FAST), {}x zoom, {} iters - Dragging/zooming",
            elapsed.as_millis(),
            (self.view.scale / 200.0).round() as i32,
            self.view.max_iter
        ));
    }

    fn render_mandelbrot_high_quality(&mut self) {
        let start = Instant::now();
        let adaptive_iters = self.calculate_adaptive_iterations();

        self.buffer
            .par_chunks_mut(WIDTH)
            .enumerate()
            .for_each(|(y, row)| {
                let y0 = self.view.center_y + (y as f64 - HEIGHT as f64 / 2.0) / self.view.scale;

                for x in 0..WIDTH {
                    let x0 = self.view.center_x + (x as f64 - WIDTH as f64 / 2.0) / self.view.scale;

                    let val = mandelbrot_smooth(x0, y0, adaptive_iters);
                    let [r, g, b] = colorize(val, adaptive_iters);

                    row[x] = ((r as u32) << 16) | ((g as u32) << 8) | (b as u32);
                }
            });

        let elapsed = start.elapsed();
        self.window.set_title(&format!(
            "Mandelbrot Explorer - {:.1}ms (HQ), {}x zoom, {} iters (auto: {}) - Click/drag=pan, scroll=zoom, Q/A=base_iters, R=reset",
            elapsed.as_millis(),
            (self.view.scale / 200.0).round() as i32,
            self.view.max_iter,
            adaptive_iters
        ));
    }

    fn calculate_adaptive_iterations(&self) -> u32 {
        // Base iterations from user setting
        let base_iters = self.view.max_iter;

        // Calculate zoom level (how much deeper than default we are)
        let default_scale = 200.0;
        let zoom_factor = self.view.scale / default_scale;

        // Add iterations based on zoom depth
        // Each 10x zoom adds roughly log2(10) ≈ 3.3 bits of precision needed
        let zoom_bonus = if zoom_factor > 1.0 {
            let log_zoom = zoom_factor.log10();
            // Add ~50 iterations per order of magnitude zoom
            (log_zoom * 50.0) as u32
        } else {
            0
        };

        // Cap at reasonable maximum to prevent infinite render times
        (base_iters + zoom_bonus).min(5000)
    }

    fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Initial render to have something on screen
        self.render_mandelbrot_high_quality();
        self.needs_redraw = false;

        while self.window.is_open() && !self.window.is_key_down(Key::Escape) {
            self.handle_input();

            if self.needs_redraw {
                if self.is_dragging {
                    // Use fast rendering while dragging
                    self.render_mandelbrot_fast();
                    self.window
                        .update_with_buffer(&self.low_quality_buffer, WIDTH, HEIGHT)?;
                } else {
                    // Use high quality immediately for non-drag interactions
                    self.render_mandelbrot_high_quality();
                    self.window
                        .update_with_buffer(&self.buffer, WIDTH, HEIGHT)?;
                }
                self.needs_redraw = false;
                self.high_quality_pending = false;
            } else if self.high_quality_pending {
                // Render high quality when done dragging
                self.render_mandelbrot_high_quality();
                self.window
                    .update_with_buffer(&self.buffer, WIDTH, HEIGHT)?;
                self.high_quality_pending = false;
            } else {
                // Just update the display with current buffer
                let buffer_to_show = if self.is_dragging {
                    &self.low_quality_buffer
                } else {
                    &self.buffer
                };
                self.window
                    .update_with_buffer(buffer_to_show, WIDTH, HEIGHT)?;
            }
        }
        Ok(())
    }
}

fn in_main_cardioid_or_bulb(x: f64, y: f64) -> bool {
    // Period-2 bulb
    let dx = x + 1.0;
    if dx * dx + y * y < 0.0625 {
        return true;
    }
    // Main cardioid
    let x_minus_quarter = x - 0.25;
    let q = x_minus_quarter * x_minus_quarter + y * y;
    q * (q + x_minus_quarter) < 0.25 * y * y
}

fn mandelbrot_smooth(c_re: f64, c_im: f64, max_iter: u32) -> f64 {
    if in_main_cardioid_or_bulb(c_re, c_im) {
        return max_iter as f64;
    }

    let mut zr = 0.0;
    let mut zi = 0.0;
    let mut it = 0u32;

    while zr * zr + zi * zi <= 4.0 && it < max_iter {
        let zr_new = zr * zr - zi * zi + c_re;
        zi = 2.0 * zr * zi + c_im;
        zr = zr_new;
        it += 1;
    }

    if it >= max_iter {
        max_iter as f64
    } else {
        let mag = (zr * zr + zi * zi).sqrt();
        (it as f64) + 1.0 - (mag.ln() / std::f64::consts::LN_2).ln() / std::f64::consts::LN_2
    }
}

fn colorize(val: f64, max_iter: u32) -> [u8; 3] {
    if val >= max_iter as f64 {
        return [0, 0, 0]; // inside set
    }

    let t = (val / max_iter as f64).fract();

    // Enhanced color palette for better interactive experience
    let r = (9.0 * (1.0 - t) * t * t * t * 255.0) as u8;
    let g = (15.0 * (1.0 - t) * (1.0 - t) * t * t * 255.0) as u8;
    let b = (8.5 * (1.0 - t) * (1.0 - t) * (1.0 - t) * t * 255.0) as u8;
    [r, g, b]
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut app = App::new()?;
    app.run()
}
