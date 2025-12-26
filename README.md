# osu! Dataset Pipeline

A Rust toolkit for building comprehensive parquet datasets from osu! beatmaps.

## Pipeline Overview

```
.osz files  →  osz-extractor  →  osu-dataset-builder  →  osu-enricher  →  Parquet Dataset
```

| Step | Tool | Purpose |
|------|------|---------|
| 1 | **osz-extractor** | Extract .osz archives (audio, .osu, images) |
| 2 | **osu-dataset-builder** | Parse .osu files into 12 parquet tables |
| 3 | **osu-enricher** | Add API metadata, PP calculations, comments |

## Quick Start

```powershell
# 1. Extract .osz archives (auto-downloads from nerinyan if corrupt)
cargo run --release --manifest-path osz-extractor/Cargo.toml

# 2. Build parquet dataset from .osu files  
cargo run --release --manifest-path osu-dataset-builder/Cargo.toml

# 3. Enrich with osu! API data and difficulty calculations
cargo run --release --manifest-path osu-enricher/Cargo.toml
```

## Directories

| Path | Purpose |
|------|---------|
| `osu_archives/` | Input .osz files |
| `osz_extracted/` | Extracted beatmap folders |
| `dataset/` | Output parquet files |

## Output Files

### Core (osu-dataset-builder)
- `beatmaps.parquet` - Beatmap metadata
- `hit_objects.parquet` - Circles, sliders, spinners
- `timing_points.parquet` - BPM and timing
- `slider_*.parquet` - Slider details
- `storyboard_*.parquet` - Storyboard data
- `breaks.parquet`, `combo_colors.parquet`, `hit_samples.parquet`

### Enriched (osu-enricher)
- `beatmap_enriched.parquet` - API metadata + PP calculations (58 columns)
- `beatmap_comments.parquet` - Beatmapset comments (16 columns)

## Configuration

### osu-enricher
Requires `osu_credentials.txt` with:
```
<client_id>
<client_secret>
```
Get credentials from https://osu.ppy.sh/home/account/edit#oauth

### Custom Paths
```powershell
osu-enricher.exe --dataset-dir E:\dataset --source-dir E:\extracted --credentials E:\creds.txt
```

## Schema

See [SCHEMA.md](SCHEMA.md) for complete parquet schema documentation.
