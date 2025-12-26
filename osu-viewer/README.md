# osu-viewer

A lightweight GPU-accelerated `.osu` beatmap file viewer built with Rust and egui.

## Features

- **Playfield Rendering**: Displays circles, sliders (with tessellated mesh bodies), and spinners
- **Audio Sync**: Plays beatmap audio with automatic time synchronization
- **Timeline**: Interactive timeline with object density visualization
- **Slider Reverse Arrows**: Visual indicators for slider repeats
- **Countdown**: Shows 3-2-1-Go! countdown before first object
- **Break Periods**: Displays break indicator with progress bar
- **Combo Counter**: Shows current/total combo count
- **FPS Graph**: Real-time frametime graph with 1% low metrics

## Usage

```bash
cargo run --release -- <path-to-osu-file>
```

### Controls

| Key | Action |
|-----|--------|
| Space | Play/Pause |
| ← / → | Seek -5s / +5s |
| ↑ / ↓ | Increase/Decrease playback speed |
| Home | Go to start |
| End | Go to end |

## Dependencies

- **egui/eframe**: UI and rendering
- **rosu-map**: osu! beatmap parsing
- **kira**: Audio playback

## Building

```bash
cargo build --release
```

The compiled binary will be at `target/release/osu-viewer.exe` (Windows) or `target/release/osu-viewer` (Linux/macOS).