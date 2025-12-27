// MSDF text shader for osu-player
// Renders text using Multi-channel Signed Distance Field texture
// Based on Chlumsky's msdf-atlas-gen output format

#import bevy_sprite::mesh2d_vertex_output::VertexOutput

struct MsdfUniforms {
    color: vec4<f32>,
    // Atlas UV bounds for this glyph (left, bottom, right, top) normalized 0-1
    uv_bounds: vec4<f32>,
    opacity: f32,
    // Distance range from atlas (typically 2.0 for msdf-atlas-gen default)
    pxRange: f32,
    _padding: vec2<f32>,
}

@group(2) @binding(0) var<uniform> uniforms: MsdfUniforms;
@group(2) @binding(1) var msdf_texture: texture_2d<f32>;
@group(2) @binding(2) var msdf_sampler: sampler;

// Median of three values - key to MSDF decoding
fn median(r: f32, g: f32, b: f32) -> f32 {
    return max(min(r, g), min(max(r, g), b));
}

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    // Map mesh UV (0-1) to the glyph's region in the atlas
    let uv = mesh.uv;
    let atlas_uv = vec2<f32>(
        mix(uniforms.uv_bounds.x, uniforms.uv_bounds.z, uv.x),
        mix(uniforms.uv_bounds.y, uniforms.uv_bounds.w, uv.y)
    );
    
    // Sample MSDF texture
    let msdf_sample = textureSample(msdf_texture, msdf_sampler, atlas_uv);
    
    // Get signed distance from median of RGB channels
    let sd = median(msdf_sample.r, msdf_sample.g, msdf_sample.b);
    
    // Calculate screen-space distance for proper anti-aliasing at any scale
    // This measures how much the UV changes per screen pixel
    let uv_dx = dpdx(atlas_uv);
    let uv_dy = dpdy(atlas_uv);
    let texel_size = max(length(uv_dx), length(uv_dy));
    
    // Convert to screen pixels: how many atlas pixels per screen pixel
    let atlas_size = vec2<f32>(textureDimensions(msdf_texture));
    let screen_px_range = uniforms.pxRange / (texel_size * max(atlas_size.x, atlas_size.y));
    
    // Convert signed distance to screen-space pixels
    let screen_px_distance = screen_px_range * (sd - 0.5);
    
    // Anti-aliased alpha using smoothstep for smooth edges
    let alpha = smoothstep(-0.5, 0.5, screen_px_distance);
    
    if alpha < 0.01 {
        discard;
    }
    
    return vec4<f32>(uniforms.color.rgb, alpha * uniforms.opacity * uniforms.color.a);
}