// Vertex shader for tile rendering

struct CameraUniform {
    view_proj: mat4x4<f32>,
}

@group(0) @binding(0)
var<uniform> camera: CameraUniform;

@group(0) @binding(1)
var t_diffuse: texture_2d<f32>;

@group(0) @binding(2)
var s_diffuse: sampler;

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) tex_coords: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
}

@vertex
fn vs_main(model: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    
    // Transform position to clip space
    out.clip_position = camera.view_proj * vec4<f32>(model.position, 0.0, 1.0);
    out.tex_coords = model.tex_coords;
    
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Sample the tile texture
    let color = textureSample(t_diffuse, s_diffuse, in.tex_coords);
    
    // Apply some basic tone mapping and gamma correction for better visuals
    let tone_mapped = color.rgb / (color.rgb + vec3<f32>(1.0));
    let gamma_corrected = pow(tone_mapped, vec3<f32>(1.0 / 2.2));
    
    return vec4<f32>(gamma_corrected, color.a);
}
