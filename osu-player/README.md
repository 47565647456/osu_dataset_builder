# osu-player

A Bevy-powered `.osu` beatmap player with proper 2D rendering.

## Features

- **Playfield Rendering**: Displays circles, sliders (with body, caps, and ball), and spinners
- **Audio Sync**: Plays beatmap audio with synchronization via bevy_kira_audio
- **Timeline**: Interactive timeline with object density visualization and scrubbing
- **Slider Reverse Arrows**: Visual indicators for slider repeats
- **Countdown**: Shows 3-2-1-Go! countdown before first object
- **Break Periods**: Displays break indicator with progress bar
- **Combo Counter**: Shows current/total combo count
- **Map Stats**: Displays AR, CS, OD, HP, and BPM
- **FPS Graph**: Real-time frametime display with 1% low metrics

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

- **bevy** 0.17 - Game engine with 2D rendering
- **bevy_kira_audio** 0.24 - Audio playback
- **rosu-map** 0.2 - osu! beatmap parsing

## Building

```bash
cargo build --release
```

The compiled binary will be at `target/release/osu-player.exe` (Windows) or `target/release/osu-player` (Linux/macOS).

## Project Structure

```
src/
├── main.rs           # Entry point, Bevy app setup
├── beatmap.rs        # Beatmap parsing and data structures
├── audio.rs          # Audio playback with bevy_kira_audio
├── playback.rs       # Playback state management
├── input.rs          # Keyboard input handling
├── rendering/
│   ├── mod.rs        # Rendering module
│   ├── playfield.rs  # Playfield background and transforms
│   ├── circles.rs    # Hit circle rendering
│   ├── sliders.rs    # Slider rendering
│   └── spinners.rs   # Spinner rendering
└── ui/
    ├── mod.rs        # UI module
    ├── overlays.rs   # Countdown and break overlays
    ├── hud.rs        # Combo, stats, FPS display
    ├── timeline.rs   # Timeline with density minimap
    └── controls.rs   # Play/pause, speed controls
```
