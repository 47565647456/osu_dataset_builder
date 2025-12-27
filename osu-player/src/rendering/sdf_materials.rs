//! Custom Material2d implementations for SDF-based hit object rendering

use bevy::{
    prelude::*,
    render::render_resource::{AsBindGroup, ShaderType},
    shader::ShaderRef,
    sprite_render::{Material2d, Material2dPlugin},
};


/// Maximum number of path points supported in slider shader
pub const MAX_SLIDER_PATH_POINTS: usize = 128;

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

impl SliderMaterial {
    /// Create a new slider material from path points
    pub fn from_path(
        path_points: &[(f32, f32)],
        radius: f32,
        body_color: Color,
        border_color: Color,
    ) -> Self {
        let mut path_data = SliderPathData::default();
        let count = path_points.len().min(MAX_SLIDER_PATH_POINTS);

        // Pack points into vec4 array
        for i in 0..count {
            let vec_idx = i / 2;
            let (x, y) = path_points[i];
            if i % 2 == 0 {
                path_data.points[vec_idx].x = x;
                path_data.points[vec_idx].y = y;
            } else {
                path_data.points[vec_idx].z = x;
                path_data.points[vec_idx].w = y;
            }
        }

        // Calculate bounding box
        let (mut min_x, mut min_y) = (f32::MAX, f32::MAX);
        let (mut max_x, mut max_y) = (f32::MIN, f32::MIN);
        for &(x, y) in path_points.iter().take(count) {
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
        }

        // Expand bbox by radius
        let bbox_min = Vec2::new(min_x - radius, min_y - radius);
        let bbox_max = Vec2::new(max_x + radius, max_y + radius);
        let bbox_size = bbox_max - bbox_min;

        Self {
            uniforms: SliderUniforms {
                body_color: body_color.into(),
                border_color: border_color.into(),
                radius,
                border_width: radius * 0.15, // 15% of radius as border
                opacity: 1.0,
                point_count: count as u32,
                bbox_min,
                bbox_size,
            },
            path_data,
        }
    }

    /// Update opacity for fade effects
    pub fn with_opacity(mut self, opacity: f32) -> Self {
        self.uniforms.opacity = opacity;
        self
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


impl CircleMaterial {
    /// Create a new circle material
    pub fn new(radius: f32, center: Vec2, body_color: Color, border_color: Color) -> Self {
        Self {
            uniforms: CircleUniforms {
                body_color: body_color.into(),
                border_color: border_color.into(),
                approach_color: LinearRgba::WHITE,
                radius,
                border_width: radius * 0.15,
                approach_scale: 1.0, // No approach circle by default
                approach_width: 3.0,
                opacity: 1.0,
                center,
            },
        }
    }


    /// Set approach circle scale (1.0 = same size as circle, >1.0 = larger)
    pub fn with_approach(mut self, scale: f32) -> Self {
        self.uniforms.approach_scale = scale;
        self
    }

    /// Update opacity for fade effects
    pub fn with_opacity(mut self, opacity: f32) -> Self {
        self.uniforms.opacity = opacity;
        self
    }
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
