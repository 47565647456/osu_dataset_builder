//! SDF-based rendering system for hit objects
//! Spawns and manages mesh entities with SDF materials for sliders and circles

use bevy::prelude::*;
use bevy::sprite_render::MeshMaterial2d;

use crate::beatmap::{BeatmapView, RenderObject, RenderObjectKind};
use crate::playback::PlaybackStateRes;
use crate::rendering::sdf_materials::{CircleMaterial, CircleUniforms, SliderMaterial, SliderPathData, SliderUniforms};
use crate::rendering::PlayfieldTransform;

/// Marker component for SDF-rendered hit objects
#[derive(Component)]
pub struct SdfHitObject {
    /// Index into beatmap objects array
    pub object_index: usize,
}

/// Marker for slider mesh entities
#[derive(Component)]
pub struct SliderMesh;

/// Marker for circle mesh entities  
#[derive(Component)]
pub struct CircleMesh;

/// Resource to track currently spawned SDF objects
#[derive(Resource, Default)]
pub struct SdfRenderState {
    /// Indices of currently spawned slider objects
    pub spawned_sliders: Vec<usize>,
    /// Indices of currently spawned circle objects
    pub spawned_circles: Vec<usize>,
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
    beatmap: Res<BeatmapView>,
    playback: Res<PlaybackStateRes>,
    transform: Res<PlayfieldTransform>,
    mut state: ResMut<SdfRenderState>,
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
        RenderObjectKind::Slider { path_points, .. } => {
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
            }
            RenderObjectKind::Spinner { .. } => {
                // Spinners use gizmo rendering for now
            }
        }
    }
}

/// Spawn a slider mesh entity
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
    let z = -1.0 - (index as f32 * 0.01);

    commands.spawn((
        Mesh2d(mesh_handle),
        MeshMaterial2d(material_handle),
        Transform::from_xyz(bbox_center.x, bbox_center.y, z),
        SdfHitObject { object_index: index },
        SliderMesh,
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
            approach_width: border_width,  // Match approach circle width to border width
            opacity,
            center: pos,
        },
    };
    let material_handle = materials.add(material);


    // Z-ordering: later objects should be behind (lower z)
    let z = -1.0 - (index as f32 * 0.01);

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
    mut circle_materials: ResMut<Assets<CircleMaterial>>,
    mut slider_materials: ResMut<Assets<SliderMaterial>>,
    circle_query: Query<(&SdfHitObject, &MeshMaterial2d<CircleMaterial>), With<CircleMesh>>,
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
            state.spawned_circles.retain(|&i| i != hit_obj.object_index);
        }
    }
}
