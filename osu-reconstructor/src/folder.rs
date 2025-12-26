//! Folder reconstruction - combines beatmaps, storyboards, and assets

use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

use crate::beatmap::BeatmapReconstructor;
use crate::storyboard::StoryboardReconstructor;
use crate::types::*;

/// Reconstructor for complete beatmap folders
pub struct FolderReconstructor {
    assets_dir: std::path::PathBuf,
}

impl FolderReconstructor {
    /// Create a new folder reconstructor
    /// 
    /// # Arguments
    /// * `assets_dir` - Path to the assets directory (e.g., E:\osu_model\dataset\assets)
    pub fn new<P: AsRef<Path>>(assets_dir: P) -> Self {
        Self {
            assets_dir: assets_dir.as_ref().to_path_buf(),
        }
    }

    /// Reconstruct a complete folder for the given folder_id
    pub fn reconstruct_folder(
        &self,
        folder_id: &str,
        output_dir: &Path,
        dataset: &Dataset,
    ) -> Result<ReconstructedFolder> {
        // Create output folder
        let folder_output = output_dir.join(folder_id);
        fs::create_dir_all(&folder_output)
            .context(format!("Failed to create output folder: {}", folder_output.display()))?;

        let mut result = ReconstructedFolder {
            folder_id: folder_id.to_string(),
            output_path: folder_output.clone(),
            osu_files: Vec::new(),
            storyboard_elements: 0,
            assets_copied: 0,
        };

        // Get all beatmaps for this folder
        let beatmap_rows: Vec<_> = dataset.beatmaps
            .iter()
            .filter(|b| b.folder_id == folder_id)
            .collect();

        // Reconstruct each .osu file
        for beatmap_row in &beatmap_rows {
            let mut beatmap = BeatmapReconstructor::reconstruct(
                beatmap_row,
                &dataset.hit_objects,
                &dataset.timing_points,
                &dataset.slider_control_points,
                &dataset.slider_data,
                &dataset.breaks,
                &dataset.combo_colors,
                &dataset.hit_samples,
            )?;

            let osu_path = folder_output.join(&beatmap_row.osu_file);
            beatmap.encode_to_path(&osu_path)
                .context(format!("Failed to write beatmap: {}", osu_path.display()))?;
            
            result.osu_files.push(beatmap_row.osu_file.clone());

            // Check for embedded storyboard content for this .osu file
            let embedded_elements = StoryboardReconstructor::reconstruct(
                folder_id,
                &beatmap_row.osu_file,
                &dataset.storyboard_elements,
                &dataset.storyboard_commands,
                &dataset.storyboard_loops,
                &dataset.storyboard_triggers,
            );
            let embedded_sb: Vec<_> = embedded_elements.into_iter().filter(|e| e.is_embedded).collect();
            if !embedded_sb.is_empty() {
                // Write embedded storyboard content to .osb file with matching name
                let osb_filename = beatmap_row.osu_file.replace(".osu", ".osb");
                let osb_content = StoryboardReconstructor::to_osb_content(&embedded_sb);
                let osb_path = folder_output.join(&osb_filename);
                fs::write(&osb_path, osb_content)
                    .context(format!("Failed to write embedded storyboard: {}", osb_path.display()))?;
                result.storyboard_elements += embedded_sb.len();
            }
        }

        // Get standalone .osb storyboard files for this folder
        let sb_files = StoryboardReconstructor::get_storyboard_files(folder_id, &dataset.storyboard_elements);
        
        for sb_file in &sb_files {
            let elements = StoryboardReconstructor::reconstruct(
                folder_id,
                sb_file,
                &dataset.storyboard_elements,
                &dataset.storyboard_commands,
                &dataset.storyboard_loops,
                &dataset.storyboard_triggers,
            );
            
            // Only write if there are sprite/animation elements
            let has_sb_content = elements.iter().any(|e| 
                e.element_type == "sprite" || e.element_type == "animation"
            );
            
            if has_sb_content {
                // Generate .osb file if separate storyboard
                if sb_file.ends_with(".osb") {
                    let osb_content = StoryboardReconstructor::to_osb_content(&elements);
                    let osb_path = folder_output.join(sb_file);
                    fs::write(&osb_path, osb_content)
                        .context(format!("Failed to write storyboard: {}", osb_path.display()))?;
                }
            }
            
            result.storyboard_elements += elements.len();
        }

        // Copy assets
        let assets_source = self.assets_dir.join(folder_id);
        if assets_source.exists() {
            result.assets_copied = self.copy_assets(&assets_source, &folder_output)?;
        }

        // Copy audio file if exists
        if let Some(first_beatmap) = beatmap_rows.first() {
            let audio_source = assets_source.join(&first_beatmap.audio_file);
            if audio_source.exists() {
                let audio_dest = folder_output.join(&first_beatmap.audio_file);
                if let Some(parent) = audio_dest.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::copy(&audio_source, &audio_dest)?;
            }
        }

        Ok(result)
    }

    /// Copy all assets from source to destination
    fn copy_assets(&self, source: &Path, dest: &Path) -> Result<usize> {
        let mut count = 0;
        
        if !source.exists() {
            return Ok(0);
        }

        for entry in walkdir::WalkDir::new(source).into_iter().flatten() {
            let path: &std::path::Path = entry.path();
            if path.is_file() {
                let rel_path = path.strip_prefix(source)?;
                let dest_path = dest.join(rel_path);
                
                if let Some(parent) = dest_path.parent() {
                    fs::create_dir_all(parent)?;
                }
                
                fs::copy(path, &dest_path)?;
                count += 1;
            }
        }
        
        Ok(count)
    }

    /// Get all unique folder IDs in the dataset
    pub fn get_folder_ids(dataset: &Dataset) -> Vec<String> {
        let mut ids: Vec<String> = dataset.beatmaps
            .iter()
            .map(|b| b.folder_id.clone())
            .collect();
        ids.sort();
        ids.dedup();
        ids
    }
}

/// Result of folder reconstruction
#[derive(Debug)]
pub struct ReconstructedFolder {
    pub folder_id: String,
    pub output_path: std::path::PathBuf,
    pub osu_files: Vec<String>,
    pub storyboard_elements: usize,
    pub assets_copied: usize,
}
