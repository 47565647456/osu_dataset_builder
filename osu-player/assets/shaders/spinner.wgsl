// Spinner SDF shader for osu-player
// Renders concentric circles with rotating indicator and progress in local UV space
// Optimized for batching: uses Transform for position and scale

#import bevy_sprite::mesh2d_vertex_output::VertexOutput

struct SpinnerUniforms {
    color: vec4<f32>,
    progress: f32,  // 0.0 to 1.0
    rotation: f32,  // Rotation angle in radians
    opacity: f32,
    _padding: vec2<f32>,
}

@group(2) @binding(0) var<uniform> uniforms: SpinnerUniforms;

// SDF for a ring (circle outline)
fn sdf_ring(p: vec2<f32>, radius: f32, thickness: f32) -> f32 {
    return abs(length(p) - radius) - thickness;
}

// SDF for a line segment from a to b
fn sdf_line_segment(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>, thickness: f32) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let h = clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0);
    return length(pa - ba * h) - thickness;
}

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    // Local coordinates in range [-1, 1]
    let rel_pos = mesh.uv * 2.0 - 1.0;
    let dist_from_center = length(rel_pos);
    
    // In local space [-1, 1], max radius is 1.0
    let max_radius = 1.0;
    let aa_width = 0.02; // Small relative width for AA
    let ring_thickness = 0.01;
    
    var final_color = uniforms.color.rgb;
    var final_alpha = 0.0;
    
    // Draw three concentric rings
    let ring_radii = array<f32, 3>(
        max_radius * 0.3,
        max_radius * 0.6,
        max_radius * 0.9
    );
    
    for (var i = 0u; i < 3u; i = i + 1u) {
        let ring_dist = sdf_ring(rel_pos, ring_radii[i], ring_thickness);
        let ring_alpha = 1.0 - smoothstep(-aa_width, aa_width, ring_dist);
        final_alpha = max(final_alpha, ring_alpha);
    }
    
    // Progress circle (filled inner area that grows)
    let progress_radius = max_radius * 0.2 * uniforms.progress;
    if progress_radius > 0.0 {
        let progress_dist = dist_from_center - progress_radius;
        let progress_alpha = 1.0 - smoothstep(-aa_width, aa_width, progress_dist);
        final_alpha = max(final_alpha, progress_alpha * uniforms.progress);
    }
    
    // Rotating line indicator
    let line_dir = vec2<f32>(cos(uniforms.rotation), sin(uniforms.rotation));
    let line_end = line_dir * max_radius;
    let line_dist = sdf_line_segment(rel_pos, vec2<f32>(0.0), line_end, 0.01);
    let line_alpha = 1.0 - smoothstep(-aa_width, aa_width, line_dist);
    final_alpha = max(final_alpha, line_alpha);
    
    if final_alpha < 0.01 {
        discard;
    }
    
    return vec4<f32>(final_color, final_alpha * uniforms.opacity);
}
