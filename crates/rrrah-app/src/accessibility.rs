//! Accessible command surface for the keyboard-driven viewer.
use accesskit::{Action, Live, Node, NodeId, Role, Tree, TreeId, TreeUpdate};
pub const ROOT: NodeId = NodeId(1);
pub const IMAGE: NodeId = NodeId(2);
pub const STATUS: NodeId = NodeId(3);
pub const FIRST_COMMAND: u64 = 10;
pub const COMMANDS: [&str; 8] = [
    "Previous image",
    "Next image",
    "Decrease exposure",
    "Increase exposure",
    "Undo edit",
    "Redo edit",
    "Save XMP edits",
    "Compare before and after",
];
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
    fn commands_are_focusable_and_selection_and_errors_have_text() {
        let update = tree(
            "image 2 of 3: picture.exr",
            "read failed",
            Some("selected folder"),
            NodeId(10),
        );
        assert_eq!(update.focus, NodeId(10));
        assert_eq!(update.nodes.len(), COMMANDS.len() + 4);
        assert_eq!(next_focus(NodeId(10), true), NodeId(17));
        assert_eq!(next_focus(NodeId(17), false), NodeId(10));
        assert!(
            update
                .nodes
                .iter()
                .any(|(id, node)| *id == NodeId(4) && node.is_selected() == Some(true))
        );
    }
}
