//! Main application implementing eframe::App

use crate::audio::AudioPlayer;
use crate::beatmap::BeatmapView;
use crate::playback::{PlaybackManager, PlaybackState};
use crate::renderer::PlayfieldRenderer;
use crate::timeline::Timeline;
use egui::{Color32, Key, Pos2, Rect, Stroke, Vec2};
use std::path::PathBuf;
use std::time::Instant;
use std::collections::VecDeque;

/// Number of bars to show in the graph
const FRAMETIME_BAR_COUNT: usize = 60;
/// Number of raw samples to average for each bar
const SAMPLES_PER_BAR: usize = 4;

/// Main application state
pub struct OsuViewerApp {
    /// Beatmap data
    beatmap: BeatmapView,
    /// Audio player
    audio: AudioPlayer,
    /// Playback state manager
    playback: PlaybackManager,
    /// Timeline UI
    timeline: Timeline,
    /// Whether audio is available
    has_audio: bool,
    /// Frame time history for graph (averaged, in milliseconds)
    frametime_history: VecDeque<f32>,
    /// Raw samples for current averaging window
    raw_samples: Vec<f32>,
    /// All raw samples for 1% low calculation
    all_samples: VecDeque<f32>,
    /// Last frame time
    last_frame_time: Instant,
}

impl OsuViewerApp {
    pub fn new(
        _cc: &eframe::CreationContext<'_>,
        beatmap: rosu_map::Beatmap,
        audio_path: Option<PathBuf>,
    ) -> Self {
        let beatmap_view = BeatmapView::new(beatmap);
        let total_duration = beatmap_view.total_duration;

        let mut audio = AudioPlayer::new().expect("Failed to create audio player");
        let has_audio = if let Some(path) = audio_path {
            match audio.load(&path) {
                Ok(()) => {
                    log::info!("Loaded audio: {}", path.display());
                    true
                }
                Err(e) => {
                    log::warn!("Failed to load audio: {}", e);
                    false
                }
            }
        } else {
            false
        };

        Self {
            beatmap: beatmap_view,
            audio,
            playback: PlaybackManager::new(total_duration),
            timeline: Timeline::new(),
            has_audio,
            frametime_history: VecDeque::with_capacity(FRAMETIME_BAR_COUNT),
            raw_samples: Vec::with_capacity(SAMPLES_PER_BAR),
            all_samples: VecDeque::with_capacity(500), // ~8 seconds at 60fps
            last_frame_time: Instant::now(),
        }
    }

    /// Handle keyboard input
    fn handle_input(&mut self, ctx: &egui::Context) {
        ctx.input(|input| {
            // Space: toggle play/pause
            if input.key_pressed(Key::Space) {
                self.toggle_playback();
            }

            // Left/Right: seek
            if input.key_pressed(Key::ArrowLeft) {
                self.seek_delta(-5000.0); // -5 seconds
            }
            if input.key_pressed(Key::ArrowRight) {
                self.seek_delta(5000.0); // +5 seconds
            }

            // Up/Down: playback speed
            if input.key_pressed(Key::ArrowUp) {
                self.change_speed(0.25);
            }
            if input.key_pressed(Key::ArrowDown) {
                self.change_speed(-0.25);
            }

            // Home: go to start
            if input.key_pressed(Key::Home) {
                self.seek(0.0);
            }

            // End: go to end
            if input.key_pressed(Key::End) {
                self.seek(self.playback.total_duration - 1000.0);
            }
        });
    }

    fn toggle_playback(&mut self) {
        self.playback.toggle_play();
        
        if self.has_audio {
            if self.playback.state == PlaybackState::Playing {
                self.audio.play();
            } else {
                self.audio.pause();
            }
        }
    }

    fn seek(&mut self, time: f64) {
        self.playback.seek(time);
        if self.has_audio {
            self.audio.seek_ms(time);
        }
    }

    fn seek_delta(&mut self, delta: f64) {
        let new_time = (self.playback.current_time + delta).clamp(0.0, self.playback.total_duration);
        self.seek(new_time);
    }

    fn change_speed(&mut self, delta: f64) {
        let new_speed = (self.playback.speed + delta).clamp(0.25, 4.0);
        self.playback.set_speed(new_speed);
        if self.has_audio {
            self.audio.set_speed(new_speed);
        }
    }

    /// Update playback timing
    fn update_playback(&mut self) {
        if self.has_audio && self.playback.state == PlaybackState::Playing {
            // Sync from audio
            self.playback.sync_from_audio(self.audio.position_ms());
        } else {
            // Manual time tracking
            self.playback.update_manual();
        }
    }
    
    /// Update frametime tracking
    fn update_frametime(&mut self) {
        let now = Instant::now();
        let frametime_ms = now.duration_since(self.last_frame_time).as_secs_f32() * 1000.0;
        self.last_frame_time = now;
        
        // Add to raw samples buffer
        self.raw_samples.push(frametime_ms);
        
        // Store for percentile calculation (keep last ~8 seconds)
        if self.all_samples.len() >= 500 {
            self.all_samples.pop_front();
        }
        self.all_samples.push_back(frametime_ms);
        
        // When we have enough samples, compute average and add to history
        if self.raw_samples.len() >= SAMPLES_PER_BAR {
            let avg = self.raw_samples.iter().sum::<f32>() / self.raw_samples.len() as f32;
            
            if self.frametime_history.len() >= FRAMETIME_BAR_COUNT {
                self.frametime_history.pop_front();
            }
            self.frametime_history.push_back(avg);
            self.raw_samples.clear();
        }
    }
    
    /// Calculate 1% low (99th percentile of frametimes)
    fn calculate_1_percent_low(&self) -> f32 {
        if self.all_samples.len() < 10 {
            return 0.0;
        }
        
        let mut sorted: Vec<f32> = self.all_samples.iter().copied().collect();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        
        // 1% low = highest 1% of frametimes (worst frames)
        let idx = ((sorted.len() as f32) * 0.99) as usize;
        let idx = idx.min(sorted.len() - 1);
        sorted[idx]
    }
    
    /// Draw frametime graph
    fn draw_frametime_graph(&self, painter: &egui::Painter, rect: Rect) {
        if self.frametime_history.is_empty() {
            return;
        }
        
        // Background
        painter.rect_filled(rect, 4.0, Color32::from_rgba_unmultiplied(0, 0, 0, 200));
        painter.rect_stroke(rect, 4.0, Stroke::new(1.0, Color32::from_rgb(60, 60, 80)));
        
        // Calculate stats
        let avg_ft: f32 = self.all_samples.iter().sum::<f32>() / self.all_samples.len().max(1) as f32;
        let one_percent_low_ft = self.calculate_1_percent_low();
        let one_percent_low_fps = if one_percent_low_ft > 0.0 { 1000.0 / one_percent_low_ft } else { 0.0 };
        let avg_fps = if avg_ft > 0.0 { 1000.0 / avg_ft } else { 0.0 };
        
        // Use a fixed scale (0-33.33ms) for consistency, but expand if needed
        let max_ft = self.frametime_history.iter().copied().fold(16.67f32, f32::max);
        let scale_max = max_ft.max(16.67).min(100.0); // Cap at 100ms for display
        
        // Graph area (lower portion of rect)
        let text_height = 40.0;
        let graph_rect = Rect::from_min_max(
            Pos2::new(rect.min.x, rect.min.y + text_height),
            rect.max,
        );
        
        // Draw reference lines
        let line_y_60fps = graph_rect.max.y - (16.67 / scale_max) * graph_rect.height();
        if line_y_60fps > graph_rect.min.y {
            painter.line_segment(
                [Pos2::new(graph_rect.min.x, line_y_60fps), Pos2::new(graph_rect.max.x, line_y_60fps)],
                Stroke::new(1.0, Color32::from_rgba_unmultiplied(0, 255, 0, 60)),
            );
            painter.text(
                Pos2::new(graph_rect.max.x - 2.0, line_y_60fps - 2.0),
                egui::Align2::RIGHT_BOTTOM,
                "60",
                egui::FontId::monospace(8.0),
                Color32::from_rgba_unmultiplied(0, 255, 0, 120),
            );
        }
        
        // Draw graph bars
        let bar_width = graph_rect.width() / FRAMETIME_BAR_COUNT as f32;
        
        for (i, &ft) in self.frametime_history.iter().enumerate() {
            let x = graph_rect.min.x + i as f32 * bar_width;
            let clamped_ft = ft.min(scale_max);
            let height = (clamped_ft / scale_max) * graph_rect.height();
            let y = graph_rect.max.y - height;
            
            // Smooth color gradient based on frametime
            let color = if ft <= 8.33 {
                // <120fps: bright green
                Color32::from_rgb(0, 255, 100)
            } else if ft <= 16.67 {
                // 60-120fps: green to yellow
                let t = (ft - 8.33) / 8.34;
                Color32::from_rgb((255.0 * t) as u8, 255, (100.0 * (1.0 - t)) as u8)
            } else if ft <= 33.33 {
                // 30-60fps: yellow to orange
                let t = (ft - 16.67) / 16.66;
                Color32::from_rgb(255, (255.0 * (1.0 - t * 0.5)) as u8, 0)
            } else {
                // <30fps: red
                Color32::from_rgb(255, 60, 60)
            };
            
            let bar_rect = Rect::from_min_max(
                Pos2::new(x, y),
                Pos2::new(x + bar_width - 1.0, graph_rect.max.y),
            );
            painter.rect_filled(bar_rect, 0.0, color);
        }
        
        // Draw text stats
        let current_ft = self.raw_samples.last().copied()
            .or_else(|| self.frametime_history.back().copied())
            .unwrap_or(0.0);
        let current_fps = if current_ft > 0.0 { 1000.0 / current_ft } else { 0.0 };
        
        painter.text(
            Pos2::new(rect.min.x + 4.0, rect.min.y + 2.0),
            egui::Align2::LEFT_TOP,
            format!("FPS: {:.0}", current_fps),
            egui::FontId::monospace(12.0),
            Color32::WHITE,
        );
        
        painter.text(
            Pos2::new(rect.min.x + 4.0, rect.min.y + 16.0),
            egui::Align2::LEFT_TOP,
            format!("Avg: {:.0} | 1% Low: {:.0}", avg_fps, one_percent_low_fps),
            egui::FontId::monospace(10.0),
            Color32::from_rgb(180, 180, 180),
        );
        
        painter.text(
            Pos2::new(rect.min.x + 4.0, rect.min.y + 28.0),
            egui::Align2::LEFT_TOP,
            format!("{:.2}ms", avg_ft),
            egui::FontId::monospace(9.0),
            Color32::from_rgb(140, 140, 140),
        );
    }
}

impl eframe::App for OsuViewerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Update frametime tracking
        self.update_frametime();
        
        // Always request repaint for smooth animation
        ctx.request_repaint();

        // Handle input
        self.handle_input(ctx);

        // Update playback
        self.update_playback();

        // Main panel with playfield
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(Color32::from_rgb(15, 15, 20)))
            .show(ctx, |ui| {
                // Calculate available space for playfield (leave room for controls and timeline)
                let available = ui.available_rect_before_wrap();
                let controls_height = 40.0;
                let timeline_height = 70.0;
                
                let playfield_rect = egui::Rect::from_min_max(
                    available.min,
                    egui::Pos2::new(available.max.x, available.max.y - controls_height - timeline_height),
                );

                // Draw playfield
                let renderer = PlayfieldRenderer::new(playfield_rect);
                let painter = ui.painter_at(playfield_rect);
                
                renderer.draw_playfield_bg(&painter);
                renderer.draw_objects(&painter, &self.beatmap, self.playback.current_time);
                
                // Draw countdown and break overlays
                renderer.draw_countdown(&painter, &self.beatmap, self.playback.current_time);
                renderer.draw_break(&painter, &self.beatmap, self.playback.current_time);
                
                // Draw combo counter in top-left corner
                renderer.draw_combo_counter(&painter, &self.beatmap, self.playback.current_time);
                
                // Draw map stats below combo
                renderer.draw_map_stats(&painter, &self.beatmap);
                
                // Draw frametime graph in top-right corner
                let graph_width = 180.0;
                let graph_height = 80.0;
                let graph_rect = Rect::from_min_size(
                    Pos2::new(playfield_rect.max.x - graph_width - 10.0, playfield_rect.min.y + 10.0),
                    Vec2::new(graph_width, graph_height),
                );
                self.draw_frametime_graph(&painter, graph_rect);

                // Allocate the playfield space
                ui.allocate_rect(playfield_rect, egui::Sense::hover());

                // Controls bar
                ui.horizontal(|ui| {
                    ui.add_space(10.0);

                    // Play/Pause button
                    let play_text = if self.playback.state == PlaybackState::Playing {
                        "⏸ Pause"
                    } else {
                        "▶ Play"
                    };
                    if ui.button(play_text).clicked() {
                        self.toggle_playback();
                    }

                    ui.separator();

                    // Speed control
                    ui.label("Speed:");
                    let speed_text = format!("{:.2}x", self.playback.speed);
                    if ui.button(&speed_text).clicked() {
                        self.playback.cycle_speed();
                        if self.has_audio {
                            self.audio.set_speed(self.playback.speed);
                        }
                    }

                    ui.separator();

                    // Object count
                    let visible_count = self.beatmap
                        .visible_objects(self.playback.current_time)
                        .count();
                    ui.label(format!(
                        "Objects: {} / {} visible",
                        self.beatmap.objects.len(),
                        visible_count
                    ));

                    ui.separator();

                    // Audio status
                    if self.has_audio {
                        ui.label("🔊 Audio");
                    } else {
                        ui.label("🔇 No Audio");
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label("Space: Play/Pause | Left/Right: Seek | Up/Down: Speed");
                    });
                });

                ui.add_space(5.0);

                // Timeline
                let current_str = self.playback.format_time(self.playback.current_time);
                let total_str = self.playback.format_time(self.playback.total_duration);
                
                if let Some(seek_time) = self.timeline.show(
                    ui,
                    &self.beatmap,
                    self.playback.current_time,
                    self.playback.total_duration,
                    &current_str,
                    &total_str,
                ) {
                    self.seek(seek_time);
                }
            });
    }
}
