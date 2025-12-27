// Reverse arrow SDF shader for osu-player
// Renders a chevron/arrow shape pointing in configurable direction

#import bevy_sprite::mesh2d_vertex_output::VertexOutput

struct ArrowUniforms {
    color: vec4<f32>,
    center: vec2<f32>,
    size: f32,
    direction: vec2<f32>,  // Normalized direction vector (arrow points this way)
    thickness: f32,
    opacity: f32,
    _padding: vec2<f32>,
}

@group(2) @binding(0) var<uniform> uniforms: ArrowUniforms;

// SDF for a chevron/arrow shape pointing right (>)
// direction rotates this shape
fn sdf_chevron(p: vec2<f32>, size: f32, thickness: f32) -> f32 {
    // Chevron is made of two angled lines meeting at a point
    // For a ">" shape pointing right:
    // Upper arm: from (-size, size*0.7) to (0, 0)
    // Lower arm: from (-size, -size*0.7) to (0, 0)
    
    let arm_length = size;
    let arm_height = size * 0.6;
    
    // Distance to upper arm line segment
    let p1 = vec2<f32>(-arm_length, arm_height);
    let p2 = vec2<f32>(0.0, 0.0);
    let d_upper = sdf_line_segment(p, p1, p2) - thickness;
    
    // Distance to lower arm line segment
    let p3 = vec2<f32>(-arm_length, -arm_height);
    let d_lower = sdf_line_segment(p, p3, p2) - thickness;
    
    // Union of both arms
    return min(d_upper, d_lower);
}

// SDF for a line segment from a to b
fn sdf_line_segment(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let h = clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0);
    return length(pa - ba * h);
}

// Rotate a 2D point by angle defined by direction vector
fn rotate_by_dir(p: vec2<f32>, dir: vec2<f32>) -> vec2<f32> {
    // dir is normalized (cos(angle), sin(angle))
    // Rotate p by -angle to align with standard chevron
    return vec2<f32>(
        p.x * dir.x + p.y * dir.y,
        -p.x * dir.y + p.y * dir.x
    );
}

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    let world_pos = mesh.world_position.xy;
    let rel_pos = world_pos - uniforms.center;
    
    // Rotate position to align with arrow direction
    let rotated_pos = rotate_by_dir(rel_pos, uniforms.direction);
    
    let aa_width = 1.5;
    
    let dist = sdf_chevron(rotated_pos, uniforms.size, uniforms.thickness);
    
    // Anti-aliased edge
    let alpha = 1.0 - smoothstep(-aa_width, aa_width, dist);
    
    if alpha < 0.01 {
        discard;
    }
    
    let final_alpha = alpha * uniforms.opacity * uniforms.color.a;
    return vec4<f32>(uniforms.color.rgb, final_alpha);
}
