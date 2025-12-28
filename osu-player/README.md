# osu-player

A Bevy-powered `.osu` beatmap player with high-quality rendering using Signed Distance Fields (SDF).

## Features

- **SDF Rendering**: Circles, sliders, and spinners rendered with pixel-perfect Signed Distance Fields for smooth scaling.
- **MSDF Font Rendering**: Combo numbers use Multi-channel Signed Distance Fields (MSDF) for high-quality, anti-aliased digits at any zoom level.
- **Smooth Animations**: Full support for object fade-in and fade-out transitions.
- **Audio Sync**: Plays beatmap audio with synchronization via `bevy_kira_audio`.
- **Interactive Timeline**: Density-based minimap visualization with scrubbing support.
- **Map Stats**: Displays AR, CS, OD, HP, and BPM.
- **FPS Display**: Real-time frametime display with 1% low metrics.

## Usage

```bash
cargo run --release -- <path-to-osu-file>
```

### Controls

| Input | Action |
|-------|--------|
| **Space** | Play/Pause |
| **Mouse Wheel** | Zoom Playfield In/Out |
| **Left-Click Drag** | Pan Playfield (Viewing Area Only) |
| **F / "Focus" Button** | Reset Zoom and Pan to Center |
| **← / →** | Seek -5s / +5s |
| **↑ / ↓** | Playback Speed + / - |
| **Right-Click Speed** | Cycle Playback Speed in Reverse |
| **Home / End** | Go to Start / End of Map |

## Dependencies

- **bevy** 0.17 - Game engine
- **bevy_kira_audio** 0.24 - Audio playback
- **rosu-map** 0.2 - osu! beatmap parsing
- **serde / serde_json** - JSON metadata parsing for font atlases

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
├── audio.rs          # Audio playback logic
├── playback.rs       # Playback state management
├── rendering/
│   ├── mod.rs        # Rendering module setup
│   ├── playfield.rs  # Coordinate transforms, panning, and zoom
│   ├── sdf_render.rs # Core SDF mesh spawning system
│   ├── sdf_materials.rs # WGSL material definitions
│   └── ...           # Specialized object rendering
└── ui/
    ├── mod.rs        # UI module setup
    ├── controls.rs   # UI bars, buttons, and interaction logic
    └── ...           # Timeline, HUD, and overlays
```
