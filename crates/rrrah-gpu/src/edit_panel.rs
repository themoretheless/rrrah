//! Bounded native editor overlay; shared physical-pixel layout for rendering and hit testing.
use crate::{HudRenderer, HudVertex, push_hud_text, push_quad};
use rrrah_core::{edit::EditState, histogram::Histogram};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditPanel {
    Conflict,
    Curve,
    Histogram,
}
/// Graph square is shared with pointer input, independent of image zoom/pan.
pub const GRAPH: [f32; 4] = [24., 56., 256., 256.];
pub fn panel_contains(position: [f32; 2]) -> bool {
    position[0] >= 12. && position[0] <= 304. && position[1] >= 12. && position[1] <= 374.
}
pub fn graph_point(position: [f32; 2]) -> Option<[f64; 2]> {
    let [left, top, width, height] = GRAPH;
    (position[0] >= left && position[0] <= left + width && position[1] >= top && position[1] <= top + height)
        .then(|| {
            [
                f64::from((position[0] - left) / width),
                f64::from(1. - (position[1] - top) / height),
            ]
        })
}
pub fn conflict_choice(position: [f32; 2]) -> Option<bool> {
    if !(88. ..=126.).contains(&position[1]) {
        return None;
    }
    if (24. ..=146.).contains(&position[0]) {
        Some(true)
    } else if (158. ..=292.).contains(&position[0]) {
        Some(false)
    } else {
        None
    }
}
fn line(vertices: &mut Vec<HudVertex>, start: [f32; 2], end: [f32; 2], color: [f32; 4]) {
    let delta = [end[0] - start[0], end[1] - start[1]];
    let len = delta[0].hypot(delta[1]);
    if len < 0.001 {
        return;
    }
    let normal = [-delta[1] / len, delta[0] / len];
    let position = [
        [start[0] + normal[0], start[1] + normal[1]],
        [start[0] - normal[0], start[1] - normal[1]],
        [end[0] + normal[0], end[1] + normal[1]],
        [end[0] - normal[0], end[1] - normal[1]],
    ];
    for i in [0, 1, 2, 2, 1, 3] {
        vertices.push(HudVertex {
            position: position[i],
            color,
        });
    }
}
fn vertices(panel: EditPanel, state: &EditState, histogram: Option<&Histogram>) -> Vec<HudVertex> {
    let mut vertices = Vec::new();
    let white = [0.92, 0.95, 1., 1.];
    let blue = [0.15, 0.6, 1., 1.];
    push_quad(
        &mut vertices,
        12.,
        12.,
        292.,
        if panel == EditPanel::Conflict { 134. } else { 362. },
        // A translucent backing leaks scene-linear HDR into controls: at the
        // half-float ceiling even 2% makes the background brighter than text.
        [0.006, 0.009, 0.016, 1.0],
    );
    let title = match panel {
        EditPanel::Conflict => "CHOOSE EDIT VERSION",
        EditPanel::Curve => "TONE CURVE - T TO CLOSE",
        EditPanel::Histogram => "SOURCE LINEAR - H TO CLOSE",
    };
    push_hud_text(&mut vertices, title, 24., 24., 1.3, white);
    if panel == EditPanel::Conflict {
        push_hud_text(&mut vertices, "CHOOSE BEFORE SAVING", 24., 52., 1.2, white);
        for (left, width, label) in [(24., 122., "LOCAL EDITS"), (158., 134., "SAVED VERSION")] {
            push_quad(&mut vertices, left, 88., width, 38., [0.04, 0.12, 0.23, 1.]);
            push_hud_text(&mut vertices, label, left + 8., 102., 1.2, white);
        }
        return vertices;
    }
    let [left, top, width, height] = GRAPH;
    for i in 0..=4 {
        let fraction = i as f32 / 4.;
        push_quad(
            &mut vertices,
            left + width * fraction,
            top,
            1.,
            height,
            [0.07, 0.09, 0.13, 1.],
        );
        push_quad(
            &mut vertices,
            left,
            top + height * fraction,
            width,
            1.,
            [0.07, 0.09, 0.13, 1.],
        );
    }
    if panel == EditPanel::Curve {
        let points: Vec<_> = state
            .points()
            .iter()
            .map(|position| (position[0], position[1]))
            .collect();
        if let Ok(curve) = rrrah_core::develop::MonotoneCurve::new(&points) {
            let mut previous = [left, top + height - curve.eval(0.) as f32 * height];
            for i in 1..=128 {
                let fraction = i as f32 / 128.;
                let next = [
                    left + fraction * width,
                    top + height - curve.eval(f64::from(fraction)) as f32 * height,
                ];
                line(&mut vertices, previous, next, blue);
                previous = next;
            }
        }
        for position in state.points() {
            push_quad(
                &mut vertices,
                left + position[0] as f32 * width - 3.,
                top + (1. - position[1] as f32) * height - 3.,
                6.,
                6.,
                white,
            );
        }
        push_hud_text(
            &mut vertices,
            "CLICK TO ADD - DRAG TO EDIT",
            24.,
            326.,
            1.2,
            white,
        );
        push_hud_text(&mut vertices, "RESET CURVE", 24., 350., 1.2, blue);
    } else if let Some(hist) = histogram {
        let colors = [
            [0.95, 0.09, 0.09, 0.65],
            [0.1, 0.85, 0.15, 0.65],
            [0.15, 0.35, 1., 0.65],
            [0.95, 0.95, 0.95, 0.8],
        ];
        let max = hist.channels.iter().flatten().copied().max().unwrap_or(0).max(1) as f32;
        for (bins, color) in hist.channels.iter().zip(colors) {
            for (i, n) in bins.iter().enumerate() {
                let bar_height = *n as f32 / max * height;
                push_quad(
                    &mut vertices,
                    left + i as f32 * width / 64.,
                    top + height - bar_height,
                    2.,
                    bar_height,
                    color,
                );
            }
        }
        push_hud_text(&mut vertices, "-16 EV      0      +16 EV", 24., 326., 1.2, white);
        push_hud_text(
            &mut vertices,
            &format!("SAMPLES {}  INVALID {}", hist.sampled, hist.invalid),
            24.,
            348.,
            1.1,
            white,
        );
    } else {
        push_hud_text(&mut vertices, "LINEAR SOURCE UNAVAILABLE", 24., 190., 1.2, white);
    }
    assert!(vertices.len() <= 16_384, "bounded panel geometry");
    vertices
}
impl HudRenderer {
    /// At most 16384 vertices (384 KiB CPU, <=512 KiB GPU capacity).
    pub fn update_edit_panel(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        panel: EditPanel,
        state: &EditState,
        histogram: Option<&Histogram>,
    ) {
        let vertices = vertices(panel, state, histogram);
        let bytes = bytemuck::cast_slice::<HudVertex, u8>(&vertices);
        if bytes.len() as u64 > self.vertex_capacity {
            self.vertex_capacity = (bytes.len() as u64).next_power_of_two();
            self.vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Rrrah editor panel"),
                size: self.vertex_capacity,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
        queue.write_buffer(&self.vertex_buffer, 0, bytes);
        self.vertex_count = vertices.len() as u32;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn histogram_bars_grow_upward_from_the_graph_baseline() {
        let mut histogram = Histogram::default();
        histogram.channels[0][32] = 50;
        histogram.channels[1][32] = 100;
        let mesh = vertices(EditPanel::Histogram, &EditState::default(), Some(&histogram));
        let red: Vec<_> = mesh
            .iter()
            .filter(|v| v.color == [0.95, 0.09, 0.09, 0.65])
            .collect();
        let bar = &red[32 * 6..33 * 6];
        let top = bar.iter().map(|v| v.position[1]).fold(f32::INFINITY, f32::min);
        let bottom = bar
            .iter()
            .map(|v| v.position[1])
            .fold(f32::NEG_INFINITY, f32::max);
        assert_eq!(top, 184.);
        assert_eq!(bottom, 312.);
    }
    #[test]
    fn coordinates_and_conflict_buttons_match_rendered_layout() {
        assert_eq!(graph_point([24., 56.]), Some([0., 1.]));
        assert_eq!(graph_point([280., 312.]), Some([1., 0.]));
        assert!(graph_point([23., 56.]).is_none());
        assert_eq!(conflict_choice([30., 100.]), Some(true));
        assert_eq!(conflict_choice([160., 100.]), Some(false));
        assert_eq!(conflict_choice([150., 100.]), None);
        for panel in [EditPanel::Conflict, EditPanel::Curve, EditPanel::Histogram] {
            let vertices = vertices(panel, &EditState::default(), Some(&Histogram::default()));
            assert!(!vertices.is_empty());
            assert!(vertices.len() <= 16_384);
            assert!(
                vertices
                    .iter()
                    .flat_map(|vertices| vertices.position)
                    .all(f32::is_finite)
            );
        }
    }
}
