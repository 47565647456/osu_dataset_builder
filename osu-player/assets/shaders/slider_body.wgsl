// Slider body SDF shader for osu-player
// Renders a thick polyline path with rounded caps and anti-aliased edges

#import bevy_sprite::mesh2d_vertex_output::VertexOutput

struct SliderUniforms {
    // Slider body colors
    body_color: vec4<f32>,
    border_color: vec4<f32>,
    
    // Sizing
    radius: f32,
    border_width: f32,
    
    // Opacity for fade in/out
    opacity: f32,
    
    // Number of valid points in path_points array
    point_count: u32,
    
    // Bounding box offset
    bbox_min: vec2<f32>,
    bbox_size: vec2<f32>,
}

struct PathPoints {
    points: array<vec4<f32>, 64>,
}

@group(2) @binding(0) var<uniform> uniforms: SliderUniforms;
@group(2) @binding(1) var<uniform> path_data: PathPoints;

fn sd_segment(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let len_sq = dot(ba, ba);
    if len_sq < 0.0001 {
        return length(pa);
    }
    let h = clamp(dot(pa, ba) / len_sq, 0.0, 1.0);
    return length(pa - ba * h);
}

fn sd_polyline(p: vec2<f32>) -> f32 {
    var min_dist = 1000000.0;
    let count = uniforms.point_count;
    
    if count < 2u {
        return min_dist;
    }
    
    var i: u32 = 0u;
    loop {
        if i >= count - 1u {
            break;
        }
        
        let vec_idx = i / 2u;
        let is_second = (i % 2u) == 1u;
        
        var pt_a: vec2<f32>;
        var pt_b: vec2<f32>;
        
        if is_second {
            pt_a = path_data.points[vec_idx].zw;
            pt_b = path_data.points[vec_idx + 1u].xy;
        } else {
            pt_a = path_data.points[vec_idx].xy;
            pt_b = path_data.points[vec_idx].zw;
        }
        
        let d = sd_segment(p, pt_a, pt_b);
        min_dist = min(min_dist, d);
        
        i = i + 1u;
    }
    
    return min_dist;
}

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    let world_pos = mesh.world_position.xy;
    let dist = sd_polyline(world_pos);
    let sd = dist - uniforms.radius;
    
    let aa_width = 1.0;
    
    // Outside the shape entirely
    if sd > aa_width {
        discard;
    }
    
    // Edge AA factor (1 inside, 0 outside)
    let edge_alpha = 1.0 - smoothstep(-aa_width, aa_width, sd);
    
    // Border is from edge inward (sd near 0 = border, sd << 0 = body)
    let border_factor = smoothstep(-uniforms.border_width, 0.0, sd);
    
    // Get colors with their alphas
    let body_rgb = uniforms.body_color.rgb;
    let body_a = uniforms.body_color.a;
    let border_rgb = uniforms.border_color.rgb;
    let border_a = uniforms.border_color.a;
    
    // Blend body and border based on position
    let fill_rgb = mix(body_rgb, border_rgb, border_factor);
    let fill_a = mix(body_a, border_a, border_factor);
    
    // Apply edge AA and overall opacity
    let final_alpha = edge_alpha * fill_a * uniforms.opacity;
    
    if final_alpha < 0.01 {
        discard;
    }
    
    return vec4<f32>(fill_rgb, final_alpha);
}
