// Orbit tracks and other line work.
//
// Same camera-relative render space as body.wgsl; positions arrive already
// translated and scaled on the CPU, so there is no model matrix here.

struct Globals {
    view_projection: mat4x4<f32>,
    light: vec4<f32>,
};

@group(0) @binding(0) var<uniform> globals: Globals;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(vertex: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = globals.view_projection * vec4<f32>(vertex.position, 1.0);
    out.color = vertex.color;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}
