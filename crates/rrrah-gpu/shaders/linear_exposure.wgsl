struct Parameters { gain: f32, count: u32, padding: vec2<u32> }
@group(0) @binding(0) var<storage, read> source: array<vec4<f32>>;
@group(0) @binding(1) var<storage, read_write> output: array<vec4<f32>>;
@group(0) @binding(2) var<uniform> parameters: Parameters;
@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    if id.x >= parameters.count { return; }
    let pixel = source[id.x];
    output[id.x] = vec4<f32>(pixel.rgb * parameters.gain, pixel.a);
}
