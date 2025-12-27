// Grid background shader for osu-player playfield
// Renders a thin grey grid pattern

#import bevy_sprite::mesh2d_vertex_output::VertexOutput

struct GridUniforms {
    background_color: vec4<f32>,
    line_color: vec4<f32>,
    cell_size: f32,         // Size of each grid cell in pixels
    line_thickness: f32,    // Thickness of grid lines
    _padding: vec2<f32>,
}

@group(2) @binding(0) var<uniform> uniforms: GridUniforms;

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    let world_pos = mesh.world_position.xy;
    
    // Calculate distance to nearest grid line
    let cell = uniforms.cell_size;
    let half_thickness = uniforms.line_thickness * 0.5;
    
    // Distance to nearest horizontal and vertical lines
    let mod_x = abs(world_pos.x % cell);
    let mod_y = abs(world_pos.y % cell);
    
    // Handle negative modulo correctly
    let dist_x = min(mod_x, cell - mod_x);
    let dist_y = min(mod_y, cell - mod_y);
    
    // Check if we're on a grid line
    let on_line = dist_x < half_thickness || dist_y < half_thickness;
    
    if on_line {
        return uniforms.line_color;
    } else {
        return uniforms.background_color;
    }
}
