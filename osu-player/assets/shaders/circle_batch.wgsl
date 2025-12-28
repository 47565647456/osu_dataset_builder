#import bevy_sprite::mesh2d_view_bindings::view

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) body_color: vec4<f32>,
    @location(3) border_color: vec4<f32>,
    @location(4) approach_color: vec4<f32>,
    /// border_width_rel, approach_width_rel, approach_scale, opacity
    @location(5) params: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) body_color: vec4<f32>,
    @location(2) border_color: vec4<f32>,
    @location(3) approach_color: vec4<f32>,
    @location(4) params: vec4<f32>,
};

@vertex
fn vertex(input: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = view.clip_from_world * vec4<f32>(input.position, 1.0);
    out.uv = input.uv;
    out.body_color = input.body_color;
    out.border_color = input.border_color;
    out.approach_color = input.approach_color;
    out.params = input.params;
    return out;
}

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    // Local UV centered at (0,0) from -1 to 1
    let local_pos = mesh.uv * 2.0 - 1.0;
    
    let border_width_rel = mesh.params.x;
    let approach_width_rel = mesh.params.y;
    let approach_scale = mesh.params.z;
    let opacity = mesh.params.w;

    let max_scale = max(1.0, approach_scale);
    let expansion = 1.4;
    
    // Normalized distance where 1.0 is the circle radius
    // Since quad half-extent is radius * expansion * max_scale:
    let dist = length(local_pos) * expansion * max_scale;
    
    let aa_width = 0.02; // Approximation
    
    var final_color = vec3<f32>(0.0);
    var final_alpha = 0.0;
    
    // Approach circle (lowest layer)
    if approach_width_rel > 1.0e-4 {
        let approach_dist = abs(dist - approach_scale);
        let ring_alpha = 1.0 - smoothstep(0.0, approach_width_rel + aa_width, approach_dist);
        
        if ring_alpha > 0.001 {
            final_color = mesh.approach_color.rgb;
            final_alpha = ring_alpha;
        }
    }
    
    // Circle body and border (top layer)
    let main_sd = dist - 1.0;
    let main_edge_alpha = 1.0 - smoothstep(-aa_width, aa_width, main_sd);
    
    if main_edge_alpha > 0.01 {
        // Border factor: 1.0 at outer edge, 0.0 at inner edge of border
        let border_factor = smoothstep(-border_width_rel, 0.0, main_sd);
        
        let fill_rgb = mix(mesh.body_color.rgb, mesh.border_color.rgb, border_factor);
        let fill_a = mix(mesh.body_color.a, mesh.border_color.a, border_factor);
        
        let circle_alpha = main_edge_alpha * fill_a;
        
        // Blend OVER approach
        final_color = mix(final_color, fill_rgb, circle_alpha);
        final_alpha = mix(final_alpha, 1.0, circle_alpha);
    }
    
    if final_alpha * opacity < 0.01 {
        discard;
    }
    
    return vec4<f32>(final_color, final_alpha * opacity);
}
