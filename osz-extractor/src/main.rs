use anyhow::{Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use walkdir::WalkDir;
use zip::ZipArchive;

/// Rate limiter state for nerinyan API (25 requests per minute)
struct RateLimiter {
    last_request: Option<Instant>,
    min_interval: Duration,
}

impl RateLimiter {
    fn new(requests_per_minute: u32) -> Self {
        Self {
            last_request: None,
            min_interval: Duration::from_secs(60) / requests_per_minute,
        }
    }

    fn wait(&mut self) {
        if let Some(last) = self.last_request {
            let elapsed = last.elapsed();
            if elapsed < self.min_interval {
                thread::sleep(self.min_interval - elapsed);
            }
        }
        self.last_request = Some(Instant::now());
    }
}

/// Download beatmapset from nerinyan mirror
fn download_from_nerinyan(beatmapset_id: &str, dest_path: &Path) -> Result<()> {
    let url = format!("https://api.nerinyan.moe/d/{}", beatmapset_id);
    
    let response = reqwest::blocking::Client::new()
        .get(&url)
        .send()
        .with_context(|| format!("Failed to download from nerinyan: {}", beatmapset_id))?;
    
    if !response.status().is_success() {
        anyhow::bail!("Nerinyan returned status {}", response.status());
    }
    
    let bytes = response.bytes()
        .context("Failed to read response bytes")?;
    
    let mut file = File::create(dest_path)
        .with_context(|| format!("Failed to create file: {}", dest_path.display()))?;
    
    file.write_all(&bytes)
        .context("Failed to write downloaded file")?;
    
    Ok(())
}

fn main() -> Result<()> {
    // Set up graceful shutdown flag
    let shutdown_requested = Arc::new(AtomicBool::new(false));
    let shutdown_clone = shutdown_requested.clone();

    ctrlc::set_handler(move || {
        println!("\n⏳ Ctrl+C received! Finishing current file then stopping...");
        shutdown_clone.store(true, Ordering::SeqCst);
    })
    .expect("Error setting Ctrl+C handler");

    // Default .osz archives path - adjust if needed
    let songs_folder = std::env::args()
        .nth(1)
        .unwrap_or_else(|| r"E:\osu_model\osu_archives".to_string());

    let songs_path = Path::new(&songs_folder);

    if !songs_path.exists() {
        anyhow::bail!("Songs folder does not exist: {}", songs_folder);
    }

    println!("Scanning for .osz files in: {}", songs_folder);

    // Collect all .osz files
    let osz_files: Vec<PathBuf> = WalkDir::new(songs_path)
        .max_depth(1) // Only look in the root of songs folder
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map(|ext| ext.eq_ignore_ascii_case("osz"))
                .unwrap_or(false)
        })
        .map(|e| e.path().to_path_buf())
        .collect();

    if osz_files.is_empty() {
        println!("No .osz files found.");
        return Ok(());
    }

    println!("Found {} .osz files to extract", osz_files.len());
    println!("Press Ctrl+C to stop gracefully (will finish current file)\n");

    let pb = ProgressBar::new(osz_files.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})")
            .unwrap()
            .progress_chars("#>-"),
    );

    let mut extracted_count = 0;
    let mut failed_count = 0;
    let mut skipped_count = 0;
    let mut downloaded_count = 0;
    
    // Rate limiter for nerinyan API (25 requests per minute)
    let mut rate_limiter = RateLimiter::new(25);

    for osz_path in &osz_files {
        // Check if shutdown was requested before starting next file
        if shutdown_requested.load(Ordering::SeqCst) {
            skipped_count = osz_files.len() - extracted_count - failed_count;
            pb.println("🛑 Stopping gracefully...");
            break;
        }

        let osz_name = osz_path.file_name().unwrap_or_default().to_string_lossy();
        pb.set_message(format!("{}", osz_name));

        // Try to extract
        match extract_and_delete_osz(osz_path, songs_path) {
            Ok(_) => {
                extracted_count += 1;
            }
            Err(e) => {
                let error_str = format!("{}", e);
                
                // Check if it's a zip read failure - try downloading from nerinyan
                if error_str.contains("Failed to read zip") {
                    // Extract beatmapset ID from filename (the numeric part)
                    let beatmapset_id = osz_path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("");
                    
                    // Only try if the filename looks like a beatmapset ID
                    if beatmapset_id.chars().all(|c| c.is_ascii_digit()) && !beatmapset_id.is_empty() {
                        pb.println(format!("⬇️  {} - Downloading from nerinyan...", osz_name));
                        
                        // Rate limit
                        rate_limiter.wait();
                        
                        // Download to a temp file
                        let temp_path = osz_path.with_extension("osz.tmp");
                        match download_from_nerinyan(beatmapset_id, &temp_path) {
                            Ok(_) => {
                                downloaded_count += 1;
                                
                                // Replace the corrupt file with the downloaded one
                                if let Err(e) = fs::rename(&temp_path, osz_path) {
                                    pb.println(format!("❌ {} - Failed to replace file: {}", osz_name, e));
                                    let _ = fs::remove_file(&temp_path);
                                    failed_count += 1;
                                } else {
                                    // Retry extraction with the new file
                                    match extract_and_delete_osz(osz_path, songs_path) {
                                        Ok(_) => {
                                            pb.println(format!("✅ {} - Downloaded and extracted", osz_name));
                                            extracted_count += 1;
                                        }
                                        Err(e) => {
                                            pb.println(format!("❌ {} - Still failed after download: {}", osz_name, e));
                                            failed_count += 1;
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                pb.println(format!("❌ {} - Download failed: {}", osz_name, e));
                                let _ = fs::remove_file(&temp_path);
                                failed_count += 1;
                            }
                        }
                    } else {
                        pb.println(format!("❌ {} - {}", osz_name, e));
                        failed_count += 1;
                    }
                } else {
                    pb.println(format!("❌ {} - {}", osz_name, e));
                    failed_count += 1;
                }
            }
        }

        pb.inc(1);
    }

    pb.finish_and_clear();

    println!("\n✅ Summary:");
    println!("   Extracted:  {}", extracted_count);
    println!("   Downloaded: {}", downloaded_count);
    println!("   Failed:     {}", failed_count);
    if skipped_count > 0 {
        println!("   Skipped:    {} (due to Ctrl+C)", skipped_count);
    }

    Ok(())
}

/// Detect if file content is audio using magic bytes
fn is_audio_content(data: &[u8]) -> bool {
    infer::get(data)
        .map(|kind| kind.matcher_type() == infer::MatcherType::Audio)
        .unwrap_or(false)
}

/// Check if path has audio file extension
fn is_audio_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|ext| {
            matches!(ext.to_lowercase().as_str(), 
                "mp3" | "ogg" | "wav" | "flac" | "m4a" | "wma")
        })
}

/// Combined audio check: magic bytes OR extension
fn is_audio(path: &Path, data: &[u8]) -> bool {
    is_audio_content(data) || is_audio_extension(path)
}

/// Check if a path has .osu extension
fn is_osu_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("osu"))
}

/// Check if a path has .osb extension
fn is_osb_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("osb"))
}

/// Parsed image references from an .osu file
struct OsuImageRefs {
    /// The main background image (from 0,0 line) - required
    background: Option<String>,
    /// Optional storyboard images (sprites, animations) - not required to exist
    storyboard: Vec<String>,
}

/// Normalize path separators for consistent comparison (backslash to forward slash, lowercase)
fn normalize_path(path: &str) -> String {
    path.replace('\\', "/").to_lowercase()
}

/// Parse .osu file content to extract image references from [Events] section
fn parse_images_from_osu(content: &str) -> OsuImageRefs {
    let mut refs = OsuImageRefs {
        background: None,
        storyboard: Vec::new(),
    };
    let mut in_events = false;
    
    for line in content.lines() {
        let line = line.trim();
        
        if line == "[Events]" {
            in_events = true;
            continue;
        }
        
        // Check if we've left the Events section
        if in_events && line.starts_with('[') {
            break;
        }
        
        if !in_events {
            continue;
        }

        // Skip comments, empty lines, and videos
        if line.is_empty() || line.starts_with("//") || line.starts_with("Video,") {
            continue;
        }

        // Extract filename from quoted string in the line
        if let Some(start) = line.find('"') {
            if let Some(end) = line[start + 1..].find('"') {
                let filename = line[start + 1..start + 1 + end].to_string();
                if filename.is_empty() {
                    continue;
                }
                
                // Background line: 0,0,"filename",...
                if line.starts_with("0,0,") {
                    if refs.background.is_none() {
                        refs.background = Some(filename);
                    }
                } else {
                    // Storyboard sprite/animation
                    refs.storyboard.push(filename);
                }
            }
        }
    }
    
    refs
}

fn extract_and_delete_osz(osz_path: &Path, songs_folder: &Path) -> Result<()> {
    use std::collections::HashSet;
    
    // Get the filename without extension to use as folder name
    let folder_name = osz_path
        .file_stem()
        .context("Failed to get file stem")?
        .to_string_lossy();

    // Extract to osu_archives_extracted folder in the parent (root) directory
    let root_folder = songs_folder.parent().unwrap_or(songs_folder);
    let extract_folder = root_folder.join("osu_archives_extracted").join(folder_name.as_ref());

    // Create the extraction folder
    fs::create_dir_all(&extract_folder)
        .with_context(|| format!("Failed to create folder: {}", extract_folder.display()))?;

    // Open the .osz file (which is just a zip archive)
    let file = File::open(osz_path)
        .with_context(|| format!("Failed to open: {}", osz_path.display()))?;

    let mut archive = ZipArchive::new(file)
        .with_context(|| format!("Failed to read zip: {}", osz_path.display()))?;

    // First pass: read all files
    let mut files_data: Vec<(PathBuf, Vec<u8>)> = Vec::new();

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;

        // Skip directories
        if file.is_dir() {
            continue;
        }

        // Get the file path, handling potential directory entries
        let inner_path = match file.enclosed_name() {
            Some(path) => path.to_path_buf(),
            None => continue,
        };

        // Read file content
        let mut data = Vec::new();
        io::Read::read_to_end(&mut file, &mut data)?;

        files_data.push((inner_path, data));
    }

    // Second pass: parse .osu files to find referenced images
    let mut required_backgrounds: HashSet<String> = HashSet::new();
    let mut optional_images: HashSet<String> = HashSet::new();
    let mut has_osu_files = false;
    
    for (path, data) in &files_data {
        if is_osu_file(path) || is_osb_file(path) {
            if is_osu_file(path) {
                has_osu_files = true;
            }
            if let Ok(content) = std::str::from_utf8(data) {
                let refs = parse_images_from_osu(content);
                if let Some(bg) = refs.background {
                    required_backgrounds.insert(normalize_path(&bg));
                }
                for img in refs.storyboard {
                    optional_images.insert(normalize_path(&img));
                }
            }
        }
    }

    // Validate: must have at least one .osu file
    if !has_osu_files {
        anyhow::bail!("No .osu files found in archive");
    }

    // Note: Background images are optional (some maps use videos or plain colors)

    // Build a set of normalized archive paths for lookup (case-insensitive)
    let archive_paths: HashSet<String> = files_data
        .iter()
        .map(|(path, _)| normalize_path(&path.to_string_lossy()))
        .collect();

    // Log missing backgrounds (optional but useful to know)
    let osz_name = osz_path.file_name().unwrap_or_default().to_string_lossy();
    for bg in &required_backgrounds {
        if !archive_paths.contains(bg) {
            eprintln!("  ⚠ {} - Background not found: {}", osz_name, bg);
        }
    }

    // Log missing storyboard images
    for img in &optional_images {
        if !archive_paths.contains(img) {
            eprintln!("  ⚠ {} - Storyboard image not found: {}", osz_name, img);
        }
    }

    // Filter to only images that actually exist in the archive (case-insensitive)
    let all_images: HashSet<String> = required_backgrounds
        .union(&optional_images)
        .filter(|img| archive_paths.contains(*img))
        .cloned()
        .collect();

    // Validate: must have at least one audio file
    let has_audio = files_data.iter().any(|(path, data)| is_audio(path, data));
    if !has_audio {
        anyhow::bail!("No audio files found in archive");
    }

    // Third pass: write only essential files (audio, .osu, referenced images that exist)
    for (inner_path, data) in files_data {
        let normalized = normalize_path(&inner_path.to_string_lossy());
        
        let should_extract = is_osu_file(&inner_path) 
            || is_osb_file(&inner_path)
            || is_audio(&inner_path, &data)
            || all_images.contains(&normalized);

        if !should_extract {
            continue;
        }

        let outpath = extract_folder.join(&inner_path);

        // Ensure parent directory exists
        if let Some(parent) = outpath.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut outfile = File::create(&outpath)
            .with_context(|| format!("Failed to create file: {}", outpath.display()))?;

        io::Write::write_all(&mut outfile, &data)?;
    }

    // Delete the original .osz file after successful extraction
    // Commented out for now.
    // fs::remove_file(osz_path)
    //     .with_context(|| format!("Failed to delete: {}", osz_path.display()))?;

    Ok(())
}
