//! Accessible command surface for the keyboard-driven viewer.
use accesskit::{Action, Live, Node, NodeId, Role, Tree, TreeId, TreeUpdate};
pub const ROOT: NodeId = NodeId(1);
pub const IMAGE: NodeId = NodeId(2);
pub const STATUS: NodeId = NodeId(3);
pub const FIRST_COMMAND: u64 = 10;
pub const COMMANDS: [&str; 30] = [
    "Previous image",
    "Next image",
    "Decrease exposure",
    "Increase exposure",
    "Undo edit",
    "Redo edit",
    "Save XMP edits",
    "Compare before and after",
    "Resolve edit conflict using local edits",
    "Resolve edit conflict using saved version",
    "Start clean edits and retain refused recovery in a backup",
    "Edit tone curve",
    "Show source histogram",
    "Toggle split comparison",
    "Zoom in",
    "Zoom out",
    "Pan image left",
    "Pan image right",
    "Pan image up",
    "Pan image down",
    "Fit image",
    "Select next curve point",
    "Add curve midpoint",
    "Remove selected curve point",
    "Move curve point left",
    "Move curve point right",
    "Move curve point down",
    "Move curve point up",
    "Reset tone curve",
    "Show timing panel (F6 returns to image)",
];
pub fn view_command(zoom: &mut f32, pan: &mut [f32; 2], id: u64) {
    match id {
        24 => *zoom = (*zoom * 1.25).clamp(0.02, 128.),
        25 => *zoom = (*zoom / 1.25).clamp(0.02, 128.),
        26 => pan[0] -= 32.,
        27 => pan[0] += 32.,
        28 => pan[1] -= 32.,
        29 => pan[1] += 32.,
        30 => {
            *zoom = 1.;
            *pan = [0.; 2];
        }
        _ => {}
    }
}
pub fn next_focus(focus: NodeId, backwards: bool) -> NodeId {
    let index = focus
        .0
        .saturating_sub(FIRST_COMMAND)
        .min(COMMANDS.len() as u64 - 1);
    NodeId(
        FIRST_COMMAND
            + if backwards {
                (index + COMMANDS.len() as u64 - 1) % COMMANDS.len() as u64
            } else {
                (index + 1) % COMMANDS.len() as u64
            },
    )
}
/// Announce collection position separately from a container's selected page/frame.
pub fn document_label(
    name: &str,
    file: Option<(usize, usize)>,
    image: (usize, usize),
    exposure: f32,
    before: bool,
) -> String {
    let file = file.filter(|(index, count)| index < count).map_or_else(
        || "file not indexed".into(),
        |(index, count)| format!("file {} of {count}", index + 1),
    );
    format!(
        "{name}; {file}; image {} of {}; exposure {exposure:+.2} stops{}",
        image.0 + 1,
        image.1,
        if before { "; before" } else { "" }
    )
}
pub fn tree(name: &str, status: &str, folder: Option<&str>, focus: NodeId) -> TreeUpdate {
    let mut image = Node::new(Role::Image);
    image.set_label(name);
    let mut announcement = Node::new(Role::Label);
    announcement.set_value(status);
    announcement.set_live(Live::Polite);
    let mut nodes = vec![(IMAGE, image), (STATUS, announcement)];
    let mut children = vec![IMAGE, STATUS];
    for (i, label) in COMMANDS.iter().enumerate() {
        let id = NodeId(FIRST_COMMAND + i as u64);
        let mut command = Node::new(Role::Button);
        command.set_label(*label);
        command.add_action(Action::Focus);
        command.add_action(Action::Click);
        nodes.push((id, command));
        children.push(id);
    }
    if let Some(folder) = folder {
        let mut selected = Node::new(Role::ListBoxOption);
        selected.set_label(folder);
        selected.set_selected(true);
        nodes.push((NodeId(4), selected));
        children.push(NodeId(4));
    }
    let mut root = Node::new(Role::Window);
    root.set_label("Rrrah");
    root.set_children(children);
    nodes.push((ROOT, root));
    TreeUpdate {
        nodes,
        tree: Some(Tree::new(ROOT)),
        tree_id: TreeId::ROOT,
        focus,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn collection_and_container_positions_are_distinct() {
        let label = super::document_label("photo.tiff", Some((2, 5)), (1, 4), 0.5, false);
        assert_eq!(
            label,
            "photo.tiff; file 3 of 5; image 2 of 4; exposure +0.50 stops"
        );
        assert!(
            super::document_label("single.pfm", None, (0, 1), 0., true)
                .contains("file not indexed; image 1 of 1")
        );
    }
    #[test]
    fn commands_are_focusable_and_selection_and_errors_have_text() {
        let update = tree(
            "image 2 of 3: picture.exr",
            "read failed",
            Some("selected folder"),
            NodeId(10),
        );
        assert_eq!(update.focus, NodeId(10));
        assert_eq!(update.nodes.len(), COMMANDS.len() + 4);
        assert_eq!(next_focus(NodeId(10), true), NodeId(39));
        assert_eq!(next_focus(NodeId(39), false), NodeId(10));
        let mut focus = NodeId(10);
        let mut visited = std::collections::BTreeSet::new();
        for _ in 0..COMMANDS.len() {
            visited.insert(focus.0);
            focus = next_focus(focus, false);
        }
        assert_eq!(visited, (10..=39).collect());
        assert_eq!(focus, NodeId(10));
        assert!(
            update
                .nodes
                .iter()
                .any(|(id, node)| *id == NodeId(4) && node.is_selected() == Some(true))
        );
    }
}
