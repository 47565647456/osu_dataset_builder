// Hit circle SDF shader for osu-player
// Renders a circle with border, inner fill, and optional approach circle

#import bevy_sprite::mesh2d_vertex_output::VertexOutput

struct CircleUniforms {
    body_color: vec4<f32>,
    border_color: vec4<f32>,
    approach_color: vec4<f32>,
    radius: f32,
    border_width: f32,
    approach_scale: f32,
    approach_width: f32,
    opacity: f32,
    center: vec2<f32>,
}

@group(2) @binding(0) var<uniform> uniforms: CircleUniforms;

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    let world_pos = mesh.world_position.xy;
    let rel_pos = world_pos - uniforms.center;
    let dist_from_center = length(rel_pos);
    
    let aa_width = 1.0;
    
    var final_color = vec3<f32>(0.0);
    var final_alpha = 0.0;
    
    // Approach circle (ring behind main circle)
    if uniforms.approach_scale > 1.01 {
        let approach_radius = uniforms.radius * uniforms.approach_scale;
        let approach_dist = abs(dist_from_center - approach_radius);
        let ring_alpha = 1.0 - smoothstep(0.0, uniforms.approach_width + aa_width, approach_dist);
        
        if ring_alpha > 0.001 {
            final_color = uniforms.approach_color.rgb;
            final_alpha = ring_alpha * uniforms.opacity;
        }
    }
    
    // Main circle
    let main_sd = dist_from_center - uniforms.radius;
    
    if main_sd > aa_width {
        // Outside main circle - just return approach if any
        if final_alpha < 0.01 {
            discard;
        }
        return vec4<f32>(final_color, final_alpha);
    }
    
    let main_edge_alpha = 1.0 - smoothstep(-aa_width, aa_width, main_sd);
    
    if main_edge_alpha > 0.01 {
        // Border factor
        let border_factor = smoothstep(-uniforms.border_width, 0.0, main_sd);
        
        // Get colors with alphas
        let body_rgb = uniforms.body_color.rgb;
        let body_a = uniforms.body_color.a;
        let border_rgb = uniforms.border_color.rgb;
        let border_a = uniforms.border_color.a;
        
        // Blend body -> border
        let fill_rgb = mix(body_rgb, border_rgb, border_factor);
        let fill_a = mix(body_a, border_a, border_factor);
        
        let circle_alpha = main_edge_alpha * fill_a * uniforms.opacity;
        
        // Blend over approach
        final_color = fill_rgb * circle_alpha + final_color * (1.0 - circle_alpha);
        final_alpha = circle_alpha + final_alpha * (1.0 - circle_alpha);
    }
    
    if final_alpha < 0.01 {
        discard;
    }
    
    return vec4<f32>(final_color, final_alpha);
}
