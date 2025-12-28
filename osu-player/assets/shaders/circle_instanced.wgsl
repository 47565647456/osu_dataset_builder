// Instanced circle SDF shader for osu-player
// Renders many circles with a single draw call using GPU instancing
// Per-instance data is stored in a storage buffer

#import bevy_sprite::mesh2d_vertex_output::VertexOutput

struct CircleInstanceData {
    body_color: vec4<f32>,
    border_color: vec4<f32>,
    approach_color: vec4<f32>,
    center: vec2<f32>,
    radius: f32,
    border_width: f32,
    approach_scale: f32,
    approach_width: f32,
    opacity: f32,
    z_index: f32,
}

@group(2) @binding(0) var<storage, read> instances: array<CircleInstanceData>;

// We use a uniform to pass the current instance index since Bevy's Material2d
// doesn't expose instance_index to the fragment shader directly
struct InstanceIndex {
    index: u32,
}

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    // For now, we'll use the mesh position to determine which instance this is
    // by checking which instance's center is closest to our world position
    // This is a workaround until we implement proper instance indexing
    
    let world_pos = mesh.world_position.xy;
    
    // Find the correct instance by checking center positions
    var best_idx: u32 = 0u;
    var best_dist: f32 = 1000000.0;
    let num_instances = arrayLength(&instances);
    
    for (var i: u32 = 0u; i < num_instances; i = i + 1u) {
        let inst = instances[i];
        let dist = length(world_pos - inst.center);
        // Only consider instances where we're within their bounding box
        let max_extent = inst.radius * inst.approach_scale + 20.0;
        if dist < max_extent && dist < best_dist {
            best_dist = dist;
            best_idx = i;
        }
    }
    
    let data = instances[best_idx];
    
    let rel_pos = world_pos - data.center;
    let dist_from_center = length(rel_pos);
    
    let aa_width = 1.0;
    
    var final_color = vec3<f32>(0.0);
    var final_alpha = 0.0;
    
    // Approach circle (ring behind main circle)
    if data.approach_scale > 1.01 {
        let approach_radius = data.radius * data.approach_scale;
        let approach_dist = abs(dist_from_center - approach_radius);
        let ring_alpha = 1.0 - smoothstep(0.0, data.approach_width + aa_width, approach_dist);
        
        if ring_alpha > 0.001 {
            final_color = data.approach_color.rgb;
            final_alpha = ring_alpha * data.opacity;
        }
    }
    
    // Main circle
    let main_sd = dist_from_center - data.radius;
    
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
        let border_factor = smoothstep(-data.border_width, 0.0, main_sd);
        
        // Get colors with alphas
        let body_rgb = data.body_color.rgb;
        let body_a = data.body_color.a;
        let border_rgb = data.border_color.rgb;
        let border_a = data.border_color.a;
        
        // Blend body -> border
        let fill_rgb = mix(body_rgb, border_rgb, border_factor);
        let fill_a = mix(body_a, border_a, border_factor);
        
        let circle_alpha = main_edge_alpha * fill_a * data.opacity;
        
        // Blend over approach
        final_color = fill_rgb * circle_alpha + final_color * (1.0 - circle_alpha);
        final_alpha = circle_alpha + final_alpha * (1.0 - circle_alpha);
    }
    
    if final_alpha < 0.01 {
        discard;
    }
    
    return vec4<f32>(final_color, final_alpha);
}
