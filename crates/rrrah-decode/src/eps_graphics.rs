//! Native, budgeted EPS vector state. Paths use immutable persistent nodes;
//! rendering, clipping, text and image objects are not implemented here.
use rrrah_core::{BufferError, MemoryBudget, MutableBuffer, SharedBuffer};

pub type EpsMatrix = [f64; 6];
pub type EpsPoint = [f64; 2];
const IDENTITY: EpsMatrix = [1., 0., 0., 1., 0., 0.];
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EpsDeviceColor {
    Gray(f64),
    Rgb([f64; 3]),
    Cmyk([f64; 4]),
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EpsStrokeStyle {
    pub color: EpsDeviceColor,
    pub width: f64,
    pub cap: u8,
    pub join: u8,
    pub miter_limit: f64,
}
impl Default for EpsStrokeStyle {
    fn default() -> Self {
        Self {
            color: EpsDeviceColor::Gray(0.),
            width: 1.,
            cap: 0,
            join: 0,
            miter_limit: 10.,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EpsPathSegment {
    Move(EpsPoint),
    Line {
        start: EpsPoint,
        end: EpsPoint,
    },
    Curve {
        start: EpsPoint,
        control1: EpsPoint,
        control2: EpsPoint,
        end: EpsPoint,
    },
    Close {
        start: EpsPoint,
        end: EpsPoint,
    },
}
#[derive(Debug, Clone, Copy)]
struct Node {
    segment: EpsPathSegment,
    previous: Option<usize>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EpsPaintKind {
    Stroke,
    FillNonZero,
    FillEvenOdd,
}
#[derive(Debug, Clone, Copy)]
pub struct EpsPaint {
    pub kind: EpsPaintKind,
    /// Final CTM must be retained to define the pen under nonuniform transforms.
    pub matrix: EpsMatrix,
    pub style: EpsStrokeStyle,
    pub node_count: usize,
    head: Option<usize>,
}
#[derive(Debug, Clone, Copy)]
pub struct EpsGraphicsLimits {
    /// Total immutable arena nodes, including saved/discarded path branches.
    pub max_nodes: usize,
    pub max_paints: usize,
    pub max_saved_states: usize,
}
impl Default for EpsGraphicsLimits {
    fn default() -> Self {
        Self {
            max_nodes: 65536,
            max_paints: 4096,
            max_saved_states: 256,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EpsGraphicsLimit {
    Nodes,
    Paints,
    SavedStates,
}
#[derive(Debug, thiserror::Error)]
pub enum EpsGraphicsError {
    #[error("PostScript graphics has no current point/path")]
    NoCurrentPoint,
    #[error("PostScript graphics matrix cannot be inverted")]
    InvalidMatrix,
    #[error("PostScript graphics value is outside the supported range")]
    Range,
    #[error("PostScript graphics limit: {0:?}")]
    Limit(EpsGraphicsLimit),
    #[error("PostScript graphics query cancelled")]
    Cancelled,
    #[error("PostScript path destination is too short")]
    OutputSize,
    #[error(transparent)]
    Memory(#[from] BufferError),
}
#[derive(Debug, Clone, Copy)]
struct State {
    matrix: EpsMatrix,
    style: EpsStrokeStyle,
    head: Option<usize>,
    length: usize,
    point: Option<EpsPoint>,
    subpath: Option<EpsPoint>,
    closed: bool,
}
impl Default for State {
    fn default() -> Self {
        Self {
            matrix: IDENTITY,
            style: EpsStrokeStyle::default(),
            head: None,
            length: 0,
            point: None,
            subpath: None,
            closed: false,
        }
    }
}
#[derive(Debug)]
pub struct EpsGraphics {
    nodes: MutableBuffer<Node>,
    used_nodes: usize,
    paints: MutableBuffer<EpsPaint>,
    used_paints: usize,
    saved: MutableBuffer<State>,
    depth: usize,
    state: State,
}
#[derive(Debug, Clone)]
pub struct EpsVectorScene {
    nodes: SharedBuffer<Node>,
    paints: SharedBuffer<EpsPaint>,
    length: usize,
}
fn finite<const N: usize>(values: [f64; N]) -> Result<[f64; N], EpsGraphicsError> {
    if values.iter().all(|n| n.is_finite()) {
        Ok(values)
    } else {
        Err(EpsGraphicsError::Range)
    }
}
fn point(m: EpsMatrix, p: EpsPoint) -> Result<EpsPoint, EpsGraphicsError> {
    finite([m[0] * p[0] + m[2] * p[1] + m[4], m[1] * p[0] + m[3] * p[1] + m[5]])
}
pub(crate) fn inverse(m: EpsMatrix) -> Result<EpsMatrix, EpsGraphicsError> {
    let det = m[0] * m[3] - m[1] * m[2];
    if det == 0. || !det.is_finite() {
        return Err(EpsGraphicsError::InvalidMatrix);
    }
    finite([
        m[3] / det,
        -m[1] / det,
        -m[2] / det,
        m[0] / det,
        (m[2] * m[5] - m[3] * m[4]) / det,
        (m[1] * m[4] - m[0] * m[5]) / det,
    ])
    .map_err(|_| EpsGraphicsError::InvalidMatrix)
}
impl EpsGraphics {
    /// All arena, paint and save capacities are admitted to the shared budget
    /// before use. Source/code memory belongs to the caller. Initial matrix is
    /// canonical EPS user space, independent of eventual output page/device DPI.
    pub fn new<F: FnMut() -> bool>(
        limits: EpsGraphicsLimits,
        budget: &MemoryBudget,
        mut cancelled: F,
    ) -> Result<Self, EpsGraphicsError> {
        if cancelled() {
            return Err(EpsGraphicsError::Cancelled);
        }
        let nodes = budget.try_buffer(
            limits.max_nodes,
            Node {
                segment: EpsPathSegment::Move([0., 0.]),
                previous: None,
            },
        )?;
        if cancelled() {
            return Err(EpsGraphicsError::Cancelled);
        }
        let state = State::default();
        let paints = budget.try_buffer(
            limits.max_paints,
            EpsPaint {
                kind: EpsPaintKind::Stroke,
                matrix: IDENTITY,
                style: state.style,
                node_count: 0,
                head: None,
            },
        )?;
        if cancelled() {
            return Err(EpsGraphicsError::Cancelled);
        }
        let saved = budget.try_buffer(limits.max_saved_states, state)?;
        if cancelled() {
            return Err(EpsGraphicsError::Cancelled);
        }
        Ok(Self {
            nodes,
            used_nodes: 0,
            paints,
            used_paints: 0,
            saved,
            depth: 0,
            state,
        })
    }
    pub fn matrix(&self) -> EpsMatrix {
        self.state.matrix
    }
    pub fn style(&self) -> EpsStrokeStyle {
        self.state.style
    }
    pub fn node_count(&self) -> usize {
        self.state.length
    }
    pub fn arena_nodes_used(&self) -> usize {
        self.used_nodes
    }
    pub fn current_point(&self) -> Result<EpsPoint, EpsGraphicsError> {
        let p = self.state.point.ok_or(EpsGraphicsError::NoCurrentPoint)?;
        point(inverse(self.state.matrix)?, p)
    }
    pub fn concat(&mut self, n: EpsMatrix) -> Result<(), EpsGraphicsError> {
        let n = finite(n)?;
        let m = self.state.matrix;
        self.state.matrix = finite([
            m[0] * n[0] + m[2] * n[1],
            m[1] * n[0] + m[3] * n[1],
            m[0] * n[2] + m[2] * n[3],
            m[1] * n[2] + m[3] * n[3],
            m[0] * n[4] + m[2] * n[5] + m[4],
            m[1] * n[4] + m[3] * n[5] + m[5],
        ])?;
        Ok(())
    }
    pub fn translate(&mut self, x: f64, y: f64) -> Result<(), EpsGraphicsError> {
        self.concat([1., 0., 0., 1., x, y])
    }
    pub fn scale(&mut self, x: f64, y: f64) -> Result<(), EpsGraphicsError> {
        self.concat([x, 0., 0., y, 0., 0.])
    }
    pub fn rotate(&mut self, degrees: f64) -> Result<(), EpsGraphicsError> {
        finite([degrees])?;
        let (s, c) = degrees.to_radians().sin_cos();
        self.concat([c, s, -s, c, 0., 0.])
    }
    pub fn gsave(&mut self) -> Result<(), EpsGraphicsError> {
        if self.depth == self.saved.len() {
            return Err(EpsGraphicsError::Limit(EpsGraphicsLimit::SavedStates));
        }
        self.saved[self.depth] = self.state;
        self.depth += 1;
        Ok(())
    }
    pub fn grestore(&mut self) -> Result<(), EpsGraphicsError> {
        // No VM save anchors exist in this standalone state; an empty graphics
        // stack has no effect, as for an unencapsulated PostScript job.
        let Some(depth) = self.depth.checked_sub(1) else {
            return Ok(());
        };
        self.state = self.saved[depth];
        self.depth = depth;
        Ok(())
    }
    pub fn new_path(&mut self) {
        self.state.head = None;
        self.state.length = 0;
        self.state.point = None;
        self.state.subpath = None;
        self.state.closed = false;
    }
    fn admit_nodes(&self, n: usize) -> Result<(), EpsGraphicsError> {
        if self
            .used_nodes
            .checked_add(n)
            .is_none_or(|end| end > self.nodes.len())
        {
            Err(EpsGraphicsError::Limit(EpsGraphicsLimit::Nodes))
        } else {
            Ok(())
        }
    }
    fn append(&mut self, segment: EpsPathSegment) {
        self.nodes[self.used_nodes] = Node {
            segment,
            previous: self.state.head,
        };
        self.state.head = Some(self.used_nodes);
        self.used_nodes += 1;
        self.state.length += 1;
    }
    fn move_device(&mut self, p: EpsPoint) -> Result<(), EpsGraphicsError> {
        self.admit_nodes(1)?;
        // Replace a trailing moveto while preserving its immutable saved branch.
        if let Some(head) = self.state.head {
            if matches!(self.nodes[head].segment, EpsPathSegment::Move(_)) {
                self.state.head = self.nodes[head].previous;
                self.state.length -= 1;
            }
        }
        self.append(EpsPathSegment::Move(p));
        self.state.point = Some(p);
        self.state.subpath = Some(p);
        self.state.closed = false;
        Ok(())
    }
    pub fn move_to(&mut self, x: f64, y: f64) -> Result<(), EpsGraphicsError> {
        let p = point(self.state.matrix, finite([x, y])?)?;
        self.move_device(p)
    }
    fn relative(&self, delta: EpsPoint) -> Result<EpsPoint, EpsGraphicsError> {
        let p = self.state.point.ok_or(EpsGraphicsError::NoCurrentPoint)?;
        let d = finite(delta)?;
        let m = self.state.matrix;
        finite([p[0] + m[0] * d[0] + m[2] * d[1], p[1] + m[1] * d[0] + m[3] * d[1]])
    }
    pub fn relative_move_to(&mut self, x: f64, y: f64) -> Result<(), EpsGraphicsError> {
        let p = self.relative([x, y])?;
        self.move_device(p)
    }
    fn resume(&mut self, p: EpsPoint) {
        if self.state.closed {
            self.append(EpsPathSegment::Move(p));
            self.state.subpath = Some(p);
            self.state.closed = false;
        }
    }
    fn line_device(&mut self, end: EpsPoint) -> Result<(), EpsGraphicsError> {
        let start = self.state.point.ok_or(EpsGraphicsError::NoCurrentPoint)?;
        self.admit_nodes(1 + usize::from(self.state.closed))?;
        self.resume(start);
        self.append(EpsPathSegment::Line { start, end });
        self.state.point = Some(end);
        Ok(())
    }
    pub fn line_to(&mut self, x: f64, y: f64) -> Result<(), EpsGraphicsError> {
        let p = point(self.state.matrix, finite([x, y])?)?;
        self.line_device(p)
    }
    pub fn relative_line_to(&mut self, x: f64, y: f64) -> Result<(), EpsGraphicsError> {
        let p = self.relative([x, y])?;
        self.line_device(p)
    }
    fn curve_device(
        &mut self,
        control1: EpsPoint,
        control2: EpsPoint,
        end: EpsPoint,
    ) -> Result<(), EpsGraphicsError> {
        let start = self.state.point.ok_or(EpsGraphicsError::NoCurrentPoint)?;
        self.admit_nodes(1 + usize::from(self.state.closed))?;
        self.resume(start);
        self.append(EpsPathSegment::Curve {
            start,
            control1,
            control2,
            end,
        });
        self.state.point = Some(end);
        Ok(())
    }
    pub fn curve_to(&mut self, coords: [f64; 6]) -> Result<(), EpsGraphicsError> {
        let p = finite(coords)?;
        let m = self.state.matrix;
        self.curve_device(
            point(m, [p[0], p[1]])?,
            point(m, [p[2], p[3]])?,
            point(m, [p[4], p[5]])?,
        )
    }
    pub fn relative_curve_to(&mut self, coords: [f64; 6]) -> Result<(), EpsGraphicsError> {
        let p = finite(coords)?;
        self.curve_device(
            self.relative([p[0], p[1]])?,
            self.relative([p[2], p[3]])?,
            self.relative([p[4], p[5]])?,
        )
    }
    pub fn close_path(&mut self) -> Result<(), EpsGraphicsError> {
        if self.state.closed || self.state.point.is_none() {
            return Ok(());
        }
        let start = self.state.point.ok_or(EpsGraphicsError::NoCurrentPoint)?;
        let end = self.state.subpath.ok_or(EpsGraphicsError::NoCurrentPoint)?;
        self.admit_nodes(1)?;
        self.append(EpsPathSegment::Close { start, end });
        self.state.point = Some(end);
        self.state.closed = true;
        Ok(())
    }
    pub fn set_color(&mut self, color: EpsDeviceColor) -> Result<(), EpsGraphicsError> {
        self.state.style.color = match color {
            EpsDeviceColor::Gray(n) => EpsDeviceColor::Gray(finite([n])?[0].clamp(0., 1.)),
            EpsDeviceColor::Rgb(v) => EpsDeviceColor::Rgb(finite(v)?.map(|n| n.clamp(0., 1.))),
            EpsDeviceColor::Cmyk(v) => EpsDeviceColor::Cmyk(finite(v)?.map(|n| n.clamp(0., 1.))),
        };
        Ok(())
    }
    pub fn set_line_width(&mut self, width: f64) -> Result<(), EpsGraphicsError> {
        if !width.is_finite() || width < 0. {
            return Err(EpsGraphicsError::Range);
        }
        self.state.style.width = width;
        Ok(())
    }
    pub fn set_line_cap(&mut self, cap: u8) -> Result<(), EpsGraphicsError> {
        if cap > 2 {
            return Err(EpsGraphicsError::Range);
        }
        self.state.style.cap = cap;
        Ok(())
    }
    pub fn set_line_join(&mut self, join: u8) -> Result<(), EpsGraphicsError> {
        if join > 2 {
            return Err(EpsGraphicsError::Range);
        }
        self.state.style.join = join;
        Ok(())
    }
    pub fn set_miter_limit(&mut self, limit: f64) -> Result<(), EpsGraphicsError> {
        if !limit.is_finite() || limit < 1. {
            return Err(EpsGraphicsError::Range);
        }
        self.state.style.miter_limit = limit;
        Ok(())
    }
    /// Records path and pen metadata without rasterizing or substituting previews.
    /// Stroke/fill consume the active path, while saved persistent branches remain.
    pub fn paint(&mut self, kind: EpsPaintKind) -> Result<(), EpsGraphicsError> {
        if self.state.length != 0 {
            if self.used_paints == self.paints.len() {
                return Err(EpsGraphicsError::Limit(EpsGraphicsLimit::Paints));
            }
            self.paints[self.used_paints] = EpsPaint {
                kind,
                matrix: self.state.matrix,
                style: self.state.style,
                node_count: self.state.length,
                head: self.state.head,
            };
            self.used_paints += 1;
        }
        self.new_path();
        Ok(())
    }
    /// PostScript pathbbox: first bound device-space control points, then inverse
    /// transform all four rectangle corners. A trailing moveto is excluded except
    /// when it is the only element (LanguageLevel 2/3). This is not a tight curve box.
    pub fn path_bbox<F: FnMut() -> bool>(&self, mut cancelled: F) -> Result<[f64; 4], EpsGraphicsError> {
        if cancelled() {
            return Err(EpsGraphicsError::Cancelled);
        }
        let mut bounds = [f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY];
        let mut head = self.state.head.ok_or(EpsGraphicsError::NoCurrentPoint)?;
        let mut first = true;
        loop {
            if cancelled() {
                return Err(EpsGraphicsError::Cancelled);
            }
            let node = self.nodes[head];
            let mut add = |p: EpsPoint| {
                bounds[0] = bounds[0].min(p[0]);
                bounds[1] = bounds[1].min(p[1]);
                bounds[2] = bounds[2].max(p[0]);
                bounds[3] = bounds[3].max(p[1]);
            };
            match node.segment {
                EpsPathSegment::Move(p) => {
                    if !first || self.state.length == 1 {
                        add(p);
                    }
                }
                EpsPathSegment::Line { start, end } | EpsPathSegment::Close { start, end } => {
                    add(start);
                    add(end);
                }
                EpsPathSegment::Curve {
                    start,
                    control1,
                    control2,
                    end,
                } => {
                    add(start);
                    add(control1);
                    add(control2);
                    add(end);
                }
            }
            first = false;
            let Some(previous) = node.previous else { break };
            head = previous;
        }
        let inv = inverse(self.state.matrix)?;
        let mut output = [f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY];
        for p in [
            [bounds[0], bounds[1]],
            [bounds[0], bounds[3]],
            [bounds[2], bounds[1]],
            [bounds[2], bounds[3]],
        ] {
            let p = point(inv, p)?;
            output[0] = output[0].min(p[0]);
            output[1] = output[1].min(p[1]);
            output[2] = output[2].max(p[0]);
            output[3] = output[3].max(p[1]);
        }
        if cancelled() {
            return Err(EpsGraphicsError::Cancelled);
        }
        finite(output)
    }
    pub fn finish(self) -> EpsVectorScene {
        EpsVectorScene {
            nodes: self.nodes.freeze(),
            paints: self.paints.freeze(),
            length: self.used_paints,
        }
    }
}
impl EpsVectorScene {
    /// Materializes exactly one path under the caller's memory root. `transform`
    /// maps canonical world points into the renderer's coordinate space. For
    /// affine strokes, use the inverse paint CTM and apply the pen CTM while
    /// rasterizing; a viewport alone is sufficient for filled paths.
    /// Errors discard the private output, leaving the scene reusable.
    pub fn prepare_path<F: FnMut() -> bool>(
        &self,
        paint: usize,
        transform: EpsMatrix,
        budget: &MemoryBudget,
        mut cancelled: F,
    ) -> Result<SharedBuffer<EpsPathSegment>, EpsGraphicsError> {
        if cancelled() {
            return Err(EpsGraphicsError::Cancelled);
        }
        finite(transform)?;
        let count = self
            .paints()
            .get(paint)
            .ok_or(EpsGraphicsError::Range)?
            .node_count;
        let mut output = budget.try_buffer(count, EpsPathSegment::Move([0., 0.]))?;
        self.copy_path(paint, &mut output, &mut cancelled)?;
        for segment in output.iter_mut() {
            if cancelled() {
                return Err(EpsGraphicsError::Cancelled);
            }
            *segment = match *segment {
                EpsPathSegment::Move(p) => EpsPathSegment::Move(point(transform, p)?),
                EpsPathSegment::Line { start, end } => EpsPathSegment::Line {
                    start: point(transform, start)?,
                    end: point(transform, end)?,
                },
                EpsPathSegment::Close { start, end } => EpsPathSegment::Close {
                    start: point(transform, start)?,
                    end: point(transform, end)?,
                },
                EpsPathSegment::Curve {
                    start,
                    control1,
                    control2,
                    end,
                } => EpsPathSegment::Curve {
                    start: point(transform, start)?,
                    control1: point(transform, control1)?,
                    control2: point(transform, control2)?,
                    end: point(transform, end)?,
                },
            };
        }
        if cancelled() {
            return Err(EpsGraphicsError::Cancelled);
        }
        Ok(output.freeze())
    }
    pub fn paints(&self) -> &[EpsPaint] {
        &self.paints[..self.length]
    }
    /// Copies a painted path in source order into caller-owned scratch. A size
    /// error writes nothing; cancellation may leave a partial destination, which
    /// the caller must discard. The scene is immutable and remains reusable.
    pub fn copy_path<F: FnMut() -> bool>(
        &self,
        paint: usize,
        output: &mut [EpsPathSegment],
        mut cancelled: F,
    ) -> Result<usize, EpsGraphicsError> {
        if cancelled() {
            return Err(EpsGraphicsError::Cancelled);
        }
        let paint = self.paints().get(paint).ok_or(EpsGraphicsError::Range)?;
        if output.len() < paint.node_count {
            return Err(EpsGraphicsError::OutputSize);
        }
        let mut index = paint.node_count;
        let mut head = paint.head;
        while let Some(at) = head {
            if cancelled() {
                return Err(EpsGraphicsError::Cancelled);
            }
            let node = self.nodes[at];
            index -= 1;
            output[index] = node.segment;
            head = node.previous;
        }
        if cancelled() {
            return Err(EpsGraphicsError::Cancelled);
        }
        Ok(paint.node_count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn prepared_path_admits_exact_storage_transforms_and_releases_failed_output() {
        let source_root = MemoryBudget::new(100_000);
        let mut g = EpsGraphics::new(limits(), &source_root, || false).unwrap();
        g.scale(2., 3.).unwrap();
        g.move_to(1., 2.).unwrap();
        g.curve_to([2., 3., 4., 5., 6., 7.]).unwrap();
        g.close_path().unwrap();
        g.paint(EpsPaintKind::Stroke).unwrap();
        let scene = g.finish();
        let bytes = (3 * std::mem::size_of::<EpsPathSegment>()) as u64;
        let refused = MemoryBudget::new(bytes - 1);
        assert!(matches!(
            scene.prepare_path(0, IDENTITY, &refused, || false),
            Err(EpsGraphicsError::Memory(_))
        ));
        assert_eq!((refused.used(), refused.peak()), (0, 0));
        let root = MemoryBudget::new(bytes);
        let path = scene
            .prepare_path(0, inverse(scene.paints()[0].matrix).unwrap(), &root, || false)
            .unwrap();
        assert_eq!(path[0], EpsPathSegment::Move([1., 2.]));
        assert_eq!(
            path[1],
            EpsPathSegment::Curve {
                start: [1., 2.],
                control1: [2., 3.],
                control2: [4., 5.],
                end: [6., 7.]
            }
        );
        assert_eq!(
            path[2],
            EpsPathSegment::Close {
                start: [6., 7.],
                end: [1., 2.]
            }
        );
        let last = path.clone();
        drop(path);
        assert_eq!(root.used(), bytes);
        drop(last);
        assert_eq!(root.used(), 0);
        let mut polls = 0;
        assert!(matches!(
            scene.prepare_path(0, IDENTITY, &root, || {
                polls += 1;
                polls == 7
            }),
            Err(EpsGraphicsError::Cancelled)
        ));
        assert_eq!(root.used(), 0);
        assert!(matches!(
            scene.prepare_path(0, [f64::MAX, 0., 0., 1., 0., 0.], &root, || false),
            Err(EpsGraphicsError::Range)
        ));
        assert_eq!(root.used(), 0);
        assert!(scene.prepare_path(0, IDENTITY, &root, || false).is_ok());
        assert_eq!(root.used(), 0);
    }
    fn limits() -> EpsGraphicsLimits {
        EpsGraphicsLimits {
            max_nodes: 128,
            max_paints: 8,
            max_saved_states: 16,
        }
    }
    #[test]
    fn independent_graphics_queries_match_ghostscript_state_and_path_rules() {
        let reference: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/eps/graphics-ghostscript-reference.json"
        ))
        .unwrap();
        let cases = reference["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 17);
        let mut max_error = 0f64;
        for case in cases {
            let root = MemoryBudget::new(100_000);
            let mut g = EpsGraphics::new(limits(), &root, || false).unwrap();
            let mut values = Vec::new();
            for action in case["actions"].as_array().unwrap() {
                let a = action.as_array().unwrap();
                let op = a[0].as_str().unwrap();
                let n = |i: usize| a[i].as_f64().unwrap();
                match op {
                    "moveto" => g.move_to(n(1), n(2)).unwrap(),
                    "lineto" => g.line_to(n(1), n(2)).unwrap(),
                    "rmoveto" => g.relative_move_to(n(1), n(2)).unwrap(),
                    "rlineto" => g.relative_line_to(n(1), n(2)).unwrap(),
                    "curveto" => g.curve_to([n(1), n(2), n(3), n(4), n(5), n(6)]).unwrap(),
                    "rcurveto" => g.relative_curve_to([n(1), n(2), n(3), n(4), n(5), n(6)]).unwrap(),
                    "translate" => g.translate(n(1), n(2)).unwrap(),
                    "scale" => g.scale(n(1), n(2)).unwrap(),
                    "rotate" => g.rotate(n(1)).unwrap(),
                    "closepath" => g.close_path().unwrap(),
                    "newpath" => g.new_path(),
                    "gsave" => g.gsave().unwrap(),
                    "grestore" => g.grestore().unwrap(),
                    "stroke" => g.paint(EpsPaintKind::Stroke).unwrap(),
                    "fill" => g.paint(EpsPaintKind::FillNonZero).unwrap(),
                    "currentpoint" => values.extend(g.current_point().unwrap()),
                    "pathbbox" => values.extend(g.path_bbox(|| false).unwrap()),
                    "setrgbcolor" => g.set_color(EpsDeviceColor::Rgb([n(1), n(2), n(3)])).unwrap(),
                    "setcmykcolor" => g
                        .set_color(EpsDeviceColor::Cmyk([n(1), n(2), n(3), n(4)]))
                        .unwrap(),
                    "currentrgbcolor" => {
                        let EpsDeviceColor::Rgb(v) = g.style().color else {
                            panic!()
                        };
                        values.extend(v);
                    }
                    "currentcmykcolor" => {
                        let EpsDeviceColor::Cmyk(v) = g.style().color else {
                            panic!()
                        };
                        values.extend(v);
                    }
                    "setlinewidth" => g.set_line_width(n(1)).unwrap(),
                    "currentlinewidth" => values.push(g.style().width),
                    _ => panic!("unknown reference action {op}"),
                }
            }
            let expected = case["values"].as_array().unwrap();
            assert_eq!(values.len(), expected.len());
            for (actual, expected) in values.iter().zip(expected) {
                let error = (actual - expected.as_f64().unwrap()).abs();
                max_error = max_error.max(error);
                assert!(
                    error <= 0.001,
                    "{case}: actual={actual}, expected={expected}, error={error}"
                );
            }
            drop(g);
            assert_eq!(root.used(), 0);
        }
        eprintln!("17 graphics reference cases; max numeric difference={max_error}");
    }
    #[test]
    fn persistent_fill_stroke_scene_reuses_geometry_preserves_pen_and_last_owner() {
        let root = MemoryBudget::new(100_000);
        let caps = EpsGraphicsLimits {
            max_nodes: 8,
            max_paints: 3,
            max_saved_states: 2,
        };
        let mut g = EpsGraphics::new(caps, &root, || false).unwrap();
        g.move_to(0., 0.).unwrap();
        g.line_to(10., 0.).unwrap();
        g.line_to(10., 10.).unwrap();
        g.close_path().unwrap();
        g.set_color(EpsDeviceColor::Rgb([0.1, 0.2, 0.3])).unwrap();
        g.gsave().unwrap();
        g.paint(EpsPaintKind::FillNonZero).unwrap();
        assert!(matches!(g.current_point(), Err(EpsGraphicsError::NoCurrentPoint)));
        g.grestore().unwrap();
        assert_eq!(g.current_point().unwrap(), [0., 0.]);
        g.set_line_width(2.).unwrap();
        g.scale(2., 3.).unwrap();
        g.paint(EpsPaintKind::Stroke).unwrap();
        assert_eq!(g.arena_nodes_used(), 4);
        let scene = g.finish();
        assert_eq!(scene.paints().len(), 2);
        assert_eq!(scene.paints()[0].head, scene.paints()[1].head);
        assert_eq!(scene.paints()[1].matrix, [2., 0., 0., 3., 0., 0.]);
        assert_eq!(scene.paints()[1].style.width, 2.);
        let mut fill = [EpsPathSegment::Move([0., 0.]); 4];
        let mut stroke = fill;
        scene.copy_path(0, &mut fill, || false).unwrap();
        scene.copy_path(1, &mut stroke, || false).unwrap();
        assert_eq!(fill, stroke);
        assert_eq!(fill[0], EpsPathSegment::Move([0., 0.]));
        assert_eq!(
            fill[3],
            EpsPathSegment::Close {
                start: [10., 10.],
                end: [0., 0.]
            }
        );
        let admitted = (caps.max_nodes * std::mem::size_of::<Node>()
            + caps.max_paints * std::mem::size_of::<EpsPaint>()) as u64;
        assert_eq!(root.used(), admitted);
        let held = scene.clone();
        drop(scene);
        assert_eq!(root.used(), admitted);
        drop(held);
        assert_eq!(root.used(), 0);
    }
    #[test]
    fn limits_restore_forks_and_failed_operations_preserve_the_active_path() {
        let root = MemoryBudget::new(100_000);
        let caps = EpsGraphicsLimits {
            max_nodes: 4,
            max_paints: 1,
            max_saved_states: 1,
        };
        let mut g = EpsGraphics::new(caps, &root, || false).unwrap();
        g.move_to(0., 0.).unwrap();
        g.line_to(3., 0.).unwrap();
        g.gsave().unwrap();
        g.line_to(8., 0.).unwrap();
        assert!(matches!(
            g.gsave(),
            Err(EpsGraphicsError::Limit(EpsGraphicsLimit::SavedStates))
        ));
        g.grestore().unwrap();
        g.line_to(4., 0.).unwrap();
        assert_eq!(g.node_count(), 3);
        assert_eq!(g.arena_nodes_used(), 4);
        assert!(matches!(
            g.relative_move_to(1., 0.),
            Err(EpsGraphicsError::Limit(EpsGraphicsLimit::Nodes))
        ));
        assert_eq!(g.current_point().unwrap(), [4., 0.]);
        g.grestore().unwrap();
        assert_eq!(g.current_point().unwrap(), [4., 0.]);
        g.paint(EpsPaintKind::FillEvenOdd).unwrap();
        let scene = g.finish();
        let mut path = [EpsPathSegment::Move([0., 0.]); 3];
        scene.copy_path(0, &mut path, || false).unwrap();
        assert_eq!(
            path[2],
            EpsPathSegment::Line {
                start: [3., 0.],
                end: [4., 0.]
            }
        );
        drop(scene);
        assert_eq!(root.used(), 0);
        let mut g = EpsGraphics::new(caps, &root, || false).unwrap();
        g.move_to(0., 0.).unwrap();
        g.line_to(1., 0.).unwrap();
        g.close_path().unwrap();
        assert!(matches!(
            g.line_to(2., 0.),
            Err(EpsGraphicsError::Limit(EpsGraphicsLimit::Nodes))
        ));
        assert_eq!(g.arena_nodes_used(), 3);
        assert_eq!(g.current_point().unwrap(), [0., 0.]);
        let matrix = g.matrix();
        assert!(g.concat([f64::INFINITY, 0., 0., 1., 0., 0.]).is_err());
        assert_eq!(g.matrix(), matrix);
        let style = g.style();
        assert!(g.set_color(EpsDeviceColor::Gray(f64::NAN)).is_err());
        assert_eq!(g.style(), style);
        assert!(g.set_line_width(-1.).is_err());
        assert!(g.set_line_cap(3).is_err());
        assert!(g.set_line_join(3).is_err());
        assert!(g.set_miter_limit(0.5).is_err());
        g.scale(0., 1.).unwrap();
        assert!(matches!(g.current_point(), Err(EpsGraphicsError::InvalidMatrix)));
        drop(g);
        assert_eq!(root.used(), 0);
    }
    #[test]
    fn root_admission_query_cancellation_and_destination_size_never_corrupt_scene() {
        let caps = EpsGraphicsLimits {
            max_nodes: 4,
            max_paints: 1,
            max_saved_states: 1,
        };
        let root = MemoryBudget::new(100_000);
        let mut polls = 0;
        assert!(matches!(
            EpsGraphics::new(caps, &root, || {
                polls += 1;
                polls == 2
            }),
            Err(EpsGraphicsError::Cancelled)
        ));
        assert_eq!(root.used(), 0);
        assert!(matches!(
            EpsGraphics::new(caps, &root.child(1), || false),
            Err(EpsGraphicsError::Memory(_))
        ));
        assert_eq!(root.used(), 0);
        let mut g = EpsGraphics::new(caps, &root, || false).unwrap();
        g.close_path().unwrap();
        assert_eq!(g.arena_nodes_used(), 0);
        assert!(matches!(g.line_to(1., 2.), Err(EpsGraphicsError::NoCurrentPoint)));
        assert!(matches!(
            g.path_bbox(|| false),
            Err(EpsGraphicsError::NoCurrentPoint)
        ));
        g.move_to(1., 2.).unwrap();
        g.line_to(3., 4.).unwrap();
        let mut polls = 0;
        assert!(matches!(
            g.path_bbox(|| {
                polls += 1;
                polls == 3
            }),
            Err(EpsGraphicsError::Cancelled)
        ));
        assert_eq!(polls, 3);
        assert_eq!(g.current_point().unwrap(), [3., 4.]);
        g.gsave().unwrap();
        g.paint(EpsPaintKind::Stroke).unwrap();
        g.grestore().unwrap();
        assert!(matches!(
            g.paint(EpsPaintKind::Stroke),
            Err(EpsGraphicsError::Limit(EpsGraphicsLimit::Paints))
        ));
        assert_eq!(g.current_point().unwrap(), [3., 4.]);
        let scene = g.finish();
        let sentinel = EpsPathSegment::Move([99., 99.]);
        let mut short = [sentinel];
        assert!(matches!(
            scene.copy_path(0, &mut short, || false),
            Err(EpsGraphicsError::OutputSize)
        ));
        assert_eq!(short, [sentinel]);
        let mut output = [sentinel; 2];
        let mut polls = 0;
        assert!(matches!(
            scene.copy_path(0, &mut output, || {
                polls += 1;
                polls == 3
            }),
            Err(EpsGraphicsError::Cancelled)
        ));
        scene.copy_path(0, &mut output, || false).unwrap();
        assert_eq!(output[0], EpsPathSegment::Move([1., 2.]));
        assert_eq!(
            output[1],
            EpsPathSegment::Line {
                start: [1., 2.],
                end: [3., 4.]
            }
        );
        drop(scene);
        assert_eq!(root.used(), 0);
    }
}
