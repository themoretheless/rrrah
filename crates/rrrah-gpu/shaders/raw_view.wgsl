struct Parameters {
    viewport: vec2<f32>,
    raw_size: vec2<u32>,
    texture_size: vec2<u32>,
    sample_stride: u32,
    tile_halo: u32,
    tile_grid: vec2<u32>,
    crop_origin: vec2<u32>,
    crop_size: vec2<u32>,
    pan: vec2<f32>,
    zoom: f32,
    exposure_stops: f32,
    cfa: vec4<u32>,
    black: vec4<f32>,
    white: vec4<f32>,
    white_balance: vec4<f32>,
    camera_to_rgb_0: vec4<f32>,
    camera_to_rgb_1: vec4<f32>,
    camera_to_rgb_2: vec4<f32>,
    orientation: u32,
    algorithm: u32,
    _padding: vec4<u32>,
};

@group(0) @binding(0) var raw_mosaic: texture_2d_array<u32>;
@group(0) @binding(1) var<uniform> parameters: Parameters;

struct VertexOutput { @builtin(position) position: vec4<f32> };

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    let positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0), vec2<f32>(3.0, -1.0), vec2<f32>(-1.0, 3.0)
    );
    var output: VertexOutput;
    output.position = vec4<f32>(positions[vertex_index], 0.0, 1.0);
    return output;
}

fn clamped_sensor_coordinate(position: vec2<i32>) -> vec2<i32> {
    let upper = vec2<i32>(parameters.raw_size) - vec2<i32>(1, 1);
    return clamp(position, vec2<i32>(0, 0), upper);
}

fn phase_index(position: vec2<i32>) -> u32 {
    let positive = clamped_sensor_coordinate(position);
    return u32(positive.y & 1) * 2u + u32(positive.x & 1);
}

fn cfa_color(position: vec2<i32>) -> u32 { return parameters.cfa[phase_index(position)]; }

fn sensor_sample(position: vec2<i32>) -> f32 {
    let coordinate = clamped_sensor_coordinate(position);
    let phase = phase_index(coordinate);
    // Keep all coordinate arithmetic signed. WGSL does not implicitly convert
    // the u32 uniform fields to i32, and relying on a backend's permissive
    // parser would make shader validation/translation non-portable.
    let stride = vec2<i32>(i32(parameters.sample_stride));
    let halo = vec2<i32>(i32(parameters.tile_halo));
    let tile = coordinate / stride;
    let local = (coordinate % stride) + halo;
    let grid = vec2<i32>(i32(parameters.tile_grid.x), i32(parameters.tile_grid.y));
    let layer = tile.y * grid.x + tile.x;
    let encoded = f32(textureLoad(
        raw_mosaic,
        vec2<i32>(local),
        i32(layer),
        0,
    ).r);
    let black = parameters.black[phase];
    let range = select(max(parameters.white[phase] - black, 1.0), parameters.white[phase] - black, parameters._padding.x == 1u);
    return max(encoded - black, 0.0) / range;
}

fn normalized_sample(position: vec2<i32>) -> f32 {
    // WB belongs to each photosite before interpolation, including G2.
    let phase = phase_index(position);
    let color = parameters.cfa[phase];
    let first_green = select(1u, 0u, parameters.cfa[0] == 1u);
    let gain = select(parameters.white_balance[color], parameters.white_balance.w,
        color == 1u && phase != first_green);
    return sensor_sample(position) * gain;
}

fn bilinear_demosaic(position: vec2<i32>) -> vec3<f32> {
    let center = normalized_sample(position);
    let north = normalized_sample(position + vec2<i32>(0, -1));
    let south = normalized_sample(position + vec2<i32>(0, 1));
    let west = normalized_sample(position + vec2<i32>(-1, 0));
    let east = normalized_sample(position + vec2<i32>(1, 0));
    let north_west = normalized_sample(position + vec2<i32>(-1, -1));
    let north_east = normalized_sample(position + vec2<i32>(1, -1));
    let south_west = normalized_sample(position + vec2<i32>(-1, 1));
    let south_east = normalized_sample(position + vec2<i32>(1, 1));
    let axial = 0.25 * (north + south + west + east);
    let diagonal = 0.25 * (north_west + north_east + south_west + south_east);
    let color = cfa_color(position);
    if color == 0u { return vec3<f32>(center, axial, diagonal); }
    if color == 2u { return vec3<f32>(diagonal, axial, center); }
    let horizontal_color = cfa_color(position + vec2<i32>(1, 0));
    if horizontal_color == 0u {
        return vec3<f32>(0.5 * (west + east), center, 0.5 * (north + south));
    }
    return vec3<f32>(0.5 * (north + south), center, 0.5 * (west + east));
}

fn inverse_orientation(display_uv: vec2<f32>) -> vec2<f32> {
    switch parameters.orientation {
        case 1u: { return vec2<f32>(1.0 - display_uv.x, display_uv.y); }
        case 2u: { return vec2<f32>(1.0 - display_uv.x, 1.0 - display_uv.y); }
        case 3u: { return vec2<f32>(display_uv.x, 1.0 - display_uv.y); }
        case 4u: { return vec2<f32>(display_uv.y, display_uv.x); }
        case 5u: { return vec2<f32>(display_uv.y, 1.0 - display_uv.x); }
        case 6u: { return vec2<f32>(1.0 - display_uv.y, 1.0 - display_uv.x); }
        case 7u: { return vec2<f32>(1.0 - display_uv.y, display_uv.x); }
        default: { return display_uv; }
    }
}

fn camera_rgb_at(raw_position: vec2<f32>) -> vec3<f32> {
    let position = vec2<i32>(round(raw_position));
    let rgb = bilinear_demosaic(position);
    let quad = position - (position % vec2<i32>(2));
    let clipped = sensor_sample(quad) >= 1.0 || sensor_sample(quad + vec2<i32>(1,0)) >= 1.0
        || sensor_sample(quad + vec2<i32>(0,1)) >= 1.0 || sensor_sample(quad + vec2<i32>(1,1)) >= 1.0;
    let minimum = min(rgb.r,min(rgb.g,rgb.b));
    if clipped && minimum >= 1.0 && parameters._padding.y == 0u { return vec3<f32>(minimum); }
    return rgb;
}

// Integrate complete Bayer cells over a destination pixel's sensor footprint.
// Sampling isolated photosites at a large stride aliases CFA phase and edges.
fn quad_camera_rgb(origin: vec2<i32>) -> vec3<f32> {
    let offsets = array<vec2<i32>,4>(vec2<i32>(0,0),vec2<i32>(1,0),vec2<i32>(0,1),vec2<i32>(1,1));
    var rgb = vec3<f32>(0.0);
    var clipped = false;
    let first_green = select(1u,0u,parameters.cfa[0] == 1u);
    for (var phase = 0u; phase < 4u; phase += 1u) {
        let raw = sensor_sample(origin+offsets[phase]);
        let color = parameters.cfa[phase];
        let gain = select(parameters.white_balance[color],parameters.white_balance.w,color == 1u && phase != first_green);
        rgb[color] += raw*gain*select(1.0,0.5,color == 1u);
        clipped = clipped || raw >= 1.0;
    }
    let minimum = min(rgb.r,min(rgb.g,rgb.b));
    if clipped && minimum >= 1.0 && parameters._padding.y == 0u { return vec3<f32>(minimum); }
    return rgb;
}

fn area_camera_rgb(position: vec2<f32>, distance: f32) -> vec3<f32> {
    let center = position+vec2<f32>(0.5);
    let lower = max(vec2<f32>(parameters.crop_origin),center-vec2<f32>(distance*0.5));
    let upper = min(vec2<f32>(parameters.crop_origin+parameters.crop_size),center+vec2<f32>(distance*0.5));
    let start = (vec2<i32>(floor(lower))/2)*2;
    let end = vec2<i32>(ceil(upper));
    var sum = vec3<f32>(0.0);
    var total = 0.0;
    for (var y = start.y; y < end.y; y += 2) {
        for (var x = start.x; x < end.x; x += 2) {
            let origin = vec2<i32>(x,y);
            let overlap = max(vec2<f32>(0.0),min(vec2<f32>(origin+vec2<i32>(2)),upper)-max(vec2<f32>(origin),lower));
            let weight = overlap.x*overlap.y;
            sum += quad_camera_rgb(origin)*weight;
            total += weight;
        }
    }
    return sum/max(total,0.00000001);
}

// RGBE uses four independent two-pixel lattices, including at sensor edges.
fn rgbe_axis(coordinate:i32,phase:i32,size:i32)->vec3<f32> {
    let last=phase+((size-1-phase)/2)*2;
    let lower=min(phase+(max(coordinate-phase,0)/2)*2,last);
    let upper=min(lower+2,last);
    var weight=0.0;
    if upper>lower { weight=clamp(f32(coordinate-lower)/f32(upper-lower),0.0,1.0); }
    return vec3<f32>(f32(lower),f32(upper),weight);
}
fn rgbe_planes(position:vec2<i32>)->vec4<f32> {
    var planes=vec4<f32>(0.0);
    let coordinate=clamped_sensor_coordinate(position);
    for(var phase=0u;phase<4u;phase+=1u) {
        let x=rgbe_axis(coordinate.x,i32(phase%2u),i32(parameters.raw_size.x));
        let y=rgbe_axis(coordinate.y,i32(phase/2u),i32(parameters.raw_size.y));
        let a=sensor_sample(vec2<i32>(i32(x.x),i32(y.x)));
        let b=sensor_sample(vec2<i32>(i32(x.y),i32(y.x)));
        let c=sensor_sample(vec2<i32>(i32(x.x),i32(y.y)));
        let d=sensor_sample(vec2<i32>(i32(x.y),i32(y.y)));
        let channel=parameters.cfa[phase];
        planes[channel]=mix(mix(a,b,x.z),mix(c,d,x.z),y.z)*parameters.white_balance[channel];
    }
    return planes;
}
fn rgbe_filtered(position:vec2<f32>,distance:f32)->vec4<f32> {
    if distance<2.0 { return rgbe_planes(vec2<i32>(round(position))); }
    let center=position+vec2<f32>(0.5);
    let lower=max(vec2<f32>(parameters.crop_origin),center-vec2<f32>(distance*0.5));
    let upper=min(vec2<f32>(parameters.crop_origin+parameters.crop_size),center+vec2<f32>(distance*0.5));
    let start=(vec2<i32>(floor(lower))/2)*2;
    let end=vec2<i32>(ceil(upper));
    var sum=vec4<f32>(0.0);
    var total=0.0;
    for(var y=start.y;y<end.y;y+=2) { for(var x=start.x;x<end.x;x+=2) {
        let origin=vec2<i32>(x,y);
        let overlap=max(vec2<f32>(0.0),min(vec2<f32>(origin+vec2<i32>(2)),upper)-max(vec2<f32>(origin),lower));
        let weight=overlap.x*overlap.y;
        var planes=vec4<f32>(0.0);
        for(var phase=0u;phase<4u;phase+=1u) {
            let channel=parameters.cfa[phase];
            let offset=vec2<i32>(i32(phase%2u),i32(phase/2u));
            let last=offset+((vec2<i32>(parameters.raw_size)-vec2<i32>(1)-offset)/2)*2;
            planes[channel]=sensor_sample(min(origin+offset,last))*parameters.white_balance[channel];
        }
        sum+=planes*weight; total+=weight;
    }}
    return sum/max(total,0.00000001);
}
fn developed_rgb_at(raw_position: vec2<f32>, raw_pixel_footprint: f32) -> vec3<f32> {
    if parameters._padding.x == 1u {
        let planes=rgbe_filtered(raw_position,raw_pixel_footprint);
        return vec3<f32>(dot(parameters.camera_to_rgb_0,planes),dot(parameters.camera_to_rgb_1,planes),dot(parameters.camera_to_rgb_2,planes))*exp2(parameters.exposure_stops);
    }
    var camera_rgb: vec3<f32>;
    if raw_pixel_footprint >= 2.0 {
        camera_rgb = area_camera_rgb(raw_position,raw_pixel_footprint);
    } else if raw_pixel_footprint > 1.5 {
        let distance = min(raw_pixel_footprint * 0.25, 12.0);
        camera_rgb = 0.25 * (
            camera_rgb_at(raw_position + vec2<f32>(-distance, -distance)) +
            camera_rgb_at(raw_position + vec2<f32>(distance, -distance)) +
            camera_rgb_at(raw_position + vec2<f32>(-distance, distance)) +
            camera_rgb_at(raw_position + vec2<f32>(distance, distance))
        );
    } else { camera_rgb = camera_rgb_at(raw_position); }
    let linear_rgb = vec3<f32>(
        dot(parameters.camera_to_rgb_0.xyz, camera_rgb),
        dot(parameters.camera_to_rgb_1.xyz, camera_rgb),
        dot(parameters.camera_to_rgb_2.xyz, camera_rgb),
    );
    return linear_rgb * exp2(parameters.exposure_stops);
}

fn aces_scalar(value: f32) -> f32 {
    let positive = max(value, 0.0);
    let numerator = positive * (2.51 * positive + 0.03);
    let denominator = positive * (2.43 * positive + 0.59) + 0.14;
    return clamp(numerator / denominator, 0.0, 1.0);
}

// Hue-preserving ACES tone/gamut mapping (color question #4). Mirrors
// `rrrah_core::aces_tone_map_rgb`; see `docs/experiments/SUMMARY.md`.
//
// 1. Sub-zero components (out-of-gamut camera colors) are desaturated toward
//    the Rec.709 luma axis until the lowest channel reaches the gamut
//    boundary, instead of being clamped to zero (which rotated hue).
// 2. The achromatic max-component norm runs through the ACES fitted curve
//    and the triplet scales by aces(norm)/norm — one common positive factor,
//    so hue and channel ratios are preserved by construction (CIELAB h° is
//    invariant under linear-RGB scaling). Achromatic inputs reduce to the
//    previous per-channel curve exactly.
// 3. The norm is the max component and aces_scalar <= 1, so the output never
//    leaves the display gamut: no post-tone-map clipping is needed.
fn aces_tone_map(rgb: vec3<f32>) -> vec3<f32> {
    var color = rgb;
    let minimum = min(color.r, min(color.g, color.b));
    if minimum < 0.0 {
        let luma = dot(color, vec3<f32>(0.2126, 0.7152, 0.0722));
        if luma > 0.0 {
            // min + t * (luma - min) = 0  ->  t = -min / (luma - min).
            let desaturate = clamp(-minimum / (luma - minimum), 0.0, 1.0);
            color = mix(color, vec3<f32>(luma), desaturate);
        } else {
            color = max(color, vec3<f32>(0.0));
        }
    }
    let norm = max(color.r, max(color.g, color.b));
    if norm <= 0.0 {
        return vec3<f32>(0.0);
    }
    return color * (aces_scalar(norm) / norm);
}

@fragment
fn fs_main(@builtin(position) fragment: vec4<f32>) -> @location(0) vec4<f32> {
    let swaps_dimensions = parameters.orientation >= 4u;
    var oriented_size = vec2<f32>(parameters.crop_size);
    if swaps_dimensions { oriented_size = oriented_size.yx; }
    let available = max(parameters.viewport - vec2<f32>(32.0), vec2<f32>(1.0));
    let fit_scale = min(available.x / oriented_size.x, available.y / oriented_size.y);
    let scale = max(fit_scale * parameters.zoom, 0.000001);
    let display_size = oriented_size * scale;
    let image_center = 0.5 * parameters.viewport + parameters.pan;
    let display_uv = (fragment.xy - image_center) / display_size + vec2<f32>(0.5);
    if any(display_uv < vec2<f32>(0.0)) || any(display_uv > vec2<f32>(1.0)) {
        return vec4<f32>(0.012, 0.014, 0.018, 1.0);
    }
    let raw_uv = inverse_orientation(display_uv);
    let crop_extent = max(vec2<f32>(parameters.crop_size) - vec2<f32>(1.0), vec2<f32>(0.0));
    let raw_position = vec2<f32>(parameters.crop_origin) + raw_uv * crop_extent;
    let linear_rgb = developed_rgb_at(raw_position, 1.0 / scale);
    if parameters._padding.y == 1u { return vec4<f32>(linear_rgb, 1.0); }
    return vec4<f32>(aces_tone_map(linear_rgb), 1.0);
}
