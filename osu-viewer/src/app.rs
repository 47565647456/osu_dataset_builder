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

const FRAMETIME_HISTORY_SIZE: usize = 120;

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
    /// Frame time history for graph (in milliseconds)
    frametime_history: VecDeque<f32>,
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
            frametime_history: VecDeque::with_capacity(FRAMETIME_HISTORY_SIZE),
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
        
        // Add to history
        if self.frametime_history.len() >= FRAMETIME_HISTORY_SIZE {
            self.frametime_history.pop_front();
        }
        self.frametime_history.push_back(frametime_ms);
    }
    
    /// Draw frametime graph
    fn draw_frametime_graph(&self, painter: &egui::Painter, rect: Rect) {
        if self.frametime_history.is_empty() {
            return;
        }
        
        // Background
        painter.rect_filled(rect, 4.0, Color32::from_rgba_unmultiplied(0, 0, 0, 180));
        painter.rect_stroke(rect, 4.0, Stroke::new(1.0, Color32::from_rgb(60, 60, 80)));
        
        // Calculate max frametime for scaling (use 33ms = 30fps as baseline)
        let max_ft = self.frametime_history.iter().copied().fold(33.3f32, f32::max);
        let scale_max = max_ft.max(33.3);
        
        // Draw 16.67ms (60fps) and 33.33ms (30fps) lines
        let line_y_60fps = rect.max.y - (16.67 / scale_max) * rect.height();
        let line_y_30fps = rect.max.y - (33.33 / scale_max) * rect.height();
        
        if line_y_60fps > rect.min.y {
            painter.line_segment(
                [Pos2::new(rect.min.x, line_y_60fps), Pos2::new(rect.max.x, line_y_60fps)],
                Stroke::new(1.0, Color32::from_rgba_unmultiplied(0, 255, 0, 100)),
            );
        }
        if line_y_30fps > rect.min.y && line_y_30fps < rect.max.y {
            painter.line_segment(
                [Pos2::new(rect.min.x, line_y_30fps), Pos2::new(rect.max.x, line_y_30fps)],
                Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 0, 100)),
            );
        }
        
        // Draw graph bars
        let bar_width = rect.width() / FRAMETIME_HISTORY_SIZE as f32;
        
        for (i, &ft) in self.frametime_history.iter().enumerate() {
            let x = rect.min.x + i as f32 * bar_width;
            let height = (ft / scale_max) * rect.height();
            let y = rect.max.y - height;
            
            // Color based on frametime (green < 16.67ms, yellow < 33ms, red > 33ms)
            let color = if ft <= 16.67 {
                Color32::from_rgb(0, 255, 100)
            } else if ft <= 33.33 {
                Color32::from_rgb(255, 255, 0)
            } else {
                Color32::from_rgb(255, 80, 80)
            };
            
            let bar_rect = Rect::from_min_max(
                Pos2::new(x, y),
                Pos2::new(x + bar_width.max(1.0), rect.max.y),
            );
            painter.rect_filled(bar_rect, 0.0, color);
        }
        
        // Draw current frametime and FPS text
        if let Some(&current_ft) = self.frametime_history.back() {
            let fps = 1000.0 / current_ft;
            let avg_ft: f32 = self.frametime_history.iter().sum::<f32>() / self.frametime_history.len() as f32;
            let avg_fps = 1000.0 / avg_ft;
            
            painter.text(
                Pos2::new(rect.min.x + 4.0, rect.min.y + 2.0),
                egui::Align2::LEFT_TOP,
                format!("{:.1}ms ({:.0} FPS)", current_ft, fps),
                egui::FontId::monospace(10.0),
                Color32::WHITE,
            );
            painter.text(
                Pos2::new(rect.min.x + 4.0, rect.min.y + 14.0),
                egui::Align2::LEFT_TOP,
                format!("avg: {:.1}ms ({:.0} FPS)", avg_ft, avg_fps),
                egui::FontId::monospace(10.0),
                Color32::from_rgb(180, 180, 180),
            );
        }
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
                
                // Draw frametime graph in top-right corner
                let graph_width = 200.0;
                let graph_height = 60.0;
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
                        ui.label("Space: Play/Pause | ←/→: Seek | ↑/↓: Speed");
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
