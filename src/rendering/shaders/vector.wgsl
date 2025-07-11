// Vertex shader for vector rendering (lines, polygons, etc.)

struct CameraUniform {
    view_proj: mat4x4<f32>,
}

@group(0) @binding(0)
var<uniform> camera: CameraUniform;

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) color: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
}

@vertex
fn vs_main(model: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    
    // Transform position to clip space
    out.clip_position = camera.view_proj * vec4<f32>(model.position, 0.0, 1.0);
    out.color = model.color;
    
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Simple pass-through of vertex color with some smoothing
    var color = in.color;
    
    // Add some anti-aliasing for edges
    let alpha = smoothstep(0.0, 1.0, color.a);
    
    return vec4<f32>(color.rgb, alpha);
}
