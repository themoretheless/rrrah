// Bayer 2x2 GPU compute demosaicing (Bilinear 3x3, Malvar-He-Cutler 5x5, Directional 5x5).
// Input: normalized and white-balanced sensor samples (`f32`).
// Output: camera-space or linear-RGB `vec4<f32>` (`alpha = 1.0`). No SDR tone mapping or [0, 1] clipping.
struct Parameters {
    dimensions: vec2<u32>,
    row_range: vec2<u32>,
    cfa: vec4<u32>,
    output_window: vec4<u32>, // .x = output_first_pixel, .y = source_first_sample, .z = algorithm
};

@group(0) @binding(0) var<storage, read> source: array<f32>;
@group(0) @binding(1) var<storage, read_write> output: array<vec4<f32>>;
@group(0) @binding(2) var<uniform> parameters: Parameters;

fn reflect_coord(i: i32, n: i32) -> u32 {
    if n <= 1 { return 0u; }
    let period = 2 * (n - 1);
    var j = i % period;
    if j < 0 { j += period; }
    if j >= n { return u32(period - j); }
    return u32(j);
}

fn sample_at(x: i32, y: i32) -> f32 {
    let rx = reflect_coord(x, i32(parameters.dimensions.x));
    let ry = reflect_coord(y, i32(parameters.dimensions.y));
    return source[ry * parameters.dimensions.x + rx - parameters.output_window.y];
}

fn cfa_at(x: u32, y: u32) -> u32 {
    let phase = (y & 1u) * 2u + (x & 1u);
    return parameters.cfa[phase];
}

fn bilinear_at(x: i32, y: i32) -> vec3<f32> {
    let c = sample_at(x, y);
    let n = sample_at(x, y - 1);
    let s = sample_at(x, y + 1);
    let w = sample_at(x - 1, y);
    let e = sample_at(x + 1, y);
    let nw = sample_at(x - 1, y - 1);
    let ne = sample_at(x + 1, y - 1);
    let sw = sample_at(x - 1, y + 1);
    let se = sample_at(x + 1, y + 1);
    let axial = 0.25 * (n + s + w + e);
    let diag = 0.25 * (nw + ne + sw + se);
    let own = cfa_at(u32(x), u32(y));
    if own == 0u { return vec3<f32>(c, axial, diag); }
    if own == 2u { return vec3<f32>(diag, axial, c); }
    let h_col = cfa_at(u32(x) + 1u, u32(y));
    if h_col == 0u { return vec3<f32>(0.5 * (w + e), c, 0.5 * (n + s)); }
    return vec3<f32>(0.5 * (n + s), c, 0.5 * (w + e));
}

fn mhc_at(x: i32, y: i32) -> vec3<f32> {
    let c = sample_at(x, y);
    let n1 = sample_at(x, y - 1);
    let s1 = sample_at(x, y + 1);
    let w1 = sample_at(x - 1, y);
    let e1 = sample_at(x + 1, y);
    let n2 = sample_at(x, y - 2);
    let s2 = sample_at(x, y + 2);
    let w2 = sample_at(x - 2, y);
    let e2 = sample_at(x + 2, y);
    let nw = sample_at(x - 1, y - 1);
    let ne = sample_at(x + 1, y - 1);
    let sw = sample_at(x - 1, y + 1);
    let se = sample_at(x + 1, y + 1);

    let own = cfa_at(u32(x), u32(y));
    if own == 0u || own == 2u {
        let g_min = min(min(n1, s1), min(w1, e1));
        let g_max = max(max(n1, s1), max(w1, e1));
        let g_est = 0.25 * (n1 + s1 + w1 + e1) + 0.125 * (4.0 * c - (n2 + s2 + w2 + e2));
        let g = clamp(g_est, g_min, g_max);

        let opp_min = min(min(nw, ne), min(sw, se));
        let opp_max = max(max(nw, ne), max(sw, se));
        let opp_est = 0.25 * (nw + ne + sw + se) + 0.1875 * (4.0 * c - (n2 + s2 + w2 + e2));
        let opp = clamp(opp_est, opp_min, opp_max);

        if own == 0u { return vec3<f32>(c, g, opp); }
        return vec3<f32>(opp, g, c);
    }

    let h_est = 0.5 * (w1 + e1) + 0.125 * (5.0 * c - (nw + ne + sw + se) - (w2 + e2) + 0.5 * (n2 + s2));
    let ch = clamp(h_est, min(w1, e1), max(w1, e1));
    let v_est = 0.5 * (n1 + s1) + 0.125 * (5.0 * c - (nw + ne + sw + se) - (n2 + s2) + 0.5 * (w2 + e2));
    let cv = clamp(v_est, min(n1, s1), max(n1, s1));

    let h_col = cfa_at(u32(x) + 1u, u32(y));
    if h_col == 0u { return vec3<f32>(ch, c, cv); }
    return vec3<f32>(cv, c, ch);
}

fn green_at_rb(x: i32, y: i32, c: f32) -> f32 {
    let n1 = sample_at(x, y - 1);
    let s1 = sample_at(x, y + 1);
    let w1 = sample_at(x - 1, y);
    let e1 = sample_at(x + 1, y);
    let n2 = sample_at(x, y - 2);
    let s2 = sample_at(x, y + 2);
    let w2 = sample_at(x - 2, y);
    let e2 = sample_at(x + 2, y);
    let gh = clamp(0.5 * (w1 + e1) + 0.25 * (2.0 * c - w2 - e2), min(w1, e1), max(w1, e1));
    let gv = clamp(0.5 * (n1 + s1) + 0.25 * (2.0 * c - n2 - s2), min(n1, s1), max(n1, s1));
    let grad_h = abs(w1 - e1) + abs(2.0 * c - w2 - e2);
    let grad_v = abs(n1 - s1) + abs(2.0 * c - n2 - s2);
    let wh = 1.0 / (0.00001 + grad_h);
    let wv = 1.0 / (0.00001 + grad_v);
    return (wh * gh + wv * gv) / (wh + wv);
}

fn directional_at(x: i32, y: i32) -> vec3<f32> {
    let c = sample_at(x, y);
    let own = cfa_at(u32(x), u32(y));
    if own == 0u || own == 2u {
        let gp = green_at_rb(x, y, c);
        let nw = sample_at(x - 1, y - 1);
        let ne = sample_at(x + 1, y - 1);
        let sw = sample_at(x - 1, y + 1);
        let se = sample_at(x + 1, y + 1);
        let d_nw = nw - 0.5 * (sample_at(x, y - 1) + sample_at(x - 1, y));
        let d_se = se - 0.5 * (sample_at(x, y + 1) + sample_at(x + 1, y));
        let d_ne = ne - 0.5 * (sample_at(x, y - 1) + sample_at(x + 1, y));
        let d_sw = sw - 0.5 * (sample_at(x, y + 1) + sample_at(x - 1, y));
        let w1 = 1.0 / (0.00001 + abs(nw - se) + abs(d_nw - d_se));
        let w2 = 1.0 / (0.00001 + abs(ne - sw) + abs(d_ne - d_sw));
        let opp_min = min(min(nw, ne), min(sw, se));
        let opp_max = max(max(nw, ne), max(sw, se));
        let opp = clamp(gp + (w1 * 0.5 * (d_nw + d_se) + w2 * 0.5 * (d_ne + d_sw)) / (w1 + w2), opp_min, opp_max);
        if own == 0u { return vec3<f32>(c, gp, opp); }
        return vec3<f32>(opp, gp, c);
    }

    let w1 = sample_at(x - 1, y);
    let e1 = sample_at(x + 1, y);
    let n1 = sample_at(x, y - 1);
    let s1 = sample_at(x, y + 1);
    let gw = green_at_rb(x - 1, y, w1);
    let ge = green_at_rb(x + 1, y, e1);
    let gn = green_at_rb(x, y - 1, n1);
    let gs = green_at_rb(x, y + 1, s1);
    let ch = clamp(c + 0.5 * ((w1 - gw) + (e1 - ge)), min(w1, e1), max(w1, e1));
    let cv = clamp(c + 0.5 * ((n1 - gn) + (s1 - gs)), min(n1, s1), max(n1, s1));
    let h_col = cfa_at(u32(x) + 1u, u32(y));
    if h_col == 0u { return vec3<f32>(ch, c, cv); }
    return vec3<f32>(cv, c, ch);
}

fn smooth_hue_at(x: i32, y: i32) -> vec3<f32> {
    let eps = 0.001;
    let c = sample_at(x, y);
    let own = cfa_at(u32(x), u32(y));
    let n1 = sample_at(x, y - 1);
    let s1 = sample_at(x, y + 1);
    let w1 = sample_at(x - 1, y);
    let e1 = sample_at(x + 1, y);
    if own == 0u || own == 2u {
        let gp = 0.25 * (n1 + s1 + w1 + e1);
        let nw = sample_at(x - 1, y - 1);
        let ne = sample_at(x + 1, y - 1);
        let sw = sample_at(x - 1, y + 1);
        let se = sample_at(x + 1, y + 1);
        let gnw = 0.25 * (sample_at(x - 1, y - 2) + sample_at(x - 1, y) + sample_at(x - 2, y - 1) + sample_at(x, y - 1));
        let gne = 0.25 * (sample_at(x + 1, y - 2) + sample_at(x + 1, y) + sample_at(x, y - 1) + sample_at(x + 2, y - 1));
        let gsw = 0.25 * (sample_at(x - 1, y) + sample_at(x - 1, y + 2) + sample_at(x - 2, y + 1) + sample_at(x, y + 1));
        let gse = 0.25 * (sample_at(x + 1, y) + sample_at(x + 1, y + 2) + sample_at(x, y + 1) + sample_at(x + 2, y + 1));
        let r_avg = 0.25 * (
            (nw + eps) / (gnw + eps) +
            (ne + eps) / (gne + eps) +
            (sw + eps) / (gsw + eps) +
            (se + eps) / (gse + eps)
        );
        let opp_min = min(min(nw, ne), min(sw, se));
        let opp_max = max(max(nw, ne), max(sw, se));
        let opp = clamp((gp + eps) * r_avg - eps, opp_min, opp_max);
        if own == 0u { return vec3<f32>(c, gp, opp); }
        return vec3<f32>(opp, gp, c);
    }
    let gw = 0.25 * (sample_at(x - 1, y - 1) + sample_at(x - 1, y + 1) + sample_at(x - 2, y) + c);
    let ge = 0.25 * (sample_at(x + 1, y - 1) + sample_at(x + 1, y + 1) + c + sample_at(x + 2, y));
    let gn = 0.25 * (sample_at(x, y - 2) + c + sample_at(x - 1, y - 1) + sample_at(x + 1, y - 1));
    let gs = 0.25 * (c + sample_at(x, y + 2) + sample_at(x - 1, y + 1) + sample_at(x + 1, y + 1));
    let rh = 0.5 * ((w1 + eps) / (gw + eps) + (e1 + eps) / (ge + eps));
    let rv = 0.5 * ((n1 + eps) / (gn + eps) + (s1 + eps) / (gs + eps));
    let ch = clamp((c + eps) * rh - eps, min(w1, e1), max(w1, e1));
    let cv = clamp((c + eps) * rv - eps, min(n1, s1), max(n1, s1));
    let h_col = cfa_at(u32(x) + 1u, u32(y));
    if h_col == 0u { return vec3<f32>(ch, c, cv); }
    return vec3<f32>(cv, c, ch);
}

fn vng_at(x: i32, y: i32) -> vec3<f32> {
    let c = sample_at(x, y);
    let n1 = sample_at(x, y - 1);
    let s1 = sample_at(x, y + 1);
    let w1 = sample_at(x - 1, y);
    let e1 = sample_at(x + 1, y);
    let nw = sample_at(x - 1, y - 1);
    let ne = sample_at(x + 1, y - 1);
    let sw = sample_at(x - 1, y + 1);
    let se = sample_at(x + 1, y + 1);
    let n2 = sample_at(x, y - 2);
    let s2 = sample_at(x, y + 2);
    let w2 = sample_at(x - 2, y);
    let e2 = sample_at(x + 2, y);
    let gn = abs(n1 - s1) + abs(n2 - c) + 0.5 * (abs(nw - sw) + abs(ne - se));
    let ge = abs(e1 - w1) + abs(e2 - c) + 0.5 * (abs(ne - nw) + abs(se - sw));
    let gs = abs(s1 - n1) + abs(s2 - c) + 0.5 * (abs(sw - nw) + abs(se - ne));
    let gw = abs(w1 - e1) + abs(w2 - c) + 0.5 * (abs(nw - ne) + abs(sw - se));
    let g_min = min(min(gn, ge), min(gs, gw));
    let g_max = max(max(gn, ge), max(gs, gw));
    let thresh = 1.5 * g_min + 0.5 * (g_max - g_min) + 0.00001;

    let own = cfa_at(u32(x), u32(y));
    if own == 0u || own == 2u {
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
        if own == 0u { return vec3<f32>(c, gp, opp); }
        return vec3<f32>(opp, gp, c);
    }
    return directional_at(x, y);
}

fn rcd_green_at_rb(x: i32, y: i32, c: f32) -> f32 {
    let eps = 0.00001;
    let n1 = sample_at(x, y - 1);
    let s1 = sample_at(x, y + 1);
    let w1 = sample_at(x - 1, y);
    let e1 = sample_at(x + 1, y);
    let n2 = sample_at(x, y - 2);
    let s2 = sample_at(x, y + 2);
    let w2 = sample_at(x - 2, y);
    let e2 = sample_at(x + 2, y);
    let n3 = sample_at(x, y - 3);
    let s3 = sample_at(x, y + 3);
    let w3 = sample_at(x - 3, y);
    let e3 = sample_at(x + 3, y);
    let v_stat = abs(n1 - s1) + abs(c - n2) + abs(c - s2) + 0.5 * (abs(n1 - n3) + abs(s1 - s3));
    let h_stat = abs(w1 - e1) + abs(c - w2) + abs(c - e2) + 0.5 * (abs(w1 - w3) + abs(e1 - e3));
    let wv = 1.0 / ((eps + v_stat) * (eps + v_stat));
    let wh = 1.0 / ((eps + h_stat) * (eps + h_stat));
    let gv = clamp(0.5 * (n1 + s1) + 0.25 * (2.0 * c - n2 - s2), min(n1, s1), max(n1, s1));
    let gh = clamp(0.5 * (w1 + e1) + 0.25 * (2.0 * c - w2 - e2), min(w1, e1), max(w1, e1));
    return (wh * gh + wv * gv) / (wh + wv);
}

fn rcd_at(x: i32, y: i32) -> vec3<f32> {
    let c = sample_at(x, y);
    let own = cfa_at(u32(x), u32(y));
    if own == 0u || own == 2u {
        let gp = rcd_green_at_rb(x, y, c);
        let nw = sample_at(x - 1, y - 1);
        let ne = sample_at(x + 1, y - 1);
        let sw = sample_at(x - 1, y + 1);
        let se = sample_at(x + 1, y + 1);
        let gnw = rcd_green_at_rb(x - 1, y - 1, nw);
        let gne = rcd_green_at_rb(x + 1, y - 1, ne);
        let gsw = rcd_green_at_rb(x - 1, y + 1, sw);
        let gse = rcd_green_at_rb(x + 1, y + 1, se);
        let d_nw = nw - gnw;
        let d_se = se - gse;
        let d_ne = ne - gne;
        let d_sw = sw - gsw;
        let w1 = 1.0 / (0.00001 + abs(nw - se) + abs(d_nw - d_se));
        let w2 = 1.0 / (0.00001 + abs(ne - sw) + abs(d_ne - d_sw));
        let opp_est = gp + (w1 * 0.5 * (d_nw + d_se) + w2 * 0.5 * (d_ne + d_sw)) / (w1 + w2);
        let opp = clamp(opp_est, min(min(nw, ne), min(sw, se)), max(max(nw, ne), max(sw, se)));
        if own == 0u { return vec3<f32>(c, gp, opp); }
        return vec3<f32>(opp, gp, c);
    }
    let w1 = sample_at(x - 1, y);
    let e1 = sample_at(x + 1, y);
    let n1 = sample_at(x, y - 1);
    let s1 = sample_at(x, y + 1);
    let gw = rcd_green_at_rb(x - 1, y, w1);
    let ge = rcd_green_at_rb(x + 1, y, e1);
    let gn = rcd_green_at_rb(x, y - 1, n1);
    let gs = rcd_green_at_rb(x, y + 1, s1);
    let ch = clamp(c + 0.5 * ((w1 - gw) + (e1 - ge)), min(w1, e1), max(w1, e1));
    let cv = clamp(c + 0.5 * ((n1 - gn) + (s1 - gs)), min(n1, s1), max(n1, s1));
    let h_col = cfa_at(u32(x) + 1u, u32(y));
    if h_col == 0u { return vec3<f32>(ch, c, cv); }
    return vec3<f32>(cv, c, ch);
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    if id.x >= parameters.dimensions.x || id.y >= parameters.row_range.y {
        return;
    }
    let sensor_y = id.y + parameters.row_range.x;
    let x = i32(id.x);
    let y = i32(sensor_y);
    var rgb: vec3<f32>;
    let alg = parameters.output_window.z;
    if alg == 1u {
        rgb = mhc_at(x, y);
    } else if alg == 2u {
        rgb = directional_at(x, y);
    } else if alg == 3u {
        rgb = smooth_hue_at(x, y);
    } else if alg == 4u {
        rgb = vng_at(x, y);
    } else if alg == 5u {
        rgb = rcd_at(x, y);
    } else {
        rgb = bilinear_at(x, y);
    }
    output[sensor_y * parameters.dimensions.x + id.x - parameters.output_window.x] = vec4<f32>(rgb, 1.0);
}
