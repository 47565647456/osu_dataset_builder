// Reverse arrow SDF shader for osu-player
// Renders a chevron/arrow shape in local UV space
// Optimized for batching: uses Transform for position, scale, and rotation

#import bevy_sprite::mesh2d_vertex_output::VertexOutput

struct ArrowUniforms {
    color: vec4<f32>,
    thickness_rel: f32, // Thickness relative to mesh size
    opacity: f32,
    _padding: vec2<f32>,
}

@group(2) @binding(0) var<uniform> uniforms: ArrowUniforms;

// SDF for a line segment from a to b
fn sdf_line_segment(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let h = clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0);
    return length(pa - ba * h);
}

// SDF for a chevron shape pointing "forward" in local Y or X
// Here we define it pointing right (positive X) in local space [-1, 1]
fn sdf_chevron(p: vec2<f32>, thickness: f32) -> f32 {
    let arm_length = 0.6;
    let arm_height = 0.4;
    
    // Tip is at (0.3, 0)
    // Arms go back to (-0.3, 0.4) and (-0.3, -0.4)
    let tip = vec2<f32>(0.3, 0.0);
    let upper_end = vec2<f32>(-0.3, arm_height);
    let lower_end = vec2<f32>(-0.3, -arm_height);
    
    let d_upper = sdf_line_segment(p, upper_end, tip) - thickness;
    let d_lower = sdf_line_segment(p, lower_end, tip) - thickness;
    
    return min(d_upper, d_lower);
}

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    // Local coordinates in range [-1, 1]
    let p = mesh.uv * 2.0 - 1.0;
    
    // We assume the quad is square and rotation is handled by Transform
    let aa_width = 0.05; 
    
    let dist = sdf_chevron(p, uniforms.thickness_rel);
    
    // Anti-aliased edge
    let alpha = 1.0 - smoothstep(-aa_width, aa_width, dist);
    
    if alpha < 0.01 {
        discard;
    }
    
    let final_alpha = alpha * uniforms.opacity * uniforms.color.a;
    return vec4<f32>(uniforms.color.rgb, final_alpha);
}
