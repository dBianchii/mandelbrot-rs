# Mandelbrot Explorer

An interactive, real-time Mandelbrot set visualizer built in Rust. Vibe-coded with Claude-4-Sonnet.

![Mandelbrot Set Visualization](mandelbrot.png)

## Features

- **Real-time interaction**: Smooth panning and zooming with adaptive quality rendering
- **Dual-quality rendering**: Fast low-res preview while dragging, high-quality when still
- **Adaptive iterations**: Automatically increases iteration count based on zoom level for better detail
- **Performance optimized**: Parallel computation using Rayon for fast rendering
- **Smooth coloring**: Anti-aliased fractal boundaries using escape-time smoothing

## Controls

- **Mouse drag**: Pan around the fractal
- **Scroll wheel**: Zoom in/out
- **Q**: Increase base iteration count (+10)
- **A**: Decrease base iteration count (-10)
- **R**: Reset to default view
- **Space**: Print current view coordinates to console
- **Escape**: Exit

## Usage

```bash
cargo run
```

## Technical Details

- **Window size**: 800x600 pixels
- **Default view**: Centered at (-0.75, 0.0) with 200px/unit scale
- **Iteration range**: 10-5000 (auto-adjusted based on zoom)
- **Optimization**: Cardioid and period-2 bulb detection for instant computation
- **Color palette**: Enhanced gradient for better visual distinction

The renderer uses a clever dual-buffer system: while dragging, it renders at reduced quality (2x2 pixel blocks) for smooth 60fps interaction, then automatically switches to full resolution when you stop moving.

## Dependencies

- `minifb`: Cross-platform windowing and pixel buffer display
- `rayon`: Data parallelism for fast multi-threaded rendering

## Building

Requires Rust 2024 edition. Clone and run:

```bash
git clone <repo-url>
cd mandelbrot-rs
cargo run --release
```

Use `--release` for optimal performance.
