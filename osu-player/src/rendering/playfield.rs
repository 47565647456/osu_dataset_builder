//! Playfield background and coordinate transformation

use bevy::prelude::*;

use crate::beatmap::{PLAYFIELD_HEIGHT, PLAYFIELD_WIDTH};
use crate::rendering::sdf_materials::GridMaterial;

pub struct PlayfieldPlugin;

impl Plugin for PlayfieldPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlayfieldTransform>()
            .init_resource::<ZoomLevel>()
            .add_systems(Startup, setup_camera)
            .add_systems(Startup, setup_playfield)
            .add_systems(Update, (
                handle_zoom_input,
                handle_pan_reset_input,
                update_playfield_transform,
            ).chain());
    }
}

/// Resource for zoom level control
#[derive(Resource)]
pub struct ZoomLevel {
    pub level: f32,  // 1.0 = normal, <1.0 = zoomed out, >1.0 = zoomed in
}

impl Default for ZoomLevel {
    fn default() -> Self {
        Self { level: 1.0 }
    }
}

/// Resource for coordinate transformation from osu! to screen space
#[derive(Resource, Default)]
pub struct PlayfieldTransform {
    /// Scale factor
    pub scale: f32,
    /// Offset to center playfield
    pub offset: Vec2,
    /// Manual panning offset controlled by user
    pub user_offset: Vec2,
    /// Playfield size in screen coordinates
    pub size: Vec2,
    /// Generation counter - incremented when transform changes
    pub generation: u32,
}

impl PlayfieldTransform {
    /// Convert osu! coordinates to screen coordinates
    pub fn osu_to_screen(&self, x: f32, y: f32) -> Vec2 {
        let final_offset = self.offset + self.user_offset;
        Vec2::new(
            final_offset.x + x * self.scale - PLAYFIELD_WIDTH * self.scale / 2.0,
            final_offset.y - y * self.scale + PLAYFIELD_HEIGHT * self.scale / 2.0, // Flip Y
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
    commands.spawn((
        Camera2d,
        Projection::Orthographic(OrthographicProjection {
            far: 20000.0,
            ..OrthographicProjection::default_2d()
        }),
    ));
}

/// Setup the playfield background
fn setup_playfield(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut grid_materials: ResMut<Assets<GridMaterial>>,
) {
    use crate::rendering::sdf_materials::{GridMaterial, GridUniforms};
    use bevy::sprite_render::MeshMaterial2d;

    // Create grid mesh covering playfield
    let mesh = Mesh::from(Rectangle::new(PLAYFIELD_WIDTH, PLAYFIELD_HEIGHT));
    let mesh_handle = meshes.add(mesh);

    // Create grid material with thin grey lines
    let material = GridMaterial {
        uniforms: GridUniforms {
            background_color: Color::srgb(0.06, 0.06, 0.09).into(),
            line_color: Color::srgb(0.15, 0.15, 0.2).into(),
            cell_size: 32.0,  // Grid cell size
            line_thickness: 1.0,  // Thin lines
            _padding: Vec2::ZERO,
        },
    };
    let material_handle = grid_materials.add(material);

    // Playfield grid background
    commands.spawn((
        Mesh2d(mesh_handle),
        MeshMaterial2d(material_handle),
        Transform::from_xyz(0.0, 0.0, -10000.0),
        PlayfieldBackground,
    ));

    // Playfield border
    commands.spawn((
        Sprite {
            color: Color::srgb(0.24, 0.24, 0.31),
            custom_size: Some(Vec2::new(PLAYFIELD_WIDTH + 4.0, PLAYFIELD_HEIGHT + 4.0)),
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, -10001.0),
    ));
}

/// Handle keyboard and mouse wheel input for zoom
fn handle_zoom_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut scroll_events: MessageReader<bevy::input::mouse::MouseWheel>,
    mut zoom: ResMut<ZoomLevel>,
) {
    let zoom_speed = 0.05;
    let scroll_zoom_speed = 0.1;
    let min_zoom = 0.3;
    let max_zoom = 2.0;

    // Keyboard zoom
    if keyboard.pressed(KeyCode::Equal) || keyboard.pressed(KeyCode::NumpadAdd) {
        zoom.level = (zoom.level + zoom_speed).min(max_zoom);
    }
    if keyboard.pressed(KeyCode::Minus) || keyboard.pressed(KeyCode::NumpadSubtract) {
        zoom.level = (zoom.level - zoom_speed).max(min_zoom);
    }
    // Reset zoom with 0 key
    if keyboard.just_pressed(KeyCode::Digit0) || keyboard.just_pressed(KeyCode::Numpad0) {
        zoom.level = 1.0;
    }

    // Mouse wheel zoom
    for event in scroll_events.read() {
        let delta = event.y * scroll_zoom_speed;
        zoom.level = (zoom.level + delta).clamp(min_zoom, max_zoom);
    }
}

/// Handle mouse dragging for panning and F key for reset
fn handle_pan_reset_input(
    mouse_button: Res<ButtonInput<MouseButton>>,
    mut mouse_motion: MessageReader<bevy::input::mouse::MouseMotion>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut zoom: ResMut<ZoomLevel>,
    mut transform: ResMut<PlayfieldTransform>,
    ui_interaction_query: Query<&Interaction, With<Node>>,
    mut is_dragging: Local<bool>,
) {
    // Reset with F key
    if keyboard.just_pressed(KeyCode::KeyF) {
        zoom.level = 1.0;
        transform.user_offset = Vec2::ZERO;
        transform.generation = transform.generation.wrapping_add(1);
    }

    // Check for drag start
    if mouse_button.just_pressed(MouseButton::Left) {
        // Only start dragging if not clicking on any UI elements
        let is_over_ui = ui_interaction_query.iter().any(|i| *i != Interaction::None);
        if !is_over_ui {
            *is_dragging = true;
        }
    }

    if mouse_button.just_released(MouseButton::Left) {
        *is_dragging = false;
    }

    // Panning with left mouse button (only if drag started in valid area)
    if *is_dragging && mouse_button.pressed(MouseButton::Left) {
        let mut delta = Vec2::ZERO;
        for event in mouse_motion.read() {
            delta += event.delta;
        }
        
        if delta != Vec2::ZERO {
            transform.user_offset.x += delta.x;
            transform.user_offset.y -= delta.y; // Screen Y is inverted relative to our world Y
            transform.generation = transform.generation.wrapping_add(1);
        }
    } else {
        // Drain mouse motion even if not dragging
        mouse_motion.clear();
    }
}

/// Update playfield transform based on window size and zoom
fn update_playfield_transform(
    windows: Query<&Window>,
    zoom: Res<ZoomLevel>,
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

        // Calculate scale to fit playfield, then apply zoom
        let padding = 40.0;
        let scale_x = (window_width - padding * 2.0) / PLAYFIELD_WIDTH;
        let scale_y = (available_height - padding * 2.0) / PLAYFIELD_HEIGHT;
        let base_scale = scale_x.min(scale_y);
        let scale = base_scale * zoom.level;

        // Check if transform changed
        let base_offset = Vec2::new(0.0, ui_height / 2.0);
        let final_offset = base_offset + transform.user_offset;

        if (transform.scale - scale).abs() > 0.001 || (transform.offset - base_offset).length() > 0.001 {
            transform.generation = transform.generation.wrapping_add(1);
        }

        transform.scale = scale;
        transform.size = Vec2::new(PLAYFIELD_WIDTH * scale, PLAYFIELD_HEIGHT * scale);
        
        // Base alignment offset
        transform.offset = base_offset;

        // Update background sprite
        for mut tf in playfield_query.iter_mut() {
            tf.scale = Vec3::splat(scale);
            tf.translation.x = transform.user_offset.x;
            tf.translation.y = final_offset.y;
        }

        // Update border (background elements are Z < -100.0)
        for mut tf in border_query.iter_mut() {
            if tf.translation.z < -100.0 {
                tf.scale = Vec3::splat(scale);
                tf.translation.x = transform.user_offset.x;
                tf.translation.y = final_offset.y;
            }
        }
    }
}

