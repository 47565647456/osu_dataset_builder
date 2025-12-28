//! SDF-based rendering system for hit objects
//! Spawns and manages mesh entities with SDF materials for sliders and circles

use bevy::prelude::*;
use bevy::sprite_render::MeshMaterial2d;

use crate::beatmap::{BeatmapView, RenderObject, RenderObjectKind, PLAYFIELD_HEIGHT, PLAYFIELD_WIDTH};
use crate::playback::PlaybackStateRes;
use crate::rendering::sdf_materials::{
    ArrowMaterial, ArrowUniforms, MsdfMaterial, SliderMaterial, SliderPathData, SliderUniforms, SpinnerMaterial, SpinnerUniforms,
    CircleBatchMaterial, MsdfBatchMaterial,
    ATTRIBUTE_BODY_COLOR, ATTRIBUTE_BORDER_COLOR, ATTRIBUTE_APPROACH_COLOR, ATTRIBUTE_SDF_PARAMS,
    ATTRIBUTE_MSDF_UV_BOUNDS, ATTRIBUTE_MSDF_PARAMS
};
use crate::rendering::PlayfieldTransform;
use bevy::render::render_resource::PrimitiveTopology;
use bevy::asset::RenderAssetUsages;
use bevy::mesh::Indices;

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
// (Marker structs removed as hit circles are now batched entity-less)

/// Marker for arrow mesh entities
#[derive(Component)]
pub struct ArrowMesh;


/// Component tracking arrow entity data
#[derive(Component)]
pub struct ArrowEntity {
    pub object_index: usize,
}

/// Marker for spinner mesh entities
#[derive(Component)]
pub struct SpinnerMesh;

/// Resource to track currently spawned SDF objects and shared resources
#[derive(Resource)]
pub struct SdfRenderState {
    /// Shared unit quad mesh (1x1)
    pub unit_mesh: Handle<Mesh>,
    /// Mesh for batched circles (updated every frame)
    pub circle_batch_mesh: Handle<Mesh>,
    /// Mesh for batched MSDF digits (updated every frame)
    pub msdf_batch_mesh: Handle<Mesh>,
    
    /// Cache for shared arrow materials by opacity
    pub arrow_cache: std::collections::HashMap<u32, Handle<ArrowMaterial>>,
    
    /// Indices of currently spawned slider objects
    pub spawned_sliders: Vec<usize>,
    /// Indices of sliders with spawned end arrows
    pub spawned_end_arrows: Vec<usize>,
    /// Indices of sliders with spawned start arrows
    pub spawned_start_arrows: Vec<usize>,
    /// Indices of currently spawned spinners
    pub spawned_spinners: Vec<usize>,
    /// Current vertex capacity for circle batch (number of quads)
    pub circle_capacity: usize,
    /// Current vertex capacity for MSDF batch (number of quads)
    pub msdf_capacity: usize,
    /// Last seen transform generation (for detecting resize/zoom changes)
    pub last_generation: u32,
}

impl FromWorld for SdfRenderState {
    fn from_world(world: &mut World) -> Self {
        let mut meshes = world.resource_mut::<Assets<Mesh>>();
        let unit_mesh = meshes.add(Mesh::from(Rectangle::new(1.0, 1.0)));
        
        // Create empty meshes for batching
        let circle_batch_mesh = meshes.add(Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default()));
        let msdf_batch_mesh = meshes.add(Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default()));

        Self {
            unit_mesh,
            circle_batch_mesh,
            msdf_batch_mesh,
            arrow_cache: default(),
            spawned_sliders: default(),
            spawned_end_arrows: default(),
            spawned_start_arrows: default(),
            spawned_spinners: default(),
            circle_capacity: 0,
            msdf_capacity: 0,
            last_generation: 0,
        }
    }
}
use serde::Deserialize;

/// MSDF font atlas resource with pre-computed glyph UVs
#[derive(Resource)]
pub struct MsdfAtlas {
    pub texture: Handle<Image>,
    /// UV bounds for digits 0-9 (left, bottom, right, top) normalized 0-1
    pub digit_uvs: [Vec4; 10],
    /// Advance width for digits 0-9 (normalized to em size)
    pub digit_advances: [f32; 10],
    /// Aspect ratio (width / height) for digits 0-9
    pub digit_sizes: [Vec2; 10],
    /// Distance range from atlas generation
    pub px_range: f32,
}

impl Default for MsdfAtlas {
    fn default() -> Self {
        Self {
            texture: Handle::default(),
            digit_uvs: [Vec4::ZERO; 10],
            digit_advances: [0.5; 10],
            digit_sizes: [Vec2::ONE; 10],
            px_range: 2.0, 
        }
    }
}

// JSON structures for parsing msdf-atlas-gen output
#[derive(Deserialize)]
struct AtlasBounds { left: f32, bottom: f32, right: f32, top: f32 }

#[derive(Deserialize)]
struct Glyph { 
    unicode: u32,
    advance: f32,
    #[serde(rename = "atlasBounds")]
    atlas_bounds: Option<AtlasBounds> 
}

#[derive(Deserialize)]
struct AtlasInfo {
    #[serde(rename = "distanceRange")]
     distance_range: f32,
     width: f32, 
     height: f32 
}

#[derive(Deserialize)]
struct MsdfJson { atlas: AtlasInfo, glyphs: Vec<Glyph> }

/// Plugin for SDF rendering system
pub struct SdfRenderPlugin;

impl Plugin for SdfRenderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SdfRenderState>()
            .init_resource::<MsdfAtlas>()
            .add_systems(Startup, (setup_msdf_atlas, setup_batch_entities).chain())
            .add_systems(Update, (
                clear_on_transform_change,
                spawn_sdf_objects,
                update_non_batched_materials,
                despawn_invisible_objects,
            ).chain())
            .add_systems(PostUpdate, (
                update_circle_batches,
                update_msdf_batches,
            ).after(bevy::transform::TransformSystems::Propagate));
    }
}

/// Helper components for batch entities
#[derive(Component)]
pub struct CircleBatchMarker;
#[derive(Component)]
pub struct MsdfBatchMarker;

/// Spawn the persistent batch entities
fn setup_batch_entities(
    mut commands: Commands,
    state: Res<SdfRenderState>,
    mut circle_batch_materials: ResMut<Assets<CircleBatchMaterial>>,
    mut msdf_batch_materials: ResMut<Assets<MsdfBatchMaterial>>,
    atlas: Res<MsdfAtlas>,
) {
    commands.spawn((
        Mesh2d(state.circle_batch_mesh.clone()),
        MeshMaterial2d(circle_batch_materials.add(CircleBatchMaterial::default())),
        CircleBatchMarker,
        Transform::from_xyz(0.0, 0.0, 0.1), // Slightly in front
        Visibility::Visible,
    ));

    commands.spawn((
        Mesh2d(state.msdf_batch_mesh.clone()),
        MeshMaterial2d(msdf_batch_materials.add(MsdfBatchMaterial {
            uniforms: default(),
            texture: atlas.texture.clone(),
        })),
        MsdfBatchMarker,
        Transform::from_xyz(0.0, 0.0, 0.5), // High Z for digits overlay
        Visibility::Visible,
    ));
}

/// Load MSDF atlas texture and JSON metadata at startup
fn setup_msdf_atlas(
    asset_server: Res<AssetServer>,
    mut atlas: ResMut<MsdfAtlas>,
) {
    atlas.texture = asset_server.load("fonts/digits_msdf.png");
    
    // Load and parse JSON metadata
    // Using std::fs for simplicity since we don't need hot-reloading for metrics
    if let Ok(json_str) = std::fs::read_to_string("assets/fonts/digits_msdf.json") {
        if let Ok(data) = serde_json::from_str::<MsdfJson>(&json_str) {
            let width = data.atlas.width;
            let height = data.atlas.height;
            
            for glyph in data.glyphs {
                // Check if it's a digit 0-9 (unicode 48-57)
                if glyph.unicode >= 48 && glyph.unicode <= 57 {
                    let index = (glyph.unicode - 48) as usize;
                    if let Some(bounds) = glyph.atlas_bounds {
                        // Convert to normalized UV coordinates (0-1)
                        // JSON is yOrigin: bottom (Y-up), so we need to flip for GPU (Top-Down)
                        atlas.digit_uvs[index] = Vec4::new(
                            bounds.left / width,
                            1.0 - (bounds.top / height),      // Top edge in Y-up is small Y in Top-Down
                            bounds.right / width,
                            1.0 - (bounds.bottom / height)    // Bottom edge in Y-up is large Y in Top-Down
                        );

                        // Calculate aspect ratio from atlas bounds
                        let w = bounds.right - bounds.left;
                        let h = (bounds.top - bounds.bottom).abs();
                        let aspect = if h > 0.001 { w / h } else { 1.0 };
                        atlas.digit_sizes[index] = Vec2::new(aspect, 1.0);
                    }
                    atlas.digit_advances[index] = glyph.advance;
                }
            }
            
            // Use the distance range from the JSON (should be 2.0)
            atlas.px_range = data.atlas.distance_range;
            
            log::info!("Loaded MSDF atlas metadata for digits 0-9 (px_range: {})", atlas.px_range);
        } else {
            log::error!("Failed to parse digits_msdf.json");
        }
    } else {
        log::error!("Failed to read digits_msdf.json");
    }
}

/// Clear all spawned state when transform changes (resize/zoom)
fn clear_on_transform_change(
    mut commands: Commands,
    transform: Res<PlayfieldTransform>,
    mut state: ResMut<SdfRenderState>,
    query: Query<Entity, Or<(With<SdfHitObject>, With<ArrowEntity>)>>,
) {
    if state.last_generation != transform.generation {
        // Transform changed - despawn all and clear state
        for entity in query.iter() {
            commands.entity(entity).despawn();
        }
        
        state.spawned_sliders.clear();
        state.spawned_end_arrows.clear();
        state.spawned_start_arrows.clear();
        state.spawned_spinners.clear();
        state.last_generation = transform.generation;
    }
}

/// Spawn SDF mesh entities for newly visible objects
fn spawn_sdf_objects(
    mut commands: Commands,
    mut slider_materials: ResMut<Assets<SliderMaterial>>,
    mut arrow_materials: ResMut<Assets<ArrowMaterial>>,
    mut spinner_materials: ResMut<Assets<SpinnerMaterial>>,
    _msdf_materials: ResMut<Assets<MsdfMaterial>>, // Keep for potential future use or if other MSDF entities are added
    _atlas: Res<MsdfAtlas>, // Keep for potential future use
    beatmap: Res<BeatmapView>,
    playback: Res<PlaybackStateRes>,
    transform: Res<PlayfieldTransform>,
    mut state_res: ResMut<SdfRenderState>,
) {
    let state = &mut *state_res;
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
                    spawn_slider(&mut commands, state, &mut slider_materials, *idx, obj, path_points, radius, *opacity, &transform, &beatmap);
                    state.spawned_sliders.push(*idx);
                }
                if *repeats > 0 && path_points.len() >= 2 {
                    if !state.spawned_end_arrows.contains(idx) {
                        let end = path_points.last().unwrap();
                        let prev = &path_points[path_points.len() - 2];
                        let end_pos = transform.osu_to_screen(end.0, end.1);
                        let prev_pos = transform.osu_to_screen(prev.0, prev.1);
                        let direction = prev_pos - end_pos;
                        spawn_arrow(&mut commands, state, &mut arrow_materials, *idx, end_pos, direction, radius * 0.6, *opacity);
                        state.spawned_end_arrows.push(*idx);
                    }
                    if *repeats >= 2 && !state.spawned_start_arrows.contains(idx) {
                        let start = &path_points[0];
                        let next = &path_points[1];
                        let start_pos = transform.osu_to_screen(start.0, start.1);
                        let next_pos = transform.osu_to_screen(next.0, next.1);
                        let direction = next_pos - start_pos;
                        spawn_arrow(&mut commands, state, &mut arrow_materials, *idx, start_pos, direction, radius * 0.6, *opacity);
                        state.spawned_start_arrows.push(*idx);
                    }
                }
            }
            RenderObjectKind::Circle => {}
            RenderObjectKind::Spinner { duration } => {
                if !state.spawned_spinners.contains(idx) {
                    spawn_spinner(&mut commands, state, &mut spinner_materials, *idx, obj, *duration, *opacity, current_time, &transform);
                    state.spawned_spinners.push(*idx);
                }
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
            state.spawned_spinners.retain(|&i| i != hit_obj.object_index);
        }
    }

    // Despawn arrows separately
    for (entity, arrow) in arrow_query.iter() {
        if !visible_indices.contains(&arrow.object_index) {
            commands.entity(entity).despawn();
            state.spawned_end_arrows.retain(|&i| i != arrow.object_index);
            state.spawned_start_arrows.retain(|&i| i != arrow.object_index);
        }
    }
}

/// Update materials for non-batched items (Spinners, Slider Bodies, Arrows)
fn update_non_batched_materials(
    beatmap: Res<BeatmapView>,
    playback: Res<PlaybackStateRes>,
    mut slider_materials: ResMut<Assets<SliderMaterial>>,
    mut spinner_materials: ResMut<Assets<SpinnerMaterial>>,
    mut arrow_materials: ResMut<Assets<ArrowMaterial>>,
    slider_query: Query<(&SdfHitObject, &MeshMaterial2d<SliderMaterial>), With<SliderMesh>>,
    spinner_query: Query<(&SdfHitObject, &MeshMaterial2d<SpinnerMaterial>), With<SpinnerMesh>>,
    arrow_query: Query<(&ArrowEntity, &MeshMaterial2d<ArrowMaterial>)>,
) {
    let current_time = playback.current_time;
    let visible = beatmap.visible_objects(current_time);
    let visible_map: std::collections::HashMap<usize, f32> = visible
        .iter()
        .map(|(idx, _, opacity)| (*idx, *opacity))
        .collect();

    for (hit_obj, handle) in slider_query.iter() {
        if let Some(&opacity) = visible_map.get(&hit_obj.object_index) {
            if let Some(mat) = slider_materials.get_mut(handle.id()) {
                mat.uniforms.opacity = opacity;
            }
        }
    }

    for (hit_obj, handle) in spinner_query.iter() {
        if let Some(&opacity) = visible_map.get(&hit_obj.object_index) {
            if let Some(mat) = spinner_materials.get_mut(handle.id()) {
                if let Some(obj) = beatmap.objects.get(hit_obj.object_index) {
                    if let RenderObjectKind::Spinner { duration } = &obj.kind {
                        let elapsed = (current_time - obj.start_time).max(0.0);
                        mat.uniforms.progress = (elapsed / duration).min(1.0) as f32;
                        mat.uniforms.rotation = (current_time / 50.0).to_radians() as f32;
                        mat.uniforms.opacity = opacity;
                    }
                }
            }
        }
    }

    for (arrow, handle) in arrow_query.iter() {
        if let Some(&opacity) = visible_map.get(&arrow.object_index) {
            if let Some(mat) = arrow_materials.get_mut(handle.id()) {
                mat.uniforms.opacity = opacity;
            }
        }
    }
}

fn spawn_arrow(
    commands: &mut Commands,
    state: &mut SdfRenderState,
    materials: &mut ResMut<Assets<ArrowMaterial>>,
    index: usize,
    pos: Vec2,
    direction: Vec2,  // Direction arrow points toward
    radius: f32,
    opacity: f32,
) {
    // Z-ordering: reverse arrows (+0.0005 relative to object base)
    let z = -(index as f32 * 0.001) + 0.0005;
    
    // Use cache for arrow material
    let _opacity_bits = opacity.to_bits();
    let material_handle = state.arrow_cache.entry(_opacity_bits).or_insert_with(|| {
        materials.add(ArrowMaterial {
            uniforms: ArrowUniforms {
                color: Color::WHITE.into(),
                thickness_rel: 0.2,
                opacity,
                _padding: Vec2::ZERO,
            },
        })
    }).clone();

    commands.spawn((
        Mesh2d(state.unit_mesh.clone()),
        MeshMaterial2d(material_handle),
        Transform::from_xyz(pos.x, pos.y, z)
            .with_rotation(Quat::from_rotation_arc(Vec3::Y, direction.extend(0.0).normalize()))
            .with_scale(Vec3::new(radius * 2.0, radius * 2.0, 1.0)),
        ArrowMesh,
        ArrowEntity { object_index: index },
    ));
}

/// Spawn a spinner mesh entity
fn spawn_spinner(
    commands: &mut Commands,
    state: &mut SdfRenderState,
    materials: &mut ResMut<Assets<SpinnerMaterial>>,
    index: usize,
    obj: &RenderObject,
    duration: f64,
    opacity: f32,
    current_time: f64,
    transform: &PlayfieldTransform,
) {
    // Spinner is centered on the playfield
    let center = transform.osu_to_screen(PLAYFIELD_WIDTH / 2.0, PLAYFIELD_HEIGHT / 2.0);
    let max_radius = transform.scale_radius(150.0);
    
    // Calculate initial progress and rotation
    let elapsed = (current_time - obj.start_time).max(0.0);
    let progress = (elapsed / duration).min(1.0) as f32;
    let rotation = (current_time / 50.0).to_radians() as f32;
    
    let material = SpinnerMaterial {
        uniforms: SpinnerUniforms {
            color: Color::WHITE.into(),
            progress,
            rotation,
            opacity,
            _padding: Vec2::ZERO,
        },
    };
    let material_handle = materials.add(material);
    
    // Z-ordering: spinner (+0.0000 relative to object base)
    let z = -(index as f32 * 0.001);
    
    commands.spawn((
        Mesh2d(state.unit_mesh.clone()),
        MeshMaterial2d(material_handle),
        Transform::from_xyz(center.x, center.y, z)
            .with_scale(Vec3::new(max_radius * 2.5, max_radius * 2.5, 1.0)),
        SpinnerMesh,
        SdfHitObject { object_index: index },
    ));
}

fn spawn_slider(
    commands: &mut Commands,
    state: &mut SdfRenderState,
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

    // Z-ordering: slider body (+0.0000 relative to object base)
    let z = -(index as f32 * 0.001);

    commands.spawn((
        Mesh2d(state.unit_mesh.clone()),
        MeshMaterial2d(material_handle),
        Transform::from_xyz(bbox_center.x, bbox_center.y, z)
            .with_scale(Vec3::new(bbox_size.x, bbox_size.y, 1.0)),
        SliderMesh,
        SdfHitObject { object_index: index },
    ));
}

/// Update the circle batch mesh from current entity data
fn update_circle_batches(
    beatmap: Res<BeatmapView>,
    playback: Res<PlaybackStateRes>,
    transform: Res<PlayfieldTransform>,
    mut state: ResMut<SdfRenderState>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    if transform.scale <= 0.0 {
        return;
    }

    if let Some(mesh) = meshes.get_mut(&state.circle_batch_mesh) {
        let current_time = playback.current_time;
        let visible = beatmap.visible_objects(current_time);
        
        let count_estimate = visible.len() * 4; // Max 4 circles per object (slider)
        let mut positions = Vec::with_capacity(count_estimate * 4);
        let mut uvs = Vec::with_capacity(count_estimate * 4);
        let mut indices = Vec::with_capacity(count_estimate * 6);
        let mut body_colors = Vec::with_capacity(count_estimate * 4);
        let mut border_colors = Vec::with_capacity(count_estimate * 4);
        let mut approach_colors = Vec::with_capacity(count_estimate * 4);
        let mut params = Vec::with_capacity(count_estimate * 4);

        let mut quad_count = 0usize;
        let radius = transform.scale_radius(beatmap.circle_radius);
        
        // Expansion factor for border and approach circles
        let expansion = 1.4f32;

        // visible is already sorted by time (which approximates Z in world coords)
        // We iterate and build quads. Since we are building a single mesh,
        // we use Z in the transform or just the hit_obj.object_index for ordering.
        for (index, obj, opacity_val) in visible.iter() {
            let opacity = *opacity_val;
            if opacity < 0.01 { continue; }

            let (r, g, b) = beatmap.combo_color(obj);
            let combo_color = LinearRgba::new(r, g, b, 1.0);
            let body_color = LinearRgba::new(r, g, b, 0.3 * opacity);
            let white_color = LinearRgba::new(1.0, 1.0, 1.0, 1.0).to_f32_array();
            let approach_color = combo_color.to_f32_array();

            // Helper to push a quad
            let mut push_quad = |center: Vec2, current_radius: f32, z: f32, b_col: [f32; 4], br_col: [f32; 4], a_col: [f32; 4], p: [f32; 4]| {
                let base_idx = (quad_count * 4) as u32;
                
                let approach_scale = p[2];
                let max_scale = approach_scale.max(1.0);
                let s = current_radius * expansion * max_scale;
                
                positions.push(Vec3::new(center.x - s, center.y - s, z));
                positions.push(Vec3::new(center.x + s, center.y - s, z));
                positions.push(Vec3::new(center.x + s, center.y + s, z));
                positions.push(Vec3::new(center.x - s, center.y + s, z));

                uvs.push(Vec2::new(0.0, 1.0));
                uvs.push(Vec2::new(1.0, 1.0));
                uvs.push(Vec2::new(1.0, 0.0));
                uvs.push(Vec2::new(0.0, 0.0));

                indices.extend_from_slice(&[base_idx, base_idx + 1, base_idx + 2, base_idx, base_idx + 2, base_idx + 3]);

                for _ in 0..4 {
                    body_colors.push(b_col);
                    border_colors.push(br_col);
                    approach_colors.push(a_col);
                    params.push(p);
                }
                quad_count += 1;
            };

            let z_base = -(*index as f32 * 0.001);

            match &obj.kind {
                RenderObjectKind::Circle => {
                    let screen_pos = transform.osu_to_screen(obj.x, obj.y);
                    let approach_scale = beatmap.approach_scale(obj, current_time);
                    
                    push_quad(
                        screen_pos,
                        radius,
                        z_base,
                        body_color.to_f32_array(),
                        white_color,
                        approach_color,
                        [0.05, 2.5 / radius, approach_scale, opacity]
                    );
                }
                RenderObjectKind::Slider { path_points, .. } => {
                    // Slider Head
                    let head_pos = transform.osu_to_screen(obj.x, obj.y);
                    let approach_scale = beatmap.approach_scale(obj, current_time);
                    push_quad(
                        head_pos,
                        radius,
                        z_base + 0.0001,
                        body_color.to_f32_array(),
                        white_color,
                        approach_color,
                        [0.1, 2.5 / radius, approach_scale, opacity]
                    );

                    // Slider Tail
                    if let Some(tail) = path_points.last() {
                        let tail_pos = transform.osu_to_screen(tail.0, tail.1);
                        push_quad(
                            tail_pos,
                            radius, 
                            z_base + 0.0001,
                            body_color.to_f32_array(),
                            white_color,
                            approach_color,
                            [0.1, 0.0, 1.0, opacity]
                        );
                    }

                    // Slider Ball
                    if let Some((ball_x, ball_y)) = beatmap.slider_ball_position(obj, current_time) {
                        let ball_screen = transform.osu_to_screen(ball_x, ball_y);
                        // Ball should NOT have an approach circle, so p[2] = 1.0
                        push_quad(
                            ball_screen,
                            radius,
                            z_base + 0.0002,
                            body_color.to_f32_array(),
                            white_color,
                            approach_color,
                            [0.1, 0.0, 1.0, opacity]
                        );
                    }
                }
                _ => {}
            }
        }

        if quad_count == 0 {
            // Nothing to render, but we must update state to avoid allocator churn
            // If we don't, the previous frame's geometry might remain.
        }

        // Buffer stabilization: pad capacity to next power of two
        let required_capacity = quad_count.next_power_of_two().max(128);
        if required_capacity > state.circle_capacity {
            state.circle_capacity = required_capacity;
        }
        
        let capacity = state.circle_capacity;
        // Fill remaining capacity with degenerate triangles at origin
        let dummy_col = [0.0; 4];
        let dummy_params = [0.0; 4];

        for _ in quad_count..capacity {
            for _ in 0..4 {
                positions.push(Vec3::ZERO);
                uvs.push(Vec2::ZERO);
                body_colors.push(dummy_col);
                border_colors.push(dummy_col);
                approach_colors.push(dummy_col);
                params.push(dummy_params);
            }
            indices.extend_from_slice(&[0, 0, 0, 0, 0, 0]);
        }

        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
        mesh.insert_attribute(ATTRIBUTE_BODY_COLOR, body_colors);
        mesh.insert_attribute(ATTRIBUTE_BORDER_COLOR, border_colors);
        mesh.insert_attribute(ATTRIBUTE_APPROACH_COLOR, approach_colors);
        mesh.insert_attribute(ATTRIBUTE_SDF_PARAMS, params);
        mesh.insert_indices(Indices::U32(indices));
    }
}

/// Update the MSDF batch mesh from current entity data
fn update_msdf_batches(
    beatmap: Res<BeatmapView>,
    playback: Res<PlaybackStateRes>,
    transform: Res<PlayfieldTransform>,
    atlas: Res<MsdfAtlas>,
    mut state: ResMut<SdfRenderState>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    if transform.scale <= 0.0 {
        return;
    }

    if let Some(mesh) = meshes.get_mut(&state.msdf_batch_mesh) {
        let current_time = playback.current_time;
        let visible = beatmap.visible_objects(current_time);
        let radius = transform.scale_radius(beatmap.circle_radius);
        let digit_size = radius * 0.5;

        let mut quad_count = 0usize;
        let mut positions = Vec::new();
        let mut uvs = Vec::new();
        let mut indices = Vec::new();
        let mut colors = Vec::new();
        let mut uv_bounds = Vec::new();
        let mut params = Vec::new();

        for (index, obj, opacity) in visible.iter() {
            let opacity = *opacity;
            if opacity < 0.01 { continue; }
            
            // Only circles and sliders have combo numbers
            match &obj.kind {
                RenderObjectKind::Circle | RenderObjectKind::Slider { .. } => {
                    let pos = transform.osu_to_screen(obj.x, obj.y);
                    let combo_str = obj.combo_number.to_string();
                    
                    // Calculate total width
                    let mut total_width = 0.0;
                    for ch in combo_str.chars() {
                        let digit_value = ch.to_digit(10).unwrap_or(0) as usize;
                        let advance = atlas.digit_advances.get(digit_value).copied().unwrap_or(0.5);
                        total_width += advance * digit_size;
                    }

                    let mut current_x = pos.x - total_width * 0.5;
                    let z = -(*index as f32 * 0.001) + 0.0009;

                    for ch in combo_str.chars() {
                        let digit_value = ch.to_digit(10).unwrap_or(0) as usize;
                        let size_ratio = atlas.digit_sizes.get(digit_value).copied().unwrap_or(Vec2::ONE);
                        let glyph_width = digit_size * size_ratio.x;
                        let glyph_height = digit_size * 1.2;
                        
                        let digit_center_x = current_x + (atlas.digit_advances[digit_value] * digit_size) * 0.5;
                        
                        // Push Quad
                        let base_idx = (quad_count * 4) as u32;
                        let w2 = glyph_width * 0.5;
                        let h2 = glyph_height * 0.5;

                        positions.push(Vec3::new(digit_center_x - w2, pos.y - h2, z));
                        positions.push(Vec3::new(digit_center_x + w2, pos.y - h2, z));
                        positions.push(Vec3::new(digit_center_x + w2, pos.y + h2, z));
                        positions.push(Vec3::new(digit_center_x - w2, pos.y + h2, z));

                        uvs.push(Vec2::new(0.0, 1.0));
                        uvs.push(Vec2::new(1.0, 1.0));
                        uvs.push(Vec2::new(1.0, 0.0));
                        uvs.push(Vec2::new(0.0, 0.0));

                        indices.extend_from_slice(&[base_idx, base_idx + 1, base_idx + 2, base_idx, base_idx + 2, base_idx + 3]);

                        let col = [1.0, 1.0, 1.0, 1.0];
                        let b = atlas.digit_uvs[digit_value].to_array();
                        let p = [opacity, atlas.px_range];

                        for _ in 0..4 {
                            colors.push(col);
                            uv_bounds.push(b);
                            params.push(p);
                        }

                        current_x += atlas.digit_advances[digit_value] * digit_size;
                        quad_count += 1;
                    }
                }
                _ => {}
            }
        }

        // Buffer stabilization
        let required_capacity = quad_count.next_power_of_two().max(64);
        if required_capacity > state.msdf_capacity {
            state.msdf_capacity = required_capacity;
        }

        let capacity = state.msdf_capacity;
        let dummy_col = [0.0; 4];
        let dummy_uv_b = [0.0; 4];
        let dummy_p = [0.0; 2];

        for _ in quad_count..capacity {
            for _ in 0..4 {
                positions.push(Vec3::ZERO);
                uvs.push(Vec2::ZERO);
                colors.push(dummy_col);
                uv_bounds.push(dummy_uv_b);
                params.push(dummy_p);
            }
            indices.extend_from_slice(&[0, 0, 0, 0, 0, 0]);
        }

        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
        mesh.insert_attribute(ATTRIBUTE_MSDF_UV_BOUNDS, uv_bounds);
        mesh.insert_attribute(ATTRIBUTE_MSDF_PARAMS, params);
        mesh.insert_indices(Indices::U32(indices));
    }
}
