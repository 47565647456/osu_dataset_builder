//! SDF-based rendering system for hit objects
//! Spawns and manages mesh entities with SDF materials for sliders and circles

use bevy::prelude::*;
use bevy::sprite_render::MeshMaterial2d;

use crate::beatmap::{BeatmapView, RenderObject, RenderObjectKind};
use crate::playback::PlaybackStateRes;
use crate::rendering::sdf_materials::{ArrowMaterial, ArrowUniforms, CircleMaterial, CircleUniforms, SliderMaterial, SliderPathData, SliderUniforms};
use crate::rendering::PlayfieldTransform;
use crate::ui::UiFont;

/// Marker component for SDF-rendered hit objects
#[derive(Component)]
pub struct SdfHitObject {
    /// Index into beatmap objects array
    pub object_index: usize,
}

/// Marker for slider mesh entities
#[derive(Component)]
pub struct SliderMesh;

/// Marker for slider head circle entities (for approach circle)
#[derive(Component)]
pub struct SliderHeadMesh;

/// Marker for slider tail circle entities
#[derive(Component)]
pub struct SliderTailMesh;

/// Marker for slider ball circle entities
#[derive(Component)]
pub struct SliderBallMesh;

/// Marker for circle mesh entities  
#[derive(Component)]
pub struct CircleMesh;

/// Marker for combo number text entities
#[derive(Component)]
pub struct ComboNumberText {
    pub object_index: usize,
}

/// Marker for arrow mesh entities
#[derive(Component)]
pub struct ArrowMesh;

/// Which position an arrow is placed at
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ArrowPosition {
    Start,
    End,
}

/// Component tracking arrow entity data
#[derive(Component)]
pub struct ArrowEntity {
    pub object_index: usize,
    pub position: ArrowPosition,
}

/// Resource to track currently spawned SDF objects
#[derive(Resource, Default)]
pub struct SdfRenderState {
    /// Indices of currently spawned slider objects
    pub spawned_sliders: Vec<usize>,
    /// Indices of currently spawned slider head circles
    pub spawned_slider_heads: Vec<usize>,
    /// Indices of currently spawned slider tail circles
    pub spawned_slider_tails: Vec<usize>,
    /// Indices of currently spawned slider ball circles
    pub spawned_slider_balls: Vec<usize>,
    /// Indices of currently spawned circle objects
    pub spawned_circles: Vec<usize>,
    /// Indices of currently spawned combo number texts
    pub spawned_combo_texts: Vec<usize>,
    /// Indices of sliders with spawned end arrows
    pub spawned_end_arrows: Vec<usize>,
    /// Indices of sliders with spawned start arrows
    pub spawned_start_arrows: Vec<usize>,
}

/// Plugin for SDF rendering system
pub struct SdfRenderPlugin;

impl Plugin for SdfRenderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SdfRenderState>()
            .add_systems(Update, (
                spawn_sdf_objects,
                update_sdf_materials,
                despawn_invisible_objects,
            ).chain());
    }
}

/// Spawn SDF mesh entities for newly visible objects
fn spawn_sdf_objects(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut slider_materials: ResMut<Assets<SliderMaterial>>,
    mut circle_materials: ResMut<Assets<CircleMaterial>>,
    mut arrow_materials: ResMut<Assets<ArrowMaterial>>,
    beatmap: Res<BeatmapView>,
    playback: Res<PlaybackStateRes>,
    transform: Res<PlayfieldTransform>,
    mut state: ResMut<SdfRenderState>,
    ui_font: Option<Res<UiFont>>,
) {
    // Don't spawn until transform is initialized (first frame has scale = 0)
    if transform.scale <= 0.0 {
        return;
    }

    let current_time = playback.current_time;
    let visible = beatmap.visible_objects(current_time);
    let radius = transform.scale_radius(beatmap.circle_radius);

    for (idx, obj, opacity) in visible.iter() {
        match &obj.kind {
        RenderObjectKind::Slider { path_points, repeats, .. } => {
                if !state.spawned_sliders.contains(idx) {
                    spawn_slider(
                        &mut commands,
                        &mut meshes,
                        &mut slider_materials,
                        *idx,
                        obj,
                        path_points,
                        radius,
                        *opacity,
                        &transform,
                        &beatmap,
                    );
                    state.spawned_sliders.push(*idx);
                }
                // Spawn slider head circle for approach circle
                if !state.spawned_slider_heads.contains(idx) {
                    spawn_slider_head(
                        &mut commands,
                        &mut meshes,
                        &mut circle_materials,
                        *idx,
                        obj,
                        radius,
                        *opacity,
                        current_time,
                        &beatmap,
                        &transform,
                    );
                    state.spawned_slider_heads.push(*idx);
                }
                // Spawn slider tail circle
                if !state.spawned_slider_tails.contains(idx) {
                    spawn_slider_tail(
                        &mut commands,
                        &mut meshes,
                        &mut circle_materials,
                        *idx,
                        obj,
                        path_points,
                        radius,
                        *opacity,
                        &beatmap,
                        &transform,
                    );
                    state.spawned_slider_tails.push(*idx);
                }
                // Spawn slider ball circle
                if !state.spawned_slider_balls.contains(idx) {
                    spawn_slider_ball(
                        &mut commands,
                        &mut meshes,
                        &mut circle_materials,
                        *idx,
                        obj,
                        radius,
                        *opacity,
                        current_time,
                        &beatmap,
                        &transform,
                    );
                    state.spawned_slider_balls.push(*idx);
                }
                // Spawn combo text for slider
                if !state.spawned_combo_texts.contains(idx) {
                    if let Some(ref font) = ui_font {
                        spawn_combo_text(&mut commands, *idx, obj, radius, &transform, font);
                        state.spawned_combo_texts.push(*idx);
                    }
                }
                // Spawn arrows for sliders with repeats
                if *repeats > 0 && path_points.len() >= 2 {
                    // End arrow (pointing toward start)
                    if !state.spawned_end_arrows.contains(idx) {
                        let end = path_points.last().unwrap();
                        let prev = &path_points[path_points.len() - 2];
                        let end_pos = transform.osu_to_screen(end.0, end.1);
                        let prev_pos = transform.osu_to_screen(prev.0, prev.1);
                        let direction = prev_pos - end_pos;  // Points toward start
                        spawn_arrow(
                            &mut commands, &mut meshes, &mut arrow_materials,
                            *idx, ArrowPosition::End, end_pos, direction,
                            radius * 0.6, *opacity,
                        );
                        state.spawned_end_arrows.push(*idx);
                    }
                    // Start arrow (pointing toward end) - only for 2+ repeats
                    if *repeats >= 2 && !state.spawned_start_arrows.contains(idx) {
                        let start = &path_points[0];
                        let next = &path_points[1];
                        let start_pos = transform.osu_to_screen(start.0, start.1);
                        let next_pos = transform.osu_to_screen(next.0, next.1);
                        let direction = next_pos - start_pos;  // Points toward end
                        spawn_arrow(
                            &mut commands, &mut meshes, &mut arrow_materials,
                            *idx, ArrowPosition::Start, start_pos, direction,
                            radius * 0.6, *opacity,
                        );
                        state.spawned_start_arrows.push(*idx);
                    }
                }
            }
            RenderObjectKind::Circle => {
                if !state.spawned_circles.contains(idx) {
                    spawn_circle(
                        &mut commands,
                        &mut meshes,
                        &mut circle_materials,
                        *idx,
                        obj,
                        radius,
                        *opacity,
                        current_time,
                        &beatmap,
                        &transform,
                    );
                    state.spawned_circles.push(*idx);
                }
                // Spawn combo text for circle
                if !state.spawned_combo_texts.contains(idx) {
                    if let Some(ref font) = ui_font {
                        spawn_combo_text(&mut commands, *idx, obj, radius, &transform, font);
                        state.spawned_combo_texts.push(*idx);
                    }
                }
            }
            RenderObjectKind::Spinner { .. } => {
                // Spinners use gizmo rendering for now
            }
        }
    }
}

/// Spawn a combo number text entity
fn spawn_combo_text(
    commands: &mut Commands,
    index: usize,
    obj: &RenderObject,
    radius: f32,
    transform: &PlayfieldTransform,
    font: &Res<UiFont>,
) {
    let pos = transform.osu_to_screen(obj.x, obj.y);
    let text_size = radius * 0.8;  // Text size relative to circle
    
    // Z-ordering: text on very top (above all SDF materials)
    let z = 0.0 - (index as f32 * 0.0001);  // Slight z variation per object
    
    commands.spawn((
        Text2d::new(obj.combo_number.to_string()),
        TextFont {
            font: font.0.clone(),
            font_size: text_size,
            ..default()
        },
        TextColor(Color::WHITE),
        Transform::from_xyz(pos.x, pos.y, z),
        ComboNumberText { object_index: index },
    ));
}

/// Spawn a reverse arrow entity at a slider endpoint
fn spawn_arrow(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<ArrowMaterial>>,
    index: usize,
    position: ArrowPosition,
    arrow_pos: Vec2,
    direction: Vec2,  // Direction arrow points toward
    size: f32,
    opacity: f32,
) {
    // Create square mesh for arrow
    let mesh_size = size * 3.0;  // Mesh needs to be larger than arrow
    let mesh = Mesh::from(Rectangle::new(mesh_size, mesh_size));
    let mesh_handle = meshes.add(mesh);

    let material = ArrowMaterial {
        uniforms: ArrowUniforms {
            color: Color::WHITE.into(),
            center: arrow_pos,
            size,
            direction: direction.normalize_or_zero(),
            thickness: size * 0.03,
            opacity,
            _padding: Vec2::ZERO,
        },
    };
    let material_handle = materials.add(material);

    // Z-ordering: arrows on top of circles but below combo text
    let z = -0.4 - (index as f32 * 0.001);

    commands.spawn((
        Mesh2d(mesh_handle),
        MeshMaterial2d(material_handle),
        Transform::from_xyz(arrow_pos.x, arrow_pos.y, z),
        ArrowMesh,
        ArrowEntity { object_index: index, position },
    ));
}

fn spawn_slider(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<SliderMaterial>>,
    index: usize,
    obj: &RenderObject,
    path_points: &[(f32, f32)],
    radius: f32,
    opacity: f32,
    transform: &PlayfieldTransform,
    beatmap: &BeatmapView,
) {
    // Transform path points to screen space
    let screen_points: Vec<(f32, f32)> = path_points
        .iter()
        .map(|(x, y)| {
            let pos = transform.osu_to_screen(*x, *y);
            (pos.x, pos.y)
        })
        .collect();

    if screen_points.len() < 2 {
        return;
    }

    // Calculate bounding box in screen space
    let (mut min_x, mut min_y) = (f32::MAX, f32::MAX);
    let (mut max_x, mut max_y) = (f32::MIN, f32::MIN);
    for &(x, y) in &screen_points {
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        max_x = max_x.max(x);
        max_y = max_y.max(y);
    }

    // Expand by radius for the thick line
    let padding = radius + 5.0;
    let bbox_min = Vec2::new(min_x - padding, min_y - padding);
    let bbox_max = Vec2::new(max_x + padding, max_y + padding);
    let bbox_size = bbox_max - bbox_min;
    let bbox_center = (bbox_min + bbox_max) / 2.0;

    // Create mesh covering the bounding box
    let mesh = Mesh::from(Rectangle::new(bbox_size.x, bbox_size.y));
    let mesh_handle = meshes.add(mesh);

    // Pack path data for shader
    let mut path_data = SliderPathData::default();
    let count = screen_points.len().min(128);
    for i in 0..count {
        let vec_idx = i / 2;
        let (x, y) = screen_points[i];
        if i % 2 == 0 {
            path_data.points[vec_idx].x = x;
            path_data.points[vec_idx].y = y;
        } else {
            path_data.points[vec_idx].z = x;
            path_data.points[vec_idx].w = y;
        }
    }

    // Get combo color for this object
    let (r, g, b) = beatmap.combo_color(obj);
    
    // Create material - fully transparent body, combo-colored border
    let body_color = Color::srgba(0.0, 0.0, 0.0, 0.0);  // Fully transparent
    let border_color = Color::srgba(r, g, b, 1.0);  // Combo color border

    let material = SliderMaterial {
        uniforms: SliderUniforms {
            body_color: body_color.into(),
            border_color: border_color.into(),
            radius,
            border_width: radius * 0.05,
            opacity,
            point_count: count as u32,
            bbox_min,
            bbox_size,
        },
        path_data,
    };
    let material_handle = materials.add(material);

    // Z-ordering: later objects should be behind (lower z)
    let z = -1.0 - (index as f32 * 0.001);

    commands.spawn((
        Mesh2d(mesh_handle),
        MeshMaterial2d(material_handle),
        Transform::from_xyz(bbox_center.x, bbox_center.y, z),
        SdfHitObject { object_index: index },
        SliderMesh,
    ));
}

/// Spawn a slider head circle entity (for approach circle only)
fn spawn_slider_head(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<CircleMaterial>>,
    index: usize,
    obj: &RenderObject,
    radius: f32,
    opacity: f32,
    current_time: f64,
    beatmap: &BeatmapView,
    transform: &PlayfieldTransform,
) {
    let pos = transform.osu_to_screen(obj.x, obj.y);
    
    // Approach circle scale
    let approach_scale = beatmap.approach_scale(obj, current_time);
    
    // Mesh size needs to cover approach circle at maximum scale
    let max_radius = radius * approach_scale.max(4.0) + 10.0;
    let mesh = Mesh::from(Rectangle::new(max_radius * 2.0, max_radius * 2.0));
    let mesh_handle = meshes.add(mesh);

    // Get combo color
    let (r, g, b) = beatmap.combo_color(obj);
    
    // Slider head: visible border circle + approach circle
    let body_color = Color::srgba(0.0, 0.0, 0.0, 0.0);  // Transparent body
    let border_color = Color::srgba(r, g, b, 1.0);  // Combo color border
    let approach_color = Color::srgba(r, g, b, 1.0);  // Combo color approach ring

    let border_width = radius * 0.05;

    let material = CircleMaterial {
        uniforms: CircleUniforms {
            body_color: body_color.into(),
            border_color: border_color.into(),
            approach_color: approach_color.into(),
            radius,
            border_width,
            approach_scale,
            approach_width: 2.5,  // Thin approach circle ring
            opacity,
            center: pos,
        },
    };
    let material_handle = materials.add(material);

    // Z-ordering: slider head should be slightly in front of slider body
    let z = -0.9 - (index as f32 * 0.001);

    commands.spawn((
        Mesh2d(mesh_handle),
        MeshMaterial2d(material_handle),
        Transform::from_xyz(pos.x, pos.y, z),
        SdfHitObject { object_index: index },
        SliderHeadMesh,
    ));
}

/// Spawn a slider tail circle entity (end cap)
fn spawn_slider_tail(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<CircleMaterial>>,
    index: usize,
    obj: &RenderObject,
    path_points: &[(f32, f32)],
    radius: f32,
    opacity: f32,
    beatmap: &BeatmapView,
    transform: &PlayfieldTransform,
) {
    // Get tail position from last path point
    let (tail_x, tail_y) = match path_points.last() {
        Some(&pos) => pos,
        None => return,
    };
    let pos = transform.osu_to_screen(tail_x, tail_y);
    
    // Mesh size for tail circle
    let max_radius = radius + 10.0;
    let mesh = Mesh::from(Rectangle::new(max_radius * 2.0, max_radius * 2.0));
    let mesh_handle = meshes.add(mesh);

    // Get combo color
    let (r, g, b) = beatmap.combo_color(obj);
    
    // Tail circle: visible border, transparent body, no approach circle
    let body_color = Color::srgba(0.0, 0.0, 0.0, 0.0);  // Transparent body
    let border_color = Color::srgba(r, g, b, 1.0);  // Combo color border
    let approach_color = Color::srgba(0.0, 0.0, 0.0, 0.0);  // No approach circle

    let border_width = radius * 0.05;

    let material = CircleMaterial {
        uniforms: CircleUniforms {
            body_color: body_color.into(),
            border_color: border_color.into(),
            approach_color: approach_color.into(),
            radius,
            border_width,
            approach_scale: 1.0,  // No approach circle
            approach_width: 0.0,
            opacity,
            center: pos,
        },
    };
    let material_handle = materials.add(material);

    // Z-ordering: tail slightly behind head
    let z = -0.95 - (index as f32 * 0.001);

    commands.spawn((
        Mesh2d(mesh_handle),
        MeshMaterial2d(material_handle),
        Transform::from_xyz(pos.x, pos.y, z),
        SdfHitObject { object_index: index },
        SliderTailMesh,
    ));
}

/// Spawn a slider ball circle entity
fn spawn_slider_ball(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<CircleMaterial>>,
    index: usize,
    obj: &RenderObject,
    radius: f32,
    opacity: f32,
    current_time: f64,
    beatmap: &BeatmapView,
    transform: &PlayfieldTransform,
) {
    // Get ball position (or head position if not active yet)
    let (ball_x, ball_y) = beatmap.slider_ball_position(obj, current_time)
        .unwrap_or((obj.x, obj.y));
    let pos = transform.osu_to_screen(ball_x, ball_y);
    
    // Ball is smaller than the circle
    let ball_radius = radius * 0.6;
    let max_radius = ball_radius + 10.0;
    let mesh = Mesh::from(Rectangle::new(max_radius * 2.0, max_radius * 2.0));
    let mesh_handle = meshes.add(mesh);

    // Get combo color
    let (r, g, b) = beatmap.combo_color(obj);
    
    // Ball: solid colored circle
    let body_color = Color::srgba(r * 0.3, g * 0.3, b * 0.3, 0.8);  // Darker fill
    let border_color = Color::srgba(r, g, b, 1.0);  // Combo color border
    let approach_color = Color::srgba(0.0, 0.0, 0.0, 0.0);  // No approach

    let border_width = ball_radius * 0.1;

    // Ball is only visible during active slider
    let ball_visible = beatmap.slider_ball_position(obj, current_time).is_some();
    let ball_opacity = if ball_visible { opacity } else { 0.0 };

    let material = CircleMaterial {
        uniforms: CircleUniforms {
            body_color: body_color.into(),
            border_color: border_color.into(),
            approach_color: approach_color.into(),
            radius: ball_radius,
            border_width,
            approach_scale: 1.0,
            approach_width: 0.0,
            opacity: ball_opacity,
            center: pos,
        },
    };
    let material_handle = materials.add(material);

    // Z-ordering: ball on top
    let z = -0.5 - (index as f32 * 0.001);

    commands.spawn((
        Mesh2d(mesh_handle),
        MeshMaterial2d(material_handle),
        Transform::from_xyz(pos.x, pos.y, z),
        SdfHitObject { object_index: index },
        SliderBallMesh,
    ));
}

/// Spawn a circle mesh entity
fn spawn_circle(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<CircleMaterial>>,
    index: usize,
    obj: &RenderObject,
    radius: f32,
    opacity: f32,
    current_time: f64,
    beatmap: &BeatmapView,
    transform: &PlayfieldTransform,
) {
    let pos = transform.osu_to_screen(obj.x, obj.y);
    
    // Approach circle scale
    let approach_scale = beatmap.approach_scale(obj, current_time);
    
    // Mesh size needs to cover approach circle at maximum scale
    let max_radius = radius * approach_scale.max(4.0) + 10.0;
    let mesh = Mesh::from(Rectangle::new(max_radius * 2.0, max_radius * 2.0));
    let mesh_handle = meshes.add(mesh);

    // Get combo color for this object
    let (r, g, b) = beatmap.combo_color(obj);
    
    // Create material - fully transparent body, combo-colored border
    let body_color = Color::srgba(0.0, 0.0, 0.0, 0.0);  // Fully transparent
    let border_color = Color::srgba(r, g, b, 1.0);  // Combo color border
    let approach_color = Color::srgba(r, g, b, 1.0);  // Combo color approach ring

    // Border width - match what slider uses
    let border_width = radius * 0.05;

    let material = CircleMaterial {
        uniforms: CircleUniforms {
            body_color: body_color.into(),
            border_color: border_color.into(),
            approach_color: approach_color.into(),
            radius,
            border_width,
            approach_scale,
            approach_width: 2.5,  // Thin approach circle ring
            opacity,
            center: pos,
        },
    };
    let material_handle = materials.add(material);


    // Z-ordering: later objects should be behind (lower z)
    let z = -1.0 - (index as f32 * 0.001);

    commands.spawn((
        Mesh2d(mesh_handle),
        MeshMaterial2d(material_handle),
        Transform::from_xyz(pos.x, pos.y, z),
        SdfHitObject { object_index: index },
        CircleMesh,
    ));
}

/// Update materials for existing SDF objects (opacity, approach scale, etc.)
fn update_sdf_materials(
    beatmap: Res<BeatmapView>,
    playback: Res<PlaybackStateRes>,
    transform: Res<PlayfieldTransform>,
    mut circle_materials: ResMut<Assets<CircleMaterial>>,
    mut slider_materials: ResMut<Assets<SliderMaterial>>,
    circle_query: Query<(&SdfHitObject, &MeshMaterial2d<CircleMaterial>), With<CircleMesh>>,
    slider_head_query: Query<(&SdfHitObject, &MeshMaterial2d<CircleMaterial>), With<SliderHeadMesh>>,
    slider_tail_query: Query<(&SdfHitObject, &MeshMaterial2d<CircleMaterial>), With<SliderTailMesh>>,
    mut slider_ball_query: Query<(&SdfHitObject, &MeshMaterial2d<CircleMaterial>, &mut Transform), With<SliderBallMesh>>,
    slider_query: Query<(&SdfHitObject, &MeshMaterial2d<SliderMaterial>), With<SliderMesh>>,
) {
    let current_time = playback.current_time;
    let visible = beatmap.visible_objects(current_time);
    
    // Build a map of visible objects for quick lookup
    let visible_map: std::collections::HashMap<usize, f32> = visible
        .iter()
        .map(|(idx, _obj, opacity)| (*idx, *opacity))
        .collect();

    // Update circle materials
    for (hit_obj, material_handle) in circle_query.iter() {
        if let Some(&opacity) = visible_map.get(&hit_obj.object_index) {
            if let Some(material) = circle_materials.get_mut(material_handle.id()) {
                material.uniforms.opacity = opacity;
                
                // Update approach scale
                if let Some(obj) = beatmap.objects.get(hit_obj.object_index) {
                    material.uniforms.approach_scale = beatmap.approach_scale(obj, current_time);
                }
            }
        }
    }

    // Update slider head circle materials (for approach circles)
    for (hit_obj, material_handle) in slider_head_query.iter() {
        if let Some(&opacity) = visible_map.get(&hit_obj.object_index) {
            if let Some(material) = circle_materials.get_mut(material_handle.id()) {
                material.uniforms.opacity = opacity;
                
                // Update approach scale for slider head
                if let Some(obj) = beatmap.objects.get(hit_obj.object_index) {
                    material.uniforms.approach_scale = beatmap.approach_scale(obj, current_time);
                }
            }
        }
    }

    // Update slider tail circle materials
    for (hit_obj, material_handle) in slider_tail_query.iter() {
        if let Some(&opacity) = visible_map.get(&hit_obj.object_index) {
            if let Some(material) = circle_materials.get_mut(material_handle.id()) {
                material.uniforms.opacity = opacity;
            }
        }
    }

    // Update slider ball materials AND transform position
    for (hit_obj, material_handle, mut ball_transform) in slider_ball_query.iter_mut() {
        if let Some(&opacity) = visible_map.get(&hit_obj.object_index) {
            if let Some(material) = circle_materials.get_mut(material_handle.id()) {
                if let Some(obj) = beatmap.objects.get(hit_obj.object_index) {
                    // Update ball position
                    if let Some((ball_x, ball_y)) = beatmap.slider_ball_position(obj, current_time) {
                        let pos = transform.osu_to_screen(ball_x, ball_y);
                        material.uniforms.center = pos;
                        material.uniforms.opacity = opacity;
                        // Move the mesh quad to follow the ball
                        ball_transform.translation.x = pos.x;
                        ball_transform.translation.y = pos.y;
                    } else {
                        // Ball not visible (slider not active)
                        material.uniforms.opacity = 0.0;
                    }
                }
            }
        }
    }

    // Update slider materials
    for (hit_obj, material_handle) in slider_query.iter() {
        if let Some(&opacity) = visible_map.get(&hit_obj.object_index) {
            if let Some(material) = slider_materials.get_mut(material_handle.id()) {
                material.uniforms.opacity = opacity;
            }
        }
    }
}

/// Despawn objects that are no longer visible
fn despawn_invisible_objects(
    mut commands: Commands,
    beatmap: Res<BeatmapView>,
    playback: Res<PlaybackStateRes>,
    mut state: ResMut<SdfRenderState>,
    query: Query<(Entity, &SdfHitObject)>,
    combo_text_query: Query<(Entity, &ComboNumberText)>,
    arrow_query: Query<(Entity, &ArrowEntity)>,
) {
    let current_time = playback.current_time;
    let visible = beatmap.visible_objects(current_time);
    let visible_indices: std::collections::HashSet<usize> = visible
        .iter()
        .map(|(idx, _, _)| *idx)
        .collect();

    for (entity, hit_obj) in query.iter() {
        if !visible_indices.contains(&hit_obj.object_index) {
            commands.entity(entity).despawn();
            
            // Remove from state tracking
            state.spawned_sliders.retain(|&i| i != hit_obj.object_index);
            state.spawned_slider_heads.retain(|&i| i != hit_obj.object_index);
            state.spawned_slider_tails.retain(|&i| i != hit_obj.object_index);
            state.spawned_slider_balls.retain(|&i| i != hit_obj.object_index);
            state.spawned_circles.retain(|&i| i != hit_obj.object_index);
        }
    }

    // Despawn combo texts separately
    for (entity, combo_text) in combo_text_query.iter() {
        if !visible_indices.contains(&combo_text.object_index) {
            commands.entity(entity).despawn();
            state.spawned_combo_texts.retain(|&i| i != combo_text.object_index);
        }
    }

    // Despawn arrows separately
    for (entity, arrow) in arrow_query.iter() {
        if !visible_indices.contains(&arrow.object_index) {
            commands.entity(entity).despawn();
            match arrow.position {
                ArrowPosition::End => state.spawned_end_arrows.retain(|&i| i != arrow.object_index),
                ArrowPosition::Start => state.spawned_start_arrows.retain(|&i| i != arrow.object_index),
            }
        }
    }
}
