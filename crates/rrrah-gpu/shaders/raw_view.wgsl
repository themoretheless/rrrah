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

fn normalized_sample(position: vec2<i32>) -> f32 {
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
    let range = max(parameters.white[phase] - black, 1.0);
    return max(encoded - black, 0.0) / range;
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
        case 5u: { return vec2<f32>(1.0 - display_uv.y, display_uv.x); }
        case 6u: { return vec2<f32>(1.0 - display_uv.y, 1.0 - display_uv.x); }
        case 7u: { return vec2<f32>(display_uv.y, 1.0 - display_uv.x); }
        default: { return display_uv; }
    }
}

fn camera_rgb_at(raw_position: vec2<f32>) -> vec3<f32> {
    return bilinear_demosaic(vec2<i32>(round(raw_position)));
}

fn developed_rgb_at(raw_position: vec2<f32>, raw_pixel_footprint: f32) -> vec3<f32> {
    var camera_rgb: vec3<f32>;
    if raw_pixel_footprint > 1.5 {
        let distance = min(raw_pixel_footprint * 0.25, 12.0);
        camera_rgb = 0.25 * (
            camera_rgb_at(raw_position + vec2<f32>(-distance, -distance)) +
            camera_rgb_at(raw_position + vec2<f32>(distance, -distance)) +
            camera_rgb_at(raw_position + vec2<f32>(-distance, distance)) +
            camera_rgb_at(raw_position + vec2<f32>(distance, distance))
        );
    } else { camera_rgb = camera_rgb_at(raw_position); }
    camera_rgb *= parameters.white_balance.xyz;
    let linear_rgb = vec3<f32>(
        dot(parameters.camera_to_rgb_0.xyz, camera_rgb),
        dot(parameters.camera_to_rgb_1.xyz, camera_rgb),
        dot(parameters.camera_to_rgb_2.xyz, camera_rgb),
    );
    return linear_rgb * exp2(parameters.exposure_stops);
}

fn aces_fitted(value: vec3<f32>) -> vec3<f32> {
    let positive = max(value, vec3<f32>(0.0));
    let numerator = positive * (2.51 * positive + vec3<f32>(0.03));
    let denominator = positive * (2.43 * positive + vec3<f32>(0.59)) + vec3<f32>(0.14);
    return clamp(numerator / denominator, vec3<f32>(0.0), vec3<f32>(1.0));
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
    return vec4<f32>(aces_fitted(linear_rgb), 1.0);
}
