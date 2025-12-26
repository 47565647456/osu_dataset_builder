# osu-reconstructor

Rust library to reconstruct osu! beatmap folders from parquet dataset exported by `osu-validator`.

## Installation

Add to your `Cargo.toml`:
```toml
[dependencies]
osu-reconstructor = { path = "../osu-reconstructor" }
```

## CLI Usage

```bash
# Reconstruct a single folder by ID
reconstruct --dataset E:\osu_model\dataset \
            --assets E:\osu_model\dataset\assets \
            --output E:\osu_model\reconstructed \
            --folder-id 100

# Reconstruct first N folders (for testing)
reconstruct --dataset E:\osu_model\dataset \
            --assets E:\osu_model\dataset\assets \
            --output E:\osu_model\reconstructed \
            --limit 10

# Reconstruct all folders
reconstruct --dataset E:\osu_model\dataset \
            --assets E:\osu_model\dataset\assets \
            --output E:\osu_model\reconstructed
```

### CLI Options

| Option | Description |
|--------|-------------|
| `-d, --dataset` | Path to dataset directory containing parquet files |
| `-a, --assets` | Path to assets directory |
| `-o, --output` | Output directory for reconstructed folders |
| `-f, --folder-id` | Specific folder ID to reconstruct (optional) |
| `--limit` | Limit number of folders to process (optional) |

## Library API

```rust
use osu_reconstructor::{ParquetReader, FolderReconstructor, Dataset};
use std::path::Path;

fn main() -> anyhow::Result<()> {
    // Load entire dataset from parquet files
    let reader = ParquetReader::new("E:/osu_model/dataset");
    let dataset: Dataset = reader.load_all()?;

    println!("Loaded {} beatmaps", dataset.beatmaps.len());

    // Reconstruct a specific folder
    let reconstructor = FolderReconstructor::new("E:/osu_model/dataset/assets");
    let output = Path::new("E:/osu_model/reconstructed");
    
    let result = reconstructor.reconstruct_folder("100", output, &dataset)?;
    
    println!("Created {} .osu files", result.osu_files.len());
    println!("Copied {} assets", result.assets_copied);

    Ok(())
}
```

### Key Types

| Type | Description |
|------|-------------|
| `ParquetReader` | Loads parquet files into `Dataset` |
| `Dataset` | Container for all row data |
| `BeatmapReconstructor` | Rebuilds `rosu_map::Beatmap` from rows |
| `StoryboardReconstructor` | Rebuilds storyboard elements |
| `FolderReconstructor` | Creates complete folder with `.osu`, `.osb`, and assets |

### Dataset Structure

The library reads these parquet files:
- `beatmaps.parquet` - Beatmap metadata
- `hit_objects.parquet` - All hit objects (circle, slider, spinner, hold)
- `timing_points.parquet` - Timing and difficulty points
- `storyboard_elements.parquet` - Sprites, animations, samples
- `storyboard_commands.parquet` - Storyboard command timelines
- `slider_control_points.parquet` - Slider path control points
- `slider_data.parquet` - Slider velocity, repeat count, expected distance

## Output Structure

Reconstructed folders contain:
```
{folder_id}/
├── {beatmap}.osu     # Reconstructed beatmap(s)
├── {storyboard}.osb  # Storyboard (if present)
├── audio.mp3         # Audio file
├── bg.jpg            # Background image
└── ...               # Other assets
```

## Dependencies

- `rosu-map` - Beatmap parsing and encoding
- `rosu-storyboard` - Storyboard parsing
- `arrow` / `parquet` - Parquet file reading
- `walkdir` - Directory traversal
- `clap` - CLI argument parsing
