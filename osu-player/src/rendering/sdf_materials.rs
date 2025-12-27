//! Custom Material2d implementations for SDF-based hit object rendering

use bevy::{
    prelude::*,
    render::render_resource::{AsBindGroup, ShaderType},
    shader::ShaderRef,
    sprite_render::{Material2d, Material2dPlugin},
};

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
    pub radius: f32,
    pub border_width: f32,
    pub approach_scale: f32,
    pub approach_width: f32,
    pub opacity: f32,
    pub center: Vec2,
}

impl Material2d for CircleMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/circle.wgsl".into()
    }
}

/// Plugin to register SDF materials
pub struct SdfMaterialsPlugin;

impl Plugin for SdfMaterialsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(Material2dPlugin::<SliderMaterial>::default())
            .add_plugins(Material2dPlugin::<CircleMaterial>::default());
    }
}
