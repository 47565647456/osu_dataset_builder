use bevy::prelude::*;
use bevy::mesh::{MeshVertexAttribute, MeshVertexBufferLayoutRef};
use bevy::render::render_resource::{AsBindGroup, RenderPipelineDescriptor, ShaderType, SpecializedMeshPipelineError, VertexFormat};
use bevy::shader::ShaderRef;
use bevy::sprite_render::{AlphaMode2d, Material2d, Material2dKey, Material2dPlugin};

/// Custom vertex attributes for SDF batch rendering
pub const ATTRIBUTE_BODY_COLOR: MeshVertexAttribute =
    MeshVertexAttribute::new("BodyColor", 20001, VertexFormat::Float32x4);
pub const ATTRIBUTE_BORDER_COLOR: MeshVertexAttribute =
    MeshVertexAttribute::new("BorderColor", 20002, VertexFormat::Float32x4);
pub const ATTRIBUTE_APPROACH_COLOR: MeshVertexAttribute =
    MeshVertexAttribute::new("ApproachColor", 20003, VertexFormat::Float32x4);
/// Packed params: (border_width_rel, approach_width_rel, approach_scale, opacity)
pub const ATTRIBUTE_SDF_PARAMS: MeshVertexAttribute =
    MeshVertexAttribute::new("SdfParams", 20004, VertexFormat::Float32x4);

pub const ATTRIBUTE_MSDF_UV_BOUNDS: MeshVertexAttribute =
    MeshVertexAttribute::new("MsdfUvBounds", 20005, VertexFormat::Float32x4);
pub const ATTRIBUTE_MSDF_PARAMS: MeshVertexAttribute =
    MeshVertexAttribute::new("MsdfParams", 20006, VertexFormat::Float32x2);



/// Dummy uniforms for batch materials to satisfy shader bindings
#[derive(Clone, Copy, Debug, Default, ShaderType)]
pub struct BatchUniforms {
    pub _dummy: Vec4,
}

/// Material for rendering batched hit circles with SDF
#[derive(Asset, TypePath, AsBindGroup, Clone, Debug, Default)]
pub struct CircleBatchMaterial {
    #[uniform(0)]
    pub uniforms: BatchUniforms,
}

impl Material2d for CircleBatchMaterial {
    fn vertex_shader() -> ShaderRef {
        "shaders/circle_batch.wgsl".into()
    }
    fn fragment_shader() -> ShaderRef {
        "shaders/circle_batch.wgsl".into()
    }
    fn alpha_mode(&self) -> AlphaMode2d {
        AlphaMode2d::Blend
    }
    fn specialize(
        descriptor: &mut RenderPipelineDescriptor,
        layout: &MeshVertexBufferLayoutRef,
        _key: Material2dKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        let vertex_attributes = vec![
            Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
            Mesh::ATTRIBUTE_UV_0.at_shader_location(1),
            ATTRIBUTE_BODY_COLOR.at_shader_location(2),
            ATTRIBUTE_BORDER_COLOR.at_shader_location(3),
            ATTRIBUTE_APPROACH_COLOR.at_shader_location(4),
            ATTRIBUTE_SDF_PARAMS.at_shader_location(5),
        ];
        descriptor.vertex.buffers[0] = layout.0.get_layout(&vertex_attributes)?;
        Ok(())
    }
}

/// Material for rendering batched MSDF digits
#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
pub struct MsdfBatchMaterial {
    #[uniform(0)]
    pub uniforms: BatchUniforms,
    #[texture(1)]
    #[sampler(2)]
    pub texture: Handle<Image>,
}

impl Material2d for MsdfBatchMaterial {
    fn vertex_shader() -> ShaderRef {
        "shaders/msdf_batch.wgsl".into()
    }
    fn fragment_shader() -> ShaderRef {
        "shaders/msdf_batch.wgsl".into()
    }
    fn alpha_mode(&self) -> AlphaMode2d {
        AlphaMode2d::Blend
    }
    fn specialize(
        descriptor: &mut RenderPipelineDescriptor,
        layout: &MeshVertexBufferLayoutRef,
        _key: Material2dKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        let vertex_attributes = vec![
            Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
            Mesh::ATTRIBUTE_UV_0.at_shader_location(1),
            Mesh::ATTRIBUTE_COLOR.at_shader_location(2),
            ATTRIBUTE_MSDF_UV_BOUNDS.at_shader_location(3),
            ATTRIBUTE_MSDF_PARAMS.at_shader_location(4),
        ];
        descriptor.vertex.buffers[0] = layout.0.get_layout(&vertex_attributes)?;
        Ok(())
    }
}

/// Material for rendering slider bodies with SDF
#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
pub struct SliderMaterial {
    #[uniform(0)]
    pub uniforms: SliderUniforms,
    #[uniform(1)]
    pub path_data: SliderPathData,
}

/// Uniform data for slider rendering
#[derive(Clone, Copy, Debug, Default, ShaderType)]
pub struct SliderUniforms {
    pub body_color: LinearRgba,
    pub border_color: LinearRgba,
    pub radius: f32,
    pub border_width: f32,
    pub opacity: f32,
    pub point_count: u32,
    pub bbox_min: Vec2,
    pub bbox_size: Vec2,
}

/// Path point data packed as vec4s (xy = point N, zw = point N+1)
#[derive(Clone, Copy, Debug, ShaderType)]
pub struct SliderPathData {
    pub points: [Vec4; 64], // 128 points packed as 64 vec4s
}

impl Default for SliderPathData {
    fn default() -> Self {
        Self {
            points: [Vec4::ZERO; 64],
        }
    }
}

impl Material2d for SliderMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/slider_body.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode2d {
        AlphaMode2d::Blend
    }
}

/// Material for rendering hit circles with SDF
#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
pub struct CircleMaterial {
    #[uniform(0)]
    pub uniforms: CircleUniforms,
}

/// Uniform data for circle rendering
#[derive(Clone, Copy, Debug, Default, ShaderType)]
pub struct CircleUniforms {
    pub body_color: LinearRgba,
    pub border_color: LinearRgba,
    pub approach_color: LinearRgba,
    pub border_width_rel: f32, // Width relative to radius 1.0
    pub approach_width_rel: f32,
    pub approach_scale: f32,
    pub opacity: f32,
    pub _padding: Vec2,
}

impl Material2d for CircleMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/circle.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode2d {
        AlphaMode2d::Blend
    }
}

/// Material for rendering reverse arrows with SDF
#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
pub struct ArrowMaterial {
    #[uniform(0)]
    pub uniforms: ArrowUniforms,
}

/// Uniform data for arrow rendering
#[derive(Clone, Copy, Debug, Default, ShaderType)]
pub struct ArrowUniforms {
    pub color: LinearRgba,
    pub thickness_rel: f32,
    pub opacity: f32,
    pub _padding: Vec2,  // For alignment
}

impl Material2d for ArrowMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/arrow.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode2d {
        AlphaMode2d::Blend
    }
}

/// Material for rendering spinners with SDF
#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
pub struct SpinnerMaterial {
    #[uniform(0)]
    pub uniforms: SpinnerUniforms,
}

/// Uniform data for spinner rendering
#[derive(Clone, Copy, Debug, Default, ShaderType)]
pub struct SpinnerUniforms {
    pub color: LinearRgba,
    pub progress: f32,    // 0.0 to 1.0
    pub rotation: f32,    // Rotation angle in radians
    pub opacity: f32,
    pub _padding: Vec2,   // For alignment
}

impl Material2d for SpinnerMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/spinner.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode2d {
        AlphaMode2d::Blend
    }
}

/// Material for rendering MSDF text (digits)
#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
pub struct MsdfMaterial {
    #[uniform(0)]
    pub uniforms: MsdfUniforms,
    #[texture(1)]
    #[sampler(2)]
    pub texture: Handle<Image>,
}

/// Uniform data for MSDF text rendering
#[derive(Clone, Copy, Debug, Default, ShaderType)]
pub struct MsdfUniforms {
    pub color: LinearRgba,
    /// UV bounds in atlas: (left, bottom, right, top) normalized 0-1
    pub uv_bounds: Vec4,
    pub opacity: f32,
    /// Distance range (typically 2.0 from msdf-atlas-gen)
    pub px_range: f32,
    pub _padding: Vec2,
}

impl Material2d for MsdfMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/msdf_text.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode2d {
        AlphaMode2d::Blend
    }
}

/// Material for rendering grid background
#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
pub struct GridMaterial {
    #[uniform(0)]
    pub uniforms: GridUniforms,
}

/// Uniform data for grid rendering
#[derive(Clone, Copy, Debug, Default, ShaderType)]
pub struct GridUniforms {
    pub background_color: LinearRgba,
    pub line_color: LinearRgba,
    pub cell_size: f32,         // Size of each grid cell in pixels
    pub line_thickness: f32,    // Thickness of grid lines
    pub _padding: Vec2,
}

impl Material2d for GridMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/grid.wgsl".into()
    }
}

/// Plugin to register SDF materials
pub struct SdfMaterialsPlugin;

impl Plugin for SdfMaterialsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(Material2dPlugin::<SliderMaterial>::default())
            .add_plugins(Material2dPlugin::<CircleMaterial>::default())
            .add_plugins(Material2dPlugin::<ArrowMaterial>::default())
            .add_plugins(Material2dPlugin::<SpinnerMaterial>::default())
            .add_plugins(Material2dPlugin::<MsdfMaterial>::default())
            .add_plugins(Material2dPlugin::<GridMaterial>::default())
            .add_plugins(Material2dPlugin::<CircleBatchMaterial>::default())
            .add_plugins(Material2dPlugin::<MsdfBatchMaterial>::default());
    }
}
