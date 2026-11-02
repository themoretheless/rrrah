struct Parameters { sensor_width:u32, x:u32, y:u32, width:u32, height:u32, stride:u32, samples:u32, padding:u32 };
@group(0) @binding(0) var<storage,read> source_pixels:array<vec4<f32>>;
@group(0) @binding(1) var<storage,read_write> counters:array<atomic<u32>>;
@group(0) @binding(2) var<uniform> p:Parameters;
@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id:vec3<u32>) {
    if id.x>=p.samples {return;}
    let local=id.x*p.stride;
    let pixel=source_pixels[(p.y+local/p.width)*p.sensor_width+p.x+local%p.width];
    atomicAdd(&counters[272],1u);
    if !all(abs(pixel)<=vec4<f32>(3.402823466e38)) || pixel.a<0.0 || pixel.a>1.0 {atomicAdd(&counters[273],1u);return;}
    if pixel.a==0.0 {atomicAdd(&counters[274],1u);return;}
    let values=vec4<f32>(pixel.rgb,dot(pixel.rgb,vec3<f32>(0.2126,0.7152,0.0722)));
    for(var c=0u;c<4u;c+=1u) {
        let value=values[c];
        if value<0.0 {atomicAdd(&counters[256u+c],1u);}
        else if value==0.0 {atomicAdd(&counters[260u+c],1u);}
        else {
            // Difference of logarithms avoids overflow/underflow in value/0.18.
            let position=(log2(value)-log2(0.18)+16.0)*2.0;
            if position<0.0 {atomicAdd(&counters[264u+c],1u);}
            else if position>=64.0 {atomicAdd(&counters[268u+c],1u);}
            else {atomicAdd(&counters[c*64u+u32(floor(position))],1u);}
        }
    }
}
