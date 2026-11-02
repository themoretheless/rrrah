//! Admitted static scene hierarchy; source geometry remains shared and lazy.
use crate::{
    DecodeError, DecodeRequest,
    glb::{GlbDocument, GlbError, GlbPrimitive, GlbTransform, JsonValue},
    glb_accessor::integer,
    glb_transform::transform_from_json,
};
use rrrah_core::{MemoryBudget, PixelBuffer};
const NONE: u32 = u32::MAX;
pub const MAX_SCENE_NODES: usize = 65_536;
pub const MAX_SCENE_DEPTH: usize = 64;
#[derive(Debug, Clone, Copy)]
struct Node {
    world: GlbTransform,
    source: u32,
    parent: u32,
    child: u32,
    sibling: u32,
    mesh: u32,
    next: u32,
    stamp: u32,
    state: u8,
    depth: u8,
    selected: bool,
}
const EMPTY: Node = Node {
    world: GlbTransform::identity(),
    source: 0,
    parent: NONE,
    child: NONE,
    sibling: NONE,
    mesh: NONE,
    next: NONE,
    stamp: 0,
    state: 0,
    depth: 0,
    selected: false,
};
#[derive(Debug, Clone)]
pub struct GlbScene {
    document: GlbDocument,
    nodes: PixelBuffer<Node>,
    index: usize,
    first: u32,
}
#[derive(Debug, Clone, Copy)]
pub struct GlbInstance {
    pub node: usize,
    pub mesh: usize,
    pub transform: GlbTransform,
}
#[derive(Debug, Clone, Copy)]
pub struct GlbScenePrimitive<'a> {
    pub local: GlbPrimitive<'a>,
    pub transform: GlbTransform,
}
impl GlbScenePrimitive<'_> {
    pub fn triangle(self, index: usize) -> Result<Option<[[f64; 3]; 3]>, GlbError> {
        let Some(local) = self.local.triangle(index) else {
            return Ok(None);
        };
        Ok(Some([
            self.transform.point(local[0])?,
            self.transform.point(local[1])?,
            self.transform.point(local[2])?,
        ]))
    }
    pub fn triangles(self) -> impl ExactSizeIterator<Item = Result<[[f64; 3]; 3], GlbError>> + Clone {
        (0..self.local.triangle_count()).map(move |i| self.triangle(i).map(|v| v.unwrap()))
    }
}
fn count(root: JsonValue<'_>, name: &str, request: &DecodeRequest) -> Result<usize, GlbError> {
    let Some(value) = root.field(name, request)? else {
        return Ok(0);
    };
    let mut count = 0;
    for _ in value.array().ok_or(GlbError::Invalid("scene property array"))? {
        request.check_cancelled()?;
        count += 1;
    }
    if count == 0 {
        return Err(GlbError::Invalid("empty declared scene array"));
    }
    Ok(count)
}
impl GlbDocument {
    /// Static source pose; optional selection overrides asset.scene, otherwise
    /// missing asset.scene deterministically selects scene0. No mesh flattening.
    pub fn scene(&self, selection: Option<usize>, request: &DecodeRequest) -> Result<GlbScene, GlbError> {
        request.check_cancelled()?;
        let root = self.root();
        let default = root.field("scene", request)?.map(integer).transpose()?;
        let scenes = count(root, "scenes", request)?;
        if scenes == 0 {
            return Err(if default.is_some() {
                GlbError::Invalid("default scene without scenes")
            } else {
                GlbError::Unsupported("asset without scene")
            });
        }
        if default.is_some_and(|v| v >= scenes) {
            return Err(GlbError::Invalid("default scene index"));
        }
        let index = selection.or(default).unwrap_or(0);
        if index >= scenes {
            return Err(GlbError::Invalid("selected scene index"));
        }
        let length = count(root, "nodes", request)?;
        if length > MAX_SCENE_NODES {
            return Err(GlbError::Limit("scene node count"));
        }
        let meshes = count(root, "meshes", request)?;
        let fallback = MemoryBudget::new((length * size_of::<Node>()) as u64);
        let budget = request.memory_budget.as_ref().unwrap_or(&fallback);
        let mut nodes = budget.try_buffer(length, EMPTY).map_err(DecodeError::Memory)?;
        if let Some(values) = root.field("nodes", request)? {
            for (i, value) in values.array().unwrap().enumerate() {
                request.check_cancelled()?;
                nodes[i].source = value.token_index();
                nodes[i].world = transform_from_json(value, request)?;
                if let Some(mesh) = value.field("mesh", request)? {
                    let mesh = integer(mesh)?;
                    if mesh >= meshes {
                        return Err(GlbError::Invalid("node mesh reference"));
                    }
                    nodes[i].mesh = mesh as u32;
                }
            }
        }
        // Each edge is visited once; parent sentinel also catches duplicate children.
        for i in 0..length {
            request.check_cancelled()?;
            let value = self.json_at(nodes[i].source);
            if let Some(children) = value.field("children", request)? {
                let mut previous: Option<usize> = None;
                let mut any = false;
                for child in children.array().ok_or(GlbError::Invalid("node children array"))? {
                    request.check_cancelled()?;
                    let child = integer(child)?;
                    if child >= length {
                        return Err(GlbError::Invalid("child node reference"));
                    }
                    if nodes[child].parent != NONE {
                        return Err(GlbError::Invalid("node multiple parents or duplicate child"));
                    }
                    nodes[child].parent = i as u32;
                    if let Some(previous) = previous {
                        nodes[previous].sibling = child as u32;
                    } else {
                        nodes[i].child = child as u32;
                    }
                    previous = Some(child);
                    any = true;
                }
                if !any {
                    return Err(GlbError::Invalid("empty children array"));
                }
            }
        }
        // Iterative parent-chain resolution; completed parents are never recomputed.
        for start in 0..length {
            request.check_cancelled()?;
            if nodes[start].state == 2 {
                continue;
            }
            let mut path = [0usize; MAX_SCENE_DEPTH];
            let mut used = 0;
            let mut at = start as u32;
            while at != NONE && nodes[at as usize].state != 2 {
                let i = at as usize;
                if nodes[i].state == 1 {
                    return Err(GlbError::Invalid("node hierarchy cycle"));
                }
                if used == MAX_SCENE_DEPTH {
                    return Err(GlbError::Limit("scene hierarchy depth"));
                }
                nodes[i].state = 1;
                path[used] = i;
                used += 1;
                at = nodes[i].parent;
            }
            for i in path[..used].iter().rev().copied() {
                request.check_cancelled()?;
                let parent = nodes[i].parent;
                let depth = if parent == NONE {
                    1
                } else {
                    usize::from(nodes[parent as usize].depth) + 1
                };
                if depth > MAX_SCENE_DEPTH {
                    return Err(GlbError::Limit("scene hierarchy depth"));
                }
                if parent != NONE {
                    nodes[i].world = nodes[parent as usize].world.compose(nodes[i].world)?;
                }
                nodes[i].depth = depth as u8;
                nodes[i].state = 2;
            }
        }
        let mut first = NONE;
        let mut last: Option<usize> = None;
        for (scene_index, scene) in root
            .field("scenes", request)?
            .unwrap()
            .array()
            .unwrap()
            .enumerate()
        {
            request.check_cancelled()?;
            // Even empty scenes must be objects. Other metadata stays in JSON.
            let roots = scene.field("nodes", request)?;
            if let Some(roots) = roots {
                let mut any = false;
                for root in roots.array().ok_or(GlbError::Invalid("scene roots array"))? {
                    request.check_cancelled()?;
                    any = true;
                    let root = integer(root)?;
                    if root >= length {
                        return Err(GlbError::Invalid("scene root reference"));
                    }
                    if nodes[root].parent != NONE {
                        return Err(GlbError::Invalid("scene entry is not a root"));
                    }
                    let stamp = (scene_index + 1) as u32;
                    if nodes[root].stamp == stamp {
                        return Err(GlbError::Invalid("duplicate scene root"));
                    }
                    nodes[root].stamp = stamp;
                    if scene_index != index {
                        continue;
                    }
                    let mut at = root;
                    loop {
                        request.check_cancelled()?;
                        nodes[at].selected = true;
                        let source = self.json_at(nodes[at].source);
                        if source.field("skin", request)?.is_some()
                            || source.field("weights", request)?.is_some()
                        {
                            return Err(GlbError::Unsupported("selected node skin/morph weights"));
                        }
                        if let Some(last) = last {
                            nodes[last].next = at as u32;
                        } else {
                            first = at as u32;
                        }
                        last = Some(at);
                        if nodes[at].child != NONE {
                            at = nodes[at].child as usize;
                            continue;
                        }
                        while at != root && nodes[at].sibling == NONE {
                            at = nodes[at].parent as usize;
                        }
                        if at == root {
                            break;
                        }
                        at = nodes[at].sibling as usize;
                    }
                }
                if !any {
                    return Err(GlbError::Invalid("empty scene roots array"));
                }
            }
        }
        request.check_cancelled()?;
        Ok(GlbScene {
            document: self.clone(),
            nodes: nodes.freeze().into(),
            index,
            first,
        })
    }
}
impl GlbScene {
    pub fn scene_index(&self) -> usize {
        self.index
    }
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }
    pub fn document(&self) -> &GlbDocument {
        &self.document
    }
    pub fn world_transform(&self, node: usize) -> Option<GlbTransform> {
        self.nodes.get(node).map(|v| v.world)
    }
    pub fn selected_nodes(&self) -> impl Iterator<Item = usize> + Clone + '_ {
        let mut at = self.first;
        std::iter::from_fn(move || {
            if at == NONE {
                return None;
            }
            let index = at as usize;
            at = self.nodes[index].next;
            Some(index)
        })
    }
    pub fn instances(&self) -> impl Iterator<Item = GlbInstance> + Clone + '_ {
        self.selected_nodes().filter_map(|index| {
            let value = self.nodes[index];
            (value.mesh != NONE).then_some(GlbInstance {
                node: index,
                mesh: value.mesh as usize,
                transform: value.world,
            })
        })
    }
    /// Geometry is validated lazily when requested; scene construction validates
    /// structure/transforms, not every mesh/material. Source pose ignores animation.
    pub fn primitive(
        &self,
        node: usize,
        primitive: usize,
        request: &DecodeRequest,
    ) -> Result<GlbScenePrimitive<'_>, GlbError> {
        request.check_cancelled()?;
        let value = self
            .nodes
            .get(node)
            .filter(|v| v.selected && v.mesh != NONE)
            .ok_or(GlbError::Invalid("node is not a selected mesh instance"))?;
        Ok(GlbScenePrimitive {
            local: self.document.primitive(value.mesh as usize, primitive, request)?,
            transform: value.world,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    };
    fn fixture(name: &str) -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/glb")
            .join(name)
    }
    fn fixture_request(name: &str, budget: &MemoryBudget) -> DecodeRequest {
        let mut r = DecodeRequest::new(fixture(name));
        r.memory_budget = Some(budget.clone());
        r
    }
    fn decode(bytes: &[u8], budget: &MemoryBudget) -> (GlbDocument, DecodeRequest) {
        static SERIAL: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "rrrah-glb-scene-{}-{}.glb",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(&path, bytes).unwrap();
        let mut request = DecodeRequest::new(&path);
        request.memory_budget = Some(budget.clone());
        let document = GlbDocument::read(&request).unwrap();
        std::fs::remove_file(path).unwrap();
        (document, request)
    }
    fn json(text: &str, budget: &MemoryBudget) -> (GlbDocument, DecodeRequest) {
        let mut text = text.as_bytes().to_vec();
        while text.len() % 4 != 0 {
            text.push(b' ');
        }
        let mut bytes = b"glTF\x02\0\0\0".to_vec();
        bytes.extend_from_slice(&((20 + text.len()) as u32).to_le_bytes());
        bytes.extend_from_slice(&(text.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&0x4e4f534au32.to_le_bytes());
        bytes.extend_from_slice(&text);
        decode(&bytes, budget)
    }
    fn close(a: [f64; 3], b: [f64; 3]) {
        for c in 0..3 {
            assert!(
                (a[c] - b[c]).abs() <= 2e-14 * b[c].abs().max(1.),
                "{a:?} != {b:?}"
            );
        }
    }
    #[test]
    fn automatic_box_scene_matches_original_transformed_triangles_at_exact_metadata_budget() {
        assert_eq!(size_of::<Node>(), 160);
        let budget = MemoryBudget::new(4624);
        let request = fixture_request("Box.glb", &budget);
        let document = GlbDocument::read(&request).unwrap();
        let scene = document.scene(None, &request).unwrap();
        assert_eq!(budget.used(), 4624);
        assert_eq!(scene.scene_index(), 0);
        assert_eq!(scene.node_count(), 2);
        assert_eq!(scene.selected_nodes().collect::<Vec<_>>(), [0, 1]);
        let instances = scene.instances().collect::<Vec<_>>();
        assert_eq!(instances.len(), 1);
        assert_eq!(instances[0].node, 1);
        assert_eq!(instances[0].mesh, 0);
        assert_eq!(
            scene.document().source().bin_bytes().unwrap().as_ptr(),
            document.source().bin_bytes().unwrap().as_ptr()
        );
        let primitive = scene.primitive(1, 0, &request).unwrap();
        let mut count = 0;
        for line in std::fs::read_to_string(fixture("Box-node-triangles.tsv"))
            .unwrap()
            .lines()
        {
            let v: Vec<f64> = line.split('\t').map(|v| v.parse().unwrap()).collect();
            let t = primitive.triangle(v[0] as usize).unwrap().unwrap();
            for c in 0..3 {
                close(t[c], [v[1 + c * 3], v[2 + c * 3], v[3 + c * 3]]);
            }
            count += 1;
        }
        assert_eq!(count, 12);
        assert_eq!(primitive.triangles().len(), 12);
        assert!(primitive.triangle(12).unwrap().is_none());
        assert_eq!(budget.peak(), 4624);
        let clone = scene.clone();
        drop(document);
        drop(scene);
        assert_eq!(budget.used(), 4624);
        drop(clone);
        assert_eq!(budget.used(), 0);
        let short = MemoryBudget::new(4623);
        let request = fixture_request("Box.glb", &short);
        let document = GlbDocument::read(&request).unwrap();
        assert!(matches!(
            document.scene(None, &request),
            Err(GlbError::Source(DecodeError::Memory(_)))
        ));
        assert_eq!(short.peak(), 4304);
        drop(document);
        assert_eq!(short.used(), 0);
    }
    #[test]
    fn scene_selection_preorder_instancing_and_decimal_world_geometry_match() {
        let budget = MemoryBudget::new(1024 * 1024);
        let request = fixture_request("scene-controls.glb", &budget);
        let document = GlbDocument::read(&request).unwrap();
        let source = budget.used();
        let order = std::fs::read_to_string(fixture("scene-instances.tsv")).unwrap();
        let triangles = std::fs::read_to_string(fixture("scene-triangles.tsv")).unwrap();
        let mut triangle_count = 0;
        let mut instance_count = 0;
        for index in 0..4 {
            let scene = document.scene(Some(index), &request).unwrap();
            assert_eq!(scene.scene_index(), index);
            assert_eq!(scene.node_count(), 7);
            let expected: Vec<(usize, usize)> = order
                .lines()
                .filter_map(|line| {
                    let v: Vec<usize> = line.split('\t').map(|v| v.parse().unwrap()).collect();
                    (v[0] == index).then_some((v[1], v[2]))
                })
                .collect();
            let actual: Vec<_> = scene.instances().map(|v| (v.node, v.mesh)).collect();
            assert_eq!(actual, expected);
            instance_count += actual.len();
            if index == 0 {
                assert_eq!(scene.selected_nodes().collect::<Vec<_>>(), [0, 3, 1, 2]);
                assert!(scene.primitive(4, 0, &request).is_err());
            }
            if index == 3 {
                assert_eq!(scene.selected_nodes().count(), 0);
            }
            let used = budget.used();
            let peak = budget.peak();
            for line in triangles.lines() {
                let v: Vec<f64> = line.split('\t').map(|v| v.parse().unwrap()).collect();
                if v[0] as usize != index {
                    continue;
                }
                let p = scene.primitive(v[1] as usize, 0, &request).unwrap();
                let t = p.triangle(v[2] as usize).unwrap().unwrap();
                for c in 0..3 {
                    close(t[c], [v[3 + c * 3], v[4 + c * 3], v[5 + c * 3]]);
                }
                triangle_count += 1;
                assert_eq!(budget.used(), used);
                assert_eq!(budget.peak(), peak);
            }
            drop(scene);
            assert_eq!(budget.used(), source);
        }
        assert_eq!(instance_count, 8);
        assert_eq!(triangle_count, 18);
        let default = document.scene(None, &request).unwrap();
        assert_eq!(default.scene_index(), 1);
        assert_eq!(default.instances().next().unwrap().node, 4);
        drop(default);
        assert!(document.scene(Some(usize::MAX), &request).is_err());
        assert_eq!(budget.used(), source);
        drop(document);
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn malformed_forests_scene_roots_and_unsupported_selected_nodes_release_admission() {
        let budget = MemoryBudget::new(1024 * 1024);
        let mut cases = 0;
        for line in std::fs::read_to_string(fixture("scene-invalid.tsv"))
            .unwrap()
            .lines()
        {
            let (name, hex) = line.split_once('\t').unwrap();
            let bytes: Vec<u8> = hex
                .as_bytes()
                .chunks_exact(2)
                .map(|v| u8::from_str_radix(std::str::from_utf8(v).unwrap(), 16).unwrap())
                .collect();
            let (document, request) = decode(&bytes, &budget);
            let used = budget.used();
            assert!(document.scene(None, &request).is_err(), "{name}");
            assert_eq!(budget.used(), used);
            drop(document);
            assert_eq!(budget.used(), 0);
            cases += 1;
        }
        assert_eq!(cases, 23);
    }
    #[test]
    fn depth_node_limits_empty_scene_and_large_flat_forest_are_bounded() {
        let budget = MemoryBudget::new(4 * 1024 * 1024);
        for depth in [64, 65] {
            let nodes: Vec<String> = (0..depth)
                .map(|i| {
                    if i == 0 {
                        "{\"translation\":[1,0,0]}".into()
                    } else {
                        format!("{{\"translation\":[1,0,0],\"children\":[{}]}}", i - 1)
                    }
                })
                .collect();
            let text = format!(
                "{{\"asset\":{{\"version\":\"2.0\"}},\"nodes\":[{}],\"scenes\":[{{\"nodes\":[{}]}}]}}",
                nodes.join(","),
                depth - 1
            );
            let (document, request) = json(&text, &budget);
            let result = document.scene(None, &request);
            if depth == 64 {
                let scene = result.unwrap();
                assert_eq!(scene.selected_nodes().count(), 64);
                assert_eq!(
                    scene.world_transform(0).unwrap().point([0.; 3]).unwrap(),
                    [64., 0., 0.]
                );
            } else {
                assert!(matches!(result, Err(GlbError::Limit("scene hierarchy depth"))));
            }
            drop(document);
            assert_eq!(budget.used(), 0);
        }
        let count = 2048;
        let nodes = vec!["{}"; count].join(",");
        let roots = (0..count).map(|i| i.to_string()).collect::<Vec<_>>().join(",");
        let text = format!(
            "{{\"asset\":{{\"version\":\"2.0\"}},\"nodes\":[{nodes}],\"scenes\":[{{\"nodes\":[{roots}]}}]}}"
        );
        let (document, request) = json(&text, &budget);
        let scene = document.scene(None, &request).unwrap();
        assert_eq!(scene.selected_nodes().count(), count);
        assert_eq!(scene.instances().count(), 0);
        drop(scene);
        drop(document);
        assert_eq!(budget.used(), 0);
        let nodes = vec!["{}"; MAX_SCENE_NODES + 1].join(",");
        let (document, request) = json(
            &format!("{{\"asset\":{{\"version\":\"2.0\"}},\"nodes\":[{nodes}],\"scenes\":[{{}}]}}"),
            &budget,
        );
        let source = budget.used();
        assert!(matches!(
            document.scene(None, &request),
            Err(GlbError::Limit("scene node count"))
        ));
        assert_eq!(budget.peak(), source);
        drop(document);
        assert_eq!(budget.used(), 0);
        let (document, request) = json("{\"asset\":{\"version\":\"2.0\"},\"scenes\":[{}]}", &budget);
        let scene = document.scene(None, &request).unwrap();
        assert_eq!(scene.node_count(), 0);
        assert_eq!(scene.instances().count(), 0);
        drop(scene);
        drop(document);
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn pressure_cancellation_new_generation_and_last_clone_allow_exact_retry() {
        let budget = MemoryBudget::new(4624);
        let mut request = fixture_request("Box.glb", &budget);
        let generation = Arc::new(AtomicU64::new(1));
        request.cancellation = Some(crate::GenerationToken::new(generation.clone(), 1));
        let document = GlbDocument::read(&request).unwrap();
        let pressure = budget.try_reserve(1).unwrap();
        assert!(matches!(
            document.scene(None, &request),
            Err(GlbError::Source(DecodeError::Memory(_)))
        ));
        assert_eq!(budget.used(), 4305);
        drop(pressure);
        generation.store(2, Ordering::Release);
        assert!(matches!(
            document.scene(None, &request),
            Err(GlbError::Source(DecodeError::Cancelled))
        ));
        assert_eq!(budget.used(), 4304);
        request.cancellation = Some(crate::GenerationToken::new(generation, 2));
        let scene = document.scene(None, &request).unwrap();
        assert_eq!(scene.primitive(1, 0, &request).unwrap().triangles().count(), 12);
        drop(document);
        assert_eq!(budget.used(), 4624);
        drop(scene);
        assert_eq!(budget.used(), 0);
    }
}
