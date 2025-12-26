//! Playfield background and coordinate transformation

use bevy::prelude::*;

use crate::beatmap::{PLAYFIELD_HEIGHT, PLAYFIELD_WIDTH};

pub struct PlayfieldPlugin;

impl Plugin for PlayfieldPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlayfieldTransform>()
            .add_systems(Startup, setup_camera)
            .add_systems(Startup, setup_playfield)
            .add_systems(Update, update_playfield_transform);
    }
}

/// Resource for coordinate transformation from osu! to screen space
#[derive(Resource, Default)]
pub struct PlayfieldTransform {
    /// Scale factor
    pub scale: f32,
    /// Offset to center playfield
    pub offset: Vec2,
    /// Playfield size in screen coordinates
    pub size: Vec2,
}

impl PlayfieldTransform {
    /// Convert osu! coordinates to screen coordinates
    pub fn osu_to_screen(&self, x: f32, y: f32) -> Vec2 {
        Vec2::new(
            self.offset.x + x * self.scale - PLAYFIELD_WIDTH * self.scale / 2.0,
            self.offset.y - y * self.scale + PLAYFIELD_HEIGHT * self.scale / 2.0, // Flip Y
        )
    }

    /// Scale a radius from osu! to screen space
    pub fn scale_radius(&self, radius: f32) -> f32 {
        radius * self.scale
    }
}

/// Marker component for playfield background
#[derive(Component)]
pub struct PlayfieldBackground;

/// Setup the 2D camera
fn setup_camera(mut commands: Commands) {
    commands.spawn(Camera2d::default());
}

/// Setup the playfield background
fn setup_playfield(mut commands: Commands) {
    // Playfield background sprite
    commands.spawn((
        Sprite {
            color: Color::srgb(0.08, 0.08, 0.12),
            custom_size: Some(Vec2::new(PLAYFIELD_WIDTH, PLAYFIELD_HEIGHT)),
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, -10.0),
        PlayfieldBackground,
    ));

    // Playfield border
    commands.spawn((
        Sprite {
            color: Color::srgb(0.24, 0.24, 0.31),
            custom_size: Some(Vec2::new(PLAYFIELD_WIDTH + 4.0, PLAYFIELD_HEIGHT + 4.0)),
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, -11.0),
    ));
}

/// Update playfield transform based on window size
fn update_playfield_transform(
    windows: Query<&Window>,
    mut transform: ResMut<PlayfieldTransform>,
    mut playfield_query: Query<&mut Transform, With<PlayfieldBackground>>,
    mut border_query: Query<&mut Transform, (Without<PlayfieldBackground>, Without<Camera2d>)>,
) {
    if let Ok(window) = windows.single() {
        let window_width = window.width();
        let window_height = window.height();

        // Reserve space for UI at bottom
        let ui_height = 120.0;
        let available_height = window_height - ui_height;

        // Calculate scale to fit playfield
        let padding = 40.0;
        let scale_x = (window_width - padding * 2.0) / PLAYFIELD_WIDTH;
        let scale_y = (available_height - padding * 2.0) / PLAYFIELD_HEIGHT;
        let scale = scale_x.min(scale_y);

        transform.scale = scale;
        transform.size = Vec2::new(PLAYFIELD_WIDTH * scale, PLAYFIELD_HEIGHT * scale);
        
        // Center playfield, offset up to make room for UI
        transform.offset = Vec2::new(0.0, ui_height / 2.0);

        // Update background sprite
        for mut tf in playfield_query.iter_mut() {
            tf.scale = Vec3::splat(scale);
            tf.translation.y = ui_height / 2.0;
        }

        // Update border
        for mut tf in border_query.iter_mut() {
            if tf.translation.z < -10.0 {
                tf.scale = Vec3::splat(scale);
                tf.translation.y = ui_height / 2.0;
            }
        }
    }
}
