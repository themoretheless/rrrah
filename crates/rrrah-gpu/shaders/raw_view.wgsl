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
@group(0) @binding(2) var<storage, read> black_grid: array<f32>;

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
    var black = parameters.black[phase];
    if parameters._padding.z != 0u {
        let x = u32(coordinate.x) % parameters._padding.z;
        let y = u32(coordinate.y) % parameters._padding.w;
        black = black_grid[y * parameters._padding.z + x];
    }
    // Upload validates a finite, strictly positive range at every grid/CFA intersection.
    let range = parameters.white[phase] - black;
    let numerator = max(encoded - black, 0.0);
    if numerator == 0.0 { return 0.0; }
    return numerator / range;
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
    // CFA direction follows the repeating lattice even when the adjacent
    // sample lies outside the sensor. Clamp sample reads, not lattice phase.
    let horizontal_phase = u32(position.y & 1) * 2u + u32((position.x + 1) & 1);
    let horizontal_color = parameters.cfa[horizontal_phase];
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

fn malvar_he_cutler_demosaic(position: vec2<i32>) -> vec3<f32> {
    let c = normalized_sample(position);
    let n1 = normalized_sample(position + vec2<i32>(0, -1));
    let s1 = normalized_sample(position + vec2<i32>(0, 1));
    let w1 = normalized_sample(position + vec2<i32>(-1, 0));
    let e1 = normalized_sample(position + vec2<i32>(1, 0));
    let n2 = normalized_sample(position + vec2<i32>(0, -2));
    let s2 = normalized_sample(position + vec2<i32>(0, 2));
    let w2 = normalized_sample(position + vec2<i32>(-2, 0));
    let e2 = normalized_sample(position + vec2<i32>(2, 0));
    let nw = normalized_sample(position + vec2<i32>(-1, -1));
    let ne = normalized_sample(position + vec2<i32>(1, -1));
    let sw = normalized_sample(position + vec2<i32>(-1, 1));
    let se = normalized_sample(position + vec2<i32>(1, 1));

    let color = cfa_color(position);
    if color == 0u || color == 2u {
        let g_min = min(min(n1, s1), min(w1, e1));
        let g_max = max(max(n1, s1), max(w1, e1));
        let g_est = 0.25 * (n1 + s1 + w1 + e1) + 0.125 * (4.0 * c - (n2 + s2 + w2 + e2));
        let g = clamp(g_est, g_min, g_max);

        let opp_min = min(min(nw, ne), min(sw, se));
        let opp_max = max(max(nw, ne), max(sw, se));
        let opp_est = 0.25 * (nw + ne + sw + se) + 0.1875 * (4.0 * c - (n2 + s2 + w2 + e2));
        let opp = clamp(opp_est, opp_min, opp_max);

        if color == 0u { return vec3<f32>(c, g, opp); }
        return vec3<f32>(opp, g, c);
    }

    let h_est = 0.5 * (w1 + e1) + 0.125 * (5.0 * c - (nw + ne + sw + se) - (w2 + e2) + 0.5 * (n2 + s2));
    let ch = clamp(h_est, min(w1, e1), max(w1, e1));
    let v_est = 0.5 * (n1 + s1) + 0.125 * (5.0 * c - (nw + ne + sw + se) - (n2 + s2) + 0.5 * (w2 + e2));
    let cv = clamp(v_est, min(n1, s1), max(n1, s1));

    let horizontal_phase = u32(position.y & 1) * 2u + u32((position.x + 1) & 1);
    let horizontal_color = parameters.cfa[horizontal_phase];
    if horizontal_color == 0u {
        return vec3<f32>(ch, c, cv);
    }
    return vec3<f32>(cv, c, ch);
}

fn green_at_rb(p: vec2<i32>, c: f32) -> f32 {
    let n1 = normalized_sample(p + vec2<i32>(0, -1));
    let s1 = normalized_sample(p + vec2<i32>(0, 1));
    let w1 = normalized_sample(p + vec2<i32>(-1, 0));
    let e1 = normalized_sample(p + vec2<i32>(1, 0));
    let n2 = normalized_sample(p + vec2<i32>(0, -2));
    let s2 = normalized_sample(p + vec2<i32>(0, 2));
    let w2 = normalized_sample(p + vec2<i32>(-2, 0));
    let e2 = normalized_sample(p + vec2<i32>(2, 0));
    let gh = clamp(0.5 * (w1 + e1) + 0.25 * (2.0 * c - w2 - e2), min(w1, e1), max(w1, e1));
    let gv = clamp(0.5 * (n1 + s1) + 0.25 * (2.0 * c - n2 - s2), min(n1, s1), max(n1, s1));
    let grad_h = abs(w1 - e1) + abs(2.0 * c - w2 - e2);
    let grad_v = abs(n1 - s1) + abs(2.0 * c - n2 - s2);
    let wh = 1.0 / (0.00001 + grad_h);
    let wv = 1.0 / (0.00001 + grad_v);
    return (wh * gh + wv * gv) / (wh + wv);
}

fn directional_demosaic(position: vec2<i32>) -> vec3<f32> {
    let c = normalized_sample(position);
    let color = cfa_color(position);
    if color == 0u || color == 2u {
        let gp = green_at_rb(position, c);
        let p_nw = position + vec2<i32>(-1, -1);
        let p_ne = position + vec2<i32>(1, -1);
        let p_sw = position + vec2<i32>(-1, 1);
        let p_se = position + vec2<i32>(1, 1);
        let nw = normalized_sample(p_nw);
        let ne = normalized_sample(p_ne);
        let sw = normalized_sample(p_sw);
        let se = normalized_sample(p_se);
        let d_nw = nw - 0.5 * (normalized_sample(position + vec2<i32>(0, -1)) + normalized_sample(position + vec2<i32>(-1, 0)));
        let d_se = se - 0.5 * (normalized_sample(position + vec2<i32>(0, 1)) + normalized_sample(position + vec2<i32>(1, 0)));
        let d_ne = ne - 0.5 * (normalized_sample(position + vec2<i32>(0, -1)) + normalized_sample(position + vec2<i32>(1, 0)));
        let d_sw = sw - 0.5 * (normalized_sample(position + vec2<i32>(0, 1)) + normalized_sample(position + vec2<i32>(-1, 0)));
        let w1 = 1.0 / (0.00001 + abs(nw - se) + abs(d_nw - d_se));
        let w2 = 1.0 / (0.00001 + abs(ne - sw) + abs(d_ne - d_sw));
        let opp_min = min(min(nw, ne), min(sw, se));
        let opp_max = max(max(nw, ne), max(sw, se));
        let opp = clamp(gp + (w1 * 0.5 * (d_nw + d_se) + w2 * 0.5 * (d_ne + d_sw)) / (w1 + w2), opp_min, opp_max);
        if color == 0u { return vec3<f32>(c, gp, opp); }
        return vec3<f32>(opp, gp, c);
    }

    let w1 = normalized_sample(position + vec2<i32>(-1, 0));
    let e1 = normalized_sample(position + vec2<i32>(1, 0));
    let n1 = normalized_sample(position + vec2<i32>(0, -1));
    let s1 = normalized_sample(position + vec2<i32>(0, 1));
    let gw = green_at_rb(position + vec2<i32>(-1, 0), w1);
    let ge = green_at_rb(position + vec2<i32>(1, 0), e1);
    let gn = green_at_rb(position + vec2<i32>(0, -1), n1);
    let gs = green_at_rb(position + vec2<i32>(0, 1), s1);

    let ch = clamp(c + 0.5 * ((w1 - gw) + (e1 - ge)), min(w1, e1), max(w1, e1));
    let cv = clamp(c + 0.5 * ((n1 - gn) + (s1 - gs)), min(n1, s1), max(n1, s1));

    let horizontal_phase = u32(position.y & 1) * 2u + u32((position.x + 1) & 1);
    let horizontal_color = parameters.cfa[horizontal_phase];
    if horizontal_color == 0u {
        return vec3<f32>(ch, c, cv);
    }
    return vec3<f32>(cv, c, ch);
}

fn smooth_hue_demosaic(position: vec2<i32>) -> vec3<f32> {
    let eps = 0.001;
    let c = normalized_sample(position);
    let color = cfa_color(position);
    let n1 = normalized_sample(position + vec2<i32>(0, -1));
    let s1 = normalized_sample(position + vec2<i32>(0, 1));
    let w1 = normalized_sample(position + vec2<i32>(-1, 0));
    let e1 = normalized_sample(position + vec2<i32>(1, 0));
    if color == 0u || color == 2u {
        let gp = 0.25 * (n1 + s1 + w1 + e1);
        let nw = normalized_sample(position + vec2<i32>(-1, -1));
        let ne = normalized_sample(position + vec2<i32>(1, -1));
        let sw = normalized_sample(position + vec2<i32>(-1, 1));
        let se = normalized_sample(position + vec2<i32>(1, 1));
        let gnw = 0.25 * (normalized_sample(position + vec2<i32>(-1, -2)) + w1 + normalized_sample(position + vec2<i32>(-2, -1)) + n1);
        let gne = 0.25 * (normalized_sample(position + vec2<i32>(1, -2)) + e1 + n1 + normalized_sample(position + vec2<i32>(2, -1)));
        let gsw = 0.25 * (w1 + normalized_sample(position + vec2<i32>(-1, 2)) + normalized_sample(position + vec2<i32>(-2, 1)) + s1);
        let gse = 0.25 * (e1 + normalized_sample(position + vec2<i32>(1, 2)) + s1 + normalized_sample(position + vec2<i32>(2, 1)));
        let r_avg = 0.25 * (
            (nw + eps) / (gnw + eps) +
            (ne + eps) / (gne + eps) +
            (sw + eps) / (gsw + eps) +
            (se + eps) / (gse + eps)
        );
        let opp = clamp((gp + eps) * r_avg - eps, min(min(nw, ne), min(sw, se)), max(max(nw, ne), max(sw, se)));
        if color == 0u { return vec3<f32>(c, gp, opp); }
        return vec3<f32>(opp, gp, c);
    }
    let gw = 0.25 * (normalized_sample(position + vec2<i32>(-1, -1)) + normalized_sample(position + vec2<i32>(-1, 1)) + normalized_sample(position + vec2<i32>(-2, 0)) + c);
    let ge = 0.25 * (normalized_sample(position + vec2<i32>(1, -1)) + normalized_sample(position + vec2<i32>(1, 1)) + c + normalized_sample(position + vec2<i32>(2, 0)));
    let gn = 0.25 * (normalized_sample(position + vec2<i32>(0, -2)) + c + normalized_sample(position + vec2<i32>(-1, -1)) + normalized_sample(position + vec2<i32>(1, -1)));
    let gs = 0.25 * (c + normalized_sample(position + vec2<i32>(0, 2)) + normalized_sample(position + vec2<i32>(-1, 1)) + normalized_sample(position + vec2<i32>(1, 1)));
    let rh = 0.5 * ((w1 + eps) / (gw + eps) + (e1 + eps) / (ge + eps));
    let rv = 0.5 * ((n1 + eps) / (gn + eps) + (s1 + eps) / (gs + eps));
    let ch = clamp((c + eps) * rh - eps, min(w1, e1), max(w1, e1));
    let cv = clamp((c + eps) * rv - eps, min(n1, s1), max(n1, s1));
    let horizontal_phase = u32(position.y & 1) * 2u + u32((position.x + 1) & 1);
    let horizontal_color = parameters.cfa[horizontal_phase];
    if horizontal_color == 0u { return vec3<f32>(ch, c, cv); }
    return vec3<f32>(cv, c, ch);
}

fn vng_demosaic(position: vec2<i32>) -> vec3<f32> {
    let c = normalized_sample(position);
    let n1 = normalized_sample(position + vec2<i32>(0, -1));
    let s1 = normalized_sample(position + vec2<i32>(0, 1));
    let w1 = normalized_sample(position + vec2<i32>(-1, 0));
    let e1 = normalized_sample(position + vec2<i32>(1, 0));
    let nw = normalized_sample(position + vec2<i32>(-1, -1));
    let ne = normalized_sample(position + vec2<i32>(1, -1));
    let sw = normalized_sample(position + vec2<i32>(-1, 1));
    let se = normalized_sample(position + vec2<i32>(1, 1));
    let n2 = normalized_sample(position + vec2<i32>(0, -2));
    let s2 = normalized_sample(position + vec2<i32>(0, 2));
    let w2 = normalized_sample(position + vec2<i32>(-2, 0));
    let e2 = normalized_sample(position + vec2<i32>(2, 0));
    let gn = abs(n1 - s1) + abs(n2 - c) + 0.5 * (abs(nw - sw) + abs(ne - se));
    let ge = abs(e1 - w1) + abs(e2 - c) + 0.5 * (abs(ne - nw) + abs(se - sw));
    let gs = abs(s1 - n1) + abs(s2 - c) + 0.5 * (abs(sw - nw) + abs(se - ne));
    let gw = abs(w1 - e1) + abs(w2 - c) + 0.5 * (abs(nw - ne) + abs(sw - se));
    let g_min = min(min(gn, ge), min(gs, gw));
    let g_max = max(max(gn, ge), max(gs, gw));
    let thresh = 1.5 * g_min + 0.5 * (g_max - g_min) + 0.00001;

    let color = cfa_color(position);
    if color == 0u || color == 2u {
        var g_sum = 0.0;
        var g_cnt = 0.0;
        if gn <= thresh { g_sum += n1 + 0.5 * (c - n2); g_cnt += 1.0; }
        if ge <= thresh { g_sum += e1 + 0.5 * (c - e2); g_cnt += 1.0; }
        if gs <= thresh { g_sum += s1 + 0.5 * (c - s2); g_cnt += 1.0; }
        if gw <= thresh { g_sum += w1 + 0.5 * (c - w2); g_cnt += 1.0; }
        let gp = clamp(g_sum / max(g_cnt, 1.0), min(min(n1, s1), min(w1, e1)), max(max(n1, s1), max(w1, e1)));
        let gnw = 0.5 * (n1 + w1);
        let gne = 0.5 * (n1 + e1);
        let gsw = 0.5 * (s1 + w1);
        let gse = 0.5 * (s1 + e1);
        let opp_est = gp + 0.25 * ((nw - gnw) + (ne - gne) + (sw - gsw) + (se - gse));
        let opp = clamp(opp_est, min(min(nw, ne), min(sw, se)), max(max(nw, ne), max(sw, se)));
        if color == 0u { return vec3<f32>(c, gp, opp); }
        return vec3<f32>(opp, gp, c);
    }
    return directional_demosaic(position);
}

fn rcd_green_at_rb(p: vec2<i32>, c: f32) -> f32 {
    let eps = 0.00001;
    let n1 = normalized_sample(p + vec2<i32>(0, -1));
    let s1 = normalized_sample(p + vec2<i32>(0, 1));
    let w1 = normalized_sample(p + vec2<i32>(-1, 0));
    let e1 = normalized_sample(p + vec2<i32>(1, 0));
    let n2 = normalized_sample(p + vec2<i32>(0, -2));
    let s2 = normalized_sample(p + vec2<i32>(0, 2));
    let w2 = normalized_sample(p + vec2<i32>(-2, 0));
    let e2 = normalized_sample(p + vec2<i32>(2, 0));
    let n3 = normalized_sample(p + vec2<i32>(0, -3));
    let s3 = normalized_sample(p + vec2<i32>(0, 3));
    let w3 = normalized_sample(p + vec2<i32>(-3, 0));
    let e3 = normalized_sample(p + vec2<i32>(3, 0));
    let v_stat = abs(n1 - s1) + abs(c - n2) + abs(c - s2) + 0.5 * (abs(n1 - n3) + abs(s1 - s3));
    let h_stat = abs(w1 - e1) + abs(c - w2) + abs(c - e2) + 0.5 * (abs(w1 - w3) + abs(e1 - e3));
    let wv = 1.0 / ((eps + v_stat) * (eps + v_stat));
    let wh = 1.0 / ((eps + h_stat) * (eps + h_stat));
    let gv = clamp(0.5 * (n1 + s1) + 0.25 * (2.0 * c - n2 - s2), min(n1, s1), max(n1, s1));
    let gh = clamp(0.5 * (w1 + e1) + 0.25 * (2.0 * c - w2 - e2), min(w1, e1), max(w1, e1));
    return (wh * gh + wv * gv) / (wh + wv);
}

fn rcd_demosaic(position: vec2<i32>) -> vec3<f32> {
    let c = normalized_sample(position);
    let color = cfa_color(position);
    if color == 0u || color == 2u {
        let gp = rcd_green_at_rb(position, c);
        let nw = normalized_sample(position + vec2<i32>(-1, -1));
        let ne = normalized_sample(position + vec2<i32>(1, -1));
        let sw = normalized_sample(position + vec2<i32>(-1, 1));
        let se = normalized_sample(position + vec2<i32>(1, 1));
        let gnw = rcd_green_at_rb(position + vec2<i32>(-1, -1), nw);
        let gne = rcd_green_at_rb(position + vec2<i32>(1, -1), ne);
        let gsw = rcd_green_at_rb(position + vec2<i32>(-1, 1), sw);
        let gse = rcd_green_at_rb(position + vec2<i32>(1, 1), se);
        let d_nw = nw - gnw;
        let d_se = se - gse;
        let d_ne = ne - gne;
        let d_sw = sw - gsw;
        let w1 = 1.0 / (0.00001 + abs(nw - se) + abs(d_nw - d_se));
        let w2 = 1.0 / (0.00001 + abs(ne - sw) + abs(d_ne - d_sw));
        let opp_est = gp + (w1 * 0.5 * (d_nw + d_se) + w2 * 0.5 * (d_ne + d_sw)) / (w1 + w2);
        let opp = clamp(opp_est, min(min(nw, ne), min(sw, se)), max(max(nw, ne), max(sw, se)));
        if color == 0u { return vec3<f32>(c, gp, opp); }
        return vec3<f32>(opp, gp, c);
    }
    let w1 = normalized_sample(position + vec2<i32>(-1, 0));
    let e1 = normalized_sample(position + vec2<i32>(1, 0));
    let n1 = normalized_sample(position + vec2<i32>(0, -1));
    let s1 = normalized_sample(position + vec2<i32>(0, 1));
    let gw = rcd_green_at_rb(position + vec2<i32>(-1, 0), w1);
    let ge = rcd_green_at_rb(position + vec2<i32>(1, 0), e1);
    let gn = rcd_green_at_rb(position + vec2<i32>(0, -1), n1);
    let gs = rcd_green_at_rb(position + vec2<i32>(0, 1), s1);
    let ch = clamp(c + 0.5 * ((w1 - gw) + (e1 - ge)), min(w1, e1), max(w1, e1));
    let cv = clamp(c + 0.5 * ((n1 - gn) + (s1 - gs)), min(n1, s1), max(n1, s1));
    let horizontal_phase = u32(position.y & 1) * 2u + u32((position.x + 1) & 1);
    let horizontal_color = parameters.cfa[horizontal_phase];
    if horizontal_color == 0u { return vec3<f32>(ch, c, cv); }
    return vec3<f32>(cv, c, ch);
}

fn camera_rgb_at(raw_position: vec2<f32>) -> vec3<f32> {
    let position = vec2<i32>(round(raw_position));
    var rgb: vec3<f32>;
    if parameters.algorithm == 1u && parameters.tile_halo >= 2u {
        rgb = malvar_he_cutler_demosaic(position);
    } else if parameters.algorithm == 2u && parameters.tile_halo >= 2u {
        rgb = directional_demosaic(position);
    } else if parameters.algorithm == 3u && parameters.tile_halo >= 2u {
        rgb = smooth_hue_demosaic(position);
    } else if parameters.algorithm == 4u && parameters.tile_halo >= 2u {
        rgb = vng_demosaic(position);
    } else if parameters.algorithm == 5u && parameters.tile_halo >= 3u {
        rgb = rcd_demosaic(position);
    } else {
        rgb = bilinear_demosaic(position);
    }
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
    // Pixel centers span the full crop extent. Area filters add 0.5 to
    // recover sensor-cell boundaries; shrinking by one distorts footprints.
    let raw_position = vec2<f32>(parameters.crop_origin)
        + raw_uv * vec2<f32>(parameters.crop_size) - vec2<f32>(0.5);
    let linear_rgb = developed_rgb_at(raw_position, 1.0 / scale);
    if parameters._padding.y == 1u { return vec4<f32>(linear_rgb, 1.0); }
    if parameters._padding.y == 2u {
        return vec4<f32>(clamp(linear_rgb, vec3<f32>(-65504.0), vec3<f32>(65504.0)), 1.0);
    }
    return vec4<f32>(aces_tone_map(linear_rgb), 1.0);
}
