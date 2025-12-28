#import bevy_sprite::mesh2d_view_bindings::view

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) uv_bounds: vec4<f32>,
    @location(4) params: vec2<f32>, // opacity, px_range
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) uv_bounds: vec4<f32>,
    @location(3) params: vec2<f32>,
};

@vertex
fn vertex(input: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = view.clip_from_world * vec4<f32>(input.position, 1.0);
    out.uv = input.uv;
    out.color = input.color;
    out.uv_bounds = input.uv_bounds;
    out.params = input.params;
    return out;
}

@group(2) @binding(0) var<uniform> dummy: vec4<f32>;
@group(2) @binding(1) var msdf_texture: texture_2d<f32>;
@group(2) @binding(2) var msdf_sampler: sampler;

fn median(r: f32, g: f32, b: f32) -> f32 {
    return max(min(r, g), min(max(r, g), b));
}

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    let uv_bounds = mesh.uv_bounds;
    let opacity = mesh.params.x;
    let px_range = mesh.params.y;
    
    // Map mesh UV (0-1) to atlas
    let uv = mesh.uv;
    let atlas_uv = vec2<f32>(
        mix(uv_bounds.x, uv_bounds.z, uv.x),
        mix(uv_bounds.y, uv_bounds.w, uv.y)
    );
    
    let msd = textureSample(msdf_texture, msdf_sampler, atlas_uv).rgb;
    let sd = median(msd.r, msd.g, msd.b);
    
    let uv_dx = dpdx(atlas_uv);
    let uv_dy = dpdy(atlas_uv);
    let texel_size = max(max(length(uv_dx), length(uv_dy)), 0.0001);
    
    let atlas_size = vec2<f32>(textureDimensions(msdf_texture));
    let screen_px_range = px_range / (texel_size * max(atlas_size.x, atlas_size.y));
    let clamped_px_range = clamp(screen_px_range, 0.1, 100.0);
    
    let screen_px_distance = clamped_px_range * (sd - 0.5);
    let alpha = smoothstep(-0.5, 0.5, screen_px_distance);
    
    if alpha < 0.01 {
        discard;
    }
    
    return vec4<f32>(mesh.color.rgb, mesh.color.a * opacity * alpha);
}
