override SINGLE_LAYER: bool = false;
override RESIDENT_REGION: bool = false;

struct Parameters {
    viewport: vec2<f32>,
    image_size: vec2<f32>,
    pan: vec2<f32>,
    zoom: f32,
    exposure: f32,
    pixel_aspect: f32,
    background: vec4<f32>,
    raw_development: vec4<u32>,
    curve: array<vec4<f32>, 256>,
    resident_rect: vec4<u32>,
    comparison: vec4<f32>,
    sensor_geometry: vec4<u32>,
    crop: vec4<u32>,
};
@group(0) @binding(0) var image: texture_2d_array<f32>;
@group(0) @binding(1) var<uniform> p: Parameters;

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let points = array<vec2<f32>, 3>(vec2<f32>(-1.0,-1.0), vec2<f32>(3.0,-1.0), vec2<f32>(-1.0,3.0));
    return vec4<f32>(points[index],0.0,1.0);
}

fn premultiplied_at(coordinate: vec2<i32>) -> vec4<f32> {
    let upper = vec2<i32>(p.image_size) - vec2<i32>(1);
    var global = clamp(coordinate, vec2<i32>(0), upper);
    let x = global.x;
    let y = global.y;
    let cw = i32(p.crop.z);
    let ch = i32(p.crop.w);
    switch p.sensor_geometry.z {
        case 1u: { global = vec2<i32>(cw - 1 - x, y); }
        case 2u: { global = vec2<i32>(cw - 1 - x, ch - 1 - y); }
        case 3u: { global = vec2<i32>(x, ch - 1 - y); }
        case 4u: { global = vec2<i32>(y, x); }
        case 5u: { global = vec2<i32>(y, ch - 1 - x); }
        case 6u: { global = vec2<i32>(cw - 1 - y, ch - 1 - x); }
        case 7u: { global = vec2<i32>(cw - 1 - y, x); }
        default: {}
    }
    global += vec2<i32>(p.crop.xy);
    if RESIDENT_REGION { global -= vec2<i32>(p.resident_rect.xy); }
    // Most images occupy one layer: bypass integer division/remainder there.
    if SINGLE_LAYER {
        let color = textureLoad(image, global, 0, 0);
        return vec4<f32>(color.rgb * color.a, color.a);
    }
    let tile_size = vec2<i32>(textureDimensions(image, 0));
    let tile = global / tile_size;
    var resident_width = i32(p.sensor_geometry.x);
    if RESIDENT_REGION { resident_width = i32(p.resident_rect.z); }
    let columns = (resident_width + tile_size.x - 1) / tile_size.x;
    let layer = tile.y * columns + tile.x;
    let color = textureLoad(image, global % tile_size, layer, 0);
    return vec4<f32>(color.rgb * color.a, color.a);
}

fn aces_scalar(value: f32) -> f32 {
    let positive = max(value, 0.0);
    if positive>=7.3 {return 1.0;}
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
fn aces_tone_map(rgb: vec3<f32>, exposure: f32) -> vec3<f32> {
    let input_scale=max(abs(rgb.r),max(abs(rgb.g),abs(rgb.b)));
    if input_scale<=0.0 {return vec3<f32>(0.0);}
    var color = rgb/input_scale;
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
    return color * (aces_scalar(input_scale*norm*exp2(exposure)) / norm);
}

fn evaluate_curve(x:f32)->f32 {
    let last=p.raw_development.y-1u;
    if x<=p.curve[0].x {return p.curve[0].y;}
    if x>=p.curve[last].x {return p.curve[last].y;}
    var low=0u;var high=last;
    while low+1u<high {
        let mid=(low+high)/2u;
        if p.curve[mid].x<=x {low=mid;} else {high=mid;}
    }
    let a=p.curve[low];let b=p.curve[high];
    let h=b.x-a.x;let t=(x-a.x)/h;let t2=t*t;let t3=t2*t;
    let value=(2.0*t3-3.0*t2+1.0)*a.y+(t3-2.0*t2+t)*h*a.z
        +(-2.0*t3+3.0*t2)*b.y+(t3-t2)*h*b.z;
    return clamp(value,a.y,b.y);
}

// Exact box integration of displayed sensor pixels, including partial edge cells.
// Integer tap mapping applies crop/orientation before resolving source array layers.
fn area_color(location: vec2<f32>, footprint: vec2<f32>) -> vec4<f32> {
    let lower = max(vec2<f32>(0.0), location - footprint * 0.5);
    let upper = min(p.image_size, location + footprint * 0.5);
    let first = vec2<i32>(floor(lower));
    let end = vec2<i32>(ceil(upper));
    if p.sensor_geometry.w != 0u {
        // Native calibrated Bayer has a proven sample-range admission bound.
    var sum = vec4<f32>(0.0);
    var total = 0.0;
    for (var y = first.y; y < end.y; y += 1) {
        for (var x = first.x; x < end.x; x += 1) {
            let cell = vec2<f32>(f32(x), f32(y));
            let overlap = max(vec2<f32>(0.0), min(cell + vec2<f32>(1.0), upper) - max(cell, lower));
            let weight = overlap.x * overlap.y;
            sum += premultiplied_at(vec2<i32>(x, y)) * weight;
            total += weight;
        }
    }
    return sum / total;
    }
    let maximum = bitcast<f32>(0x7f7fffffu);
    let safe = maximum * 0.25;
    var sum = vec4<f32>(0.0);
    var total = 0.0;
    var exponents = vec3<i32>(0);
    var scaled = false;
    for (var y = first.y; y < end.y; y += 1) {
        for (var x = first.x; x < end.x; x += 1) {
            let cell = vec2<f32>(f32(x), f32(y));
            let overlap = max(vec2<f32>(0.0), min(cell + vec2<f32>(1.0), upper) - max(cell, lower));
            let weight = overlap.x * overlap.y;
            let pixel = premultiplied_at(vec2<i32>(x, y));
            if scaled || any(abs(sum.rgb) > vec3<f32>(safe)) || any(abs(pixel.rgb) > vec3<f32>(safe)) {
                // Promote before a sum can overflow. Per-channel binary scaling
                // preserves small green/blue values beside extreme red values.
                var magnitude = abs(pixel.rgb);
                if !scaled { magnitude = max(magnitude, abs(sum.rgb)); }
                let next = max(exponents, clamp(frexp(magnitude).exp, vec3<i32>(0), vec3<i32>(127)));
                sum = vec4<f32>(ldexp(sum.rgb, exponents - next) + ldexp(pixel.rgb, -next) * weight,
                    sum.a + pixel.a * weight);
                exponents = next;
                scaled = true;
            } else {
                sum += pixel * weight;
            }
            total += weight;
        }
    }
    var mean = sum / total;
    if scaled {
        let bound = ldexp(vec3<f32>(maximum), -exponents);
        mean = vec4<f32>(ldexp(clamp(mean.rgb, -bound, bound), exponents), mean.a);
    }
    return vec4<f32>(mean.rgb, clamp(mean.a, 0.0, 1.0));
}

@fragment
fn fs_main(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let shape = vec2<f32>(p.pixel_aspect, 1.0);
    let display_size = p.image_size * shape;
    let fit = min(max(p.viewport.x-32.0,1.0)/display_size.x, max(p.viewport.y-32.0,1.0)/display_size.y);
    let location = (position.xy - p.viewport*0.5 - p.pan) / (fit*p.zoom*shape) + p.image_size*0.5;
    if any(location < vec2<f32>(0.0)) || any(location >= p.image_size) {
        return p.background;
    }
    let footprint = vec2<f32>(1.0) / (fit * p.zoom * shape);
    var color: vec4<f32>;
    if p.raw_development.w != 0u && any(footprint > vec2<f32>(1.0)) {
        if RESIDENT_REGION {
            let first = vec2<i32>(floor(max(vec2<f32>(0.0), location - footprint * 0.5)));
            let end = vec2<i32>(ceil(min(p.image_size, location + footprint * 0.5)));
            let origin = vec2<i32>(p.resident_rect.xy);
            let resident_end = origin + vec2<i32>(p.resident_rect.zw);
            if any(first < origin) || any(end > resident_end) { return p.background; }
        }
        color = area_color(location, footprint);
    } else {
        let centered = location - vec2<f32>(0.5);
        let base = vec2<i32>(floor(centered));
        let weight = fract(centered);
        if RESIDENT_REGION {
            let upper = vec2<i32>(p.image_size) - vec2<i32>(1);
            let first = clamp(base, vec2<i32>(0), upper);
            let last = clamp(base + vec2<i32>(1), vec2<i32>(0), upper);
            let origin = vec2<i32>(p.resident_rect.xy);
            let end = origin + vec2<i32>(p.resident_rect.zw);
            if any(first < origin) || any(last >= end) { return p.background; }
        }
        // Premultiply before interpolation, so invisible RGB cannot make fringes.
        let top = mix(premultiplied_at(base), premultiplied_at(base+vec2<i32>(1,0)), weight.x);
        let bottom = mix(premultiplied_at(base+vec2<i32>(0,1)), premultiplied_at(base+vec2<i32>(1,1)), weight.x);
        color = mix(top,bottom,weight.y);
    }
    let before = p.comparison.x > 0.0 && position.x < p.viewport.x * p.comparison.x;
    let exposure = select(p.exposure, p.comparison.y, before);
    var rgb = color.rgb * exp2(exposure);
    if p.raw_development.x != 0u {
        rgb = aces_tone_map(color.rgb,exposure);
        let luminance = dot(rgb, vec3<f32>(0.2126,0.7152,0.0722));
        let curve_luminance=evaluate_curve(luminance);
        if luminance > 0.0 { rgb *= curve_luminance / luminance; } else { rgb=vec3<f32>(curve_luminance); }
        let peak = max(rgb.r,max(rgb.g,rgb.b));
        if peak > 1.0 { rgb=mix(rgb,vec3<f32>(curve_luminance),clamp((peak-1.0)/max(peak-curve_luminance,0.000001),0.0,1.0)); }
    }
    var result = rgb + p.background.rgb*(1.0-color.a);
    if p.raw_development.z != 0u {
        result = clamp(result, vec3<f32>(-65504.0), vec3<f32>(65504.0));
    }
    return vec4<f32>(result,1.0);
}
