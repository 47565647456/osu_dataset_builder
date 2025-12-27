// Combo number SDF shader for osu-player
// Renders a single digit (0-9) using procedural SDF shapes
// Based on 7-segment style but with rounded/smooth shapes

#import bevy_sprite::mesh2d_vertex_output::VertexOutput

struct DigitUniforms {
    color: vec4<f32>,
    center: vec2<f32>,
    size: f32,
    digit: u32,         // 0-9
    opacity: f32,
    _padding: vec3<f32>,
}

@group(2) @binding(0) var<uniform> uniforms: DigitUniforms;

// SDF for a rounded rectangle (capsule shape when one dimension is small)
fn sdf_rounded_rect(p: vec2<f32>, half_size: vec2<f32>, radius: f32) -> f32 {
    let q = abs(p) - half_size + radius;
    return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - radius;
}

// SDF for a horizontal line segment (capsule)
fn sdf_h_segment(p: vec2<f32>, center_y: f32, half_width: f32, thickness: f32) -> f32 {
    let local_p = vec2<f32>(p.x, p.y - center_y);
    return sdf_rounded_rect(local_p, vec2<f32>(half_width, thickness), thickness);
}

// SDF for a vertical line segment (capsule)
fn sdf_v_segment(p: vec2<f32>, center_x: f32, y_min: f32, y_max: f32, thickness: f32) -> f32 {
    let center_y = (y_min + y_max) * 0.5;
    let half_height = (y_max - y_min) * 0.5;
    let local_p = vec2<f32>(p.x - center_x, p.y - center_y);
    return sdf_rounded_rect(local_p, vec2<f32>(thickness, half_height), thickness);
}

// Smooth minimum for union with rounded corners
fn smin(a: f32, b: f32, k: f32) -> f32 {
    let h = max(k - abs(a - b), 0.0) / k;
    return min(a, b) - h * h * k * 0.25;
}

// Generate SDF for digit 0-9 using segment-based approach
// Segments layout:
//    0
//   ---
// 1|   |2
//   -3-
// 4|   |5
//   ---
//    6
fn sdf_digit(p: vec2<f32>, digit: u32, size: f32) -> f32 {
    let w = size * 0.3;   // half-width of digit
    let h = size * 0.5;   // half-height of digit
    let t = size * 0.08;  // segment thickness
    let gap = t * 0.3;    // small gap between segments
    
    // Segment positions
    let top_y = h;
    let mid_y = 0.0;
    let bot_y = -h;
    let left_x = -w;
    let right_x = w;
    
    var dist: f32 = 1000.0;  // Start with large distance
    
    // Segment flags for each digit (7-segment encoding)
    // Bit 0: top, 1: top-left, 2: top-right, 3: middle, 4: bot-left, 5: bot-right, 6: bottom
    let segments = array<u32, 10>(
        0x77u,  // 0: 0110111
        0x24u,  // 1: 0100100
        0x5Du,  // 2: 1011101
        0x6Du,  // 3: 1101101
        0x2Eu,  // 4: 0101110
        0x6Bu,  // 5: 1101011
        0x7Bu,  // 6: 1111011
        0x25u,  // 7: 0100101
        0x7Fu,  // 8: 1111111
        0x6Fu,  // 9: 1101111
    );
    
    let seg = segments[digit];
    
    // Top horizontal (segment 0)
    if (seg & 0x01u) != 0u {
        let d = sdf_h_segment(p, top_y - t, w - t - gap, t);
        dist = min(dist, d);
    }
    
    // Top-left vertical (segment 1)
    if (seg & 0x02u) != 0u {
        let d = sdf_v_segment(p, left_x + t, mid_y + gap, top_y - t - gap, t);
        dist = min(dist, d);
    }
    
    // Top-right vertical (segment 2)
    if (seg & 0x04u) != 0u {
        let d = sdf_v_segment(p, right_x - t, mid_y + gap, top_y - t - gap, t);
        dist = min(dist, d);
    }
    
    // Middle horizontal (segment 3)
    if (seg & 0x08u) != 0u {
        let d = sdf_h_segment(p, mid_y, w - t - gap, t);
        dist = min(dist, d);
    }
    
    // Bottom-left vertical (segment 4)
    if (seg & 0x10u) != 0u {
        let d = sdf_v_segment(p, left_x + t, bot_y + t + gap, mid_y - gap, t);
        dist = min(dist, d);
    }
    
    // Bottom-right vertical (segment 5)
    if (seg & 0x20u) != 0u {
        let d = sdf_v_segment(p, right_x - t, bot_y + t + gap, mid_y - gap, t);
        dist = min(dist, d);
    }
    
    // Bottom horizontal (segment 6)
    if (seg & 0x40u) != 0u {
        let d = sdf_h_segment(p, bot_y + t, w - t - gap, t);
        dist = min(dist, d);
    }
    
    return dist;
}

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    let world_pos = mesh.world_position.xy;
    let rel_pos = world_pos - uniforms.center;
    
    let aa_width = 1.5;
    
    let dist = sdf_digit(rel_pos, uniforms.digit, uniforms.size);
    
    // Anti-aliased edge
    let alpha = 1.0 - smoothstep(-aa_width, aa_width, dist);
    
    if alpha < 0.01 {
        discard;
    }
    
    return vec4<f32>(uniforms.color.rgb, alpha * uniforms.opacity * uniforms.color.a);
}
