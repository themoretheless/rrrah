// Four independent normalized R/G/B/E lattices. No tone mapping or clipping.
// Host contract: dimensions >= 2, quad is a permutation of 0..3, finite samples,
// row_range is [first source row, output row count]. Input binding includes the interpolation halo;
// Output binding may include a prefix before the first writable row.
// output_window.x/y are global output-pixel/input-sample indices at binding starts.
struct Parameters {
    dimensions: vec2<u32>,
    row_range: vec2<u32>,
    quad: vec4<u32>,
    output_window: vec4<u32>,
};
@group(0) @binding(0) var<storage,read> source: array<f32>;
@group(0) @binding(1) var<storage,read_write> output: array<vec4<f32>>;
@group(0) @binding(2) var<uniform> parameters: Parameters;
struct Axis { lower:u32, upper:u32, weight:f32 };
fn lattice_axis(coordinate:u32,phase:u32,size:u32)->Axis {
    let last=phase+((size-1u-phase)/2u)*2u;
    let distance=select(0u,coordinate-phase,coordinate>=phase);
    let lower=min(phase+(distance/2u)*2u,last);
    let upper=min(lower+2u,last);
    var weight=0.0;
    if upper>lower && coordinate>=lower {
        weight=f32(min(coordinate-lower,upper-lower))/f32(upper-lower);
    }
    return Axis(lower,upper,weight);
}
fn sample_at(x:u32,y:u32)->f32 { return source[y*parameters.dimensions.x+x-parameters.output_window.y]; }
// Lattice weights are exactly 0, 1/2 or 1. Preserve endpoints and avoid
// compiler reassociation into an overflowing (b-a) for opposite extremes.
fn half_extreme(value:f32)->f32 {
    let bits=bitcast<u32>(value);
    if (bits & 0x7f800000u)>0x00800000u {
        return bitcast<f32>(bits-0x00800000u);
    }
    return value*0.5;
}
fn blend(a:f32,b:f32,t:f32)->f32 {
    if t==0.0 { return a; }
    if t==1.0 { return b; }
    if abs(a)>8.0e37 || abs(b)>8.0e37 {
        return half_extreme(a)+half_extreme(b);
    }
    return a*(1.0-t)+b*t;
}
@compute @workgroup_size(8,8)
fn main(@builtin(global_invocation_id) id:vec3<u32>) {
    if id.x>=parameters.dimensions.x || id.y>=parameters.row_range.y { return; }
    let sensor_y=id.y+parameters.row_range.x;
    var planes=vec4<f32>(0.0);
    for(var phase=0u;phase<4u;phase+=1u) {
        let x=lattice_axis(id.x,phase%2u,parameters.dimensions.x);
        let y=lattice_axis(sensor_y,phase/2u,parameters.dimensions.y);
        let first=blend(sample_at(x.lower,y.lower),sample_at(x.upper,y.lower),x.weight);
        let second=blend(sample_at(x.lower,y.upper),sample_at(x.upper,y.upper),x.weight);
        planes[parameters.quad[phase]]=blend(first,second,y.weight);
    }
    output[sensor_y*parameters.dimensions.x+id.x-parameters.output_window.x]=planes;
}
