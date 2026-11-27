use azalea_protocol::packets::game::c_commands::BrigadierParser;

use super::parse::{Parse, is_unquoted_char, parse_as};
use super::tree::{CommandTree, NodeKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Check {
    NoIssues,
    ParseErrors,
    SignatureRequired,
    PermissionsRequired,
}

impl Check {
    pub fn message_key(self) -> Option<&'static str> {
        match self {
            Check::NoIssues => None,
            Check::ParseErrors => Some("multiplayer.confirm_command.parse_errors"),
            Check::SignatureRequired => Some("multiplayer.confirm_command.signature_required"),
            Check::PermissionsRequired => Some("multiplayer.confirm_command.permissions_required"),
        }
    }
}

pub fn verify(tree: &CommandTree, command: &str) -> Check {
    let input: Vec<char> = command.chars().collect();
    let with_permissions = parse_as(tree, &input, 0, true);
    if !is_valid(tree, &with_permissions, &input) || !trusted(tree, &with_permissions, &input) {
        return Check::ParseErrors;
    }
    if has_signable_arguments(tree, &with_permissions) {
        return Check::SignatureRequired;
    }
    let without_permissions = parse_as(tree, &input, 0, false);
    if !is_valid(tree, &without_permissions, &input) {
        return Check::PermissionsRequired;
    }
    Check::NoIssues
}

fn is_valid(tree: &CommandTree, parse: &Parse, input: &[char]) -> bool {
    !parse.can_read(input)
        && parse.errors.is_empty()
        && parse
            .last()
            .nodes
            .last()
            .is_some_and(|&(node, _)| tree.node(node).executable)
}

fn visited(tree: &CommandTree, parse: &Parse) -> usize {
    parse
        .chain
        .iter()
        .enumerate()
        .skip(1)
        .find(|(_, context)| context.root == tree.root)
        .map_or(parse.chain.len(), |(i, _)| i)
}

fn has_signable_arguments(tree: &CommandTree, parse: &Parse) -> bool {
    parse.chain[..visited(tree, parse)].iter().any(|context| {
        context.nodes.iter().any(|&(node, _)| {
            matches!(
                &tree.node(node).kind,
                NodeKind::Argument {
                    parser: BrigadierParser::Message,
                    ..
                }
            )
        })
    })
}

fn trusted(tree: &CommandTree, parse: &Parse, input: &[char]) -> bool {
    parse.chain.iter().all(|context| {
        context
            .nodes
            .iter()
            .all(|&(node, range)| match &tree.node(node).kind {
                NodeKind::Argument { parser, .. } => {
                    let text = input.get(range.start..range.end).unwrap_or(&[]);
                    exact(parser, text)
                }
                NodeKind::Root | NodeKind::Literal(_) => true,
            })
    })
}

fn exact(parser: &BrigadierParser, text: &[char]) -> bool {
    match parser {
        BrigadierParser::Bool
        | BrigadierParser::Integer(_)
        | BrigadierParser::Long(_)
        | BrigadierParser::Float(_)
        | BrigadierParser::Double(_)
        | BrigadierParser::String(_)
        | BrigadierParser::Message
        | BrigadierParser::BlockPos
        | BrigadierParser::ColumnPos
        | BrigadierParser::Vec3
        | BrigadierParser::Vec2
        | BrigadierParser::Rotation => true,
        BrigadierParser::Entity(e) => selector_exact(text, |kind| {
            !(e.single && matches!(kind, 'a' | 'e') || e.players_only && matches!(kind, 'e' | 'n'))
        }),
        BrigadierParser::GameProfile => {
            selector_exact(text, |kind| matches!(kind, 'p' | 'a' | 'r'))
        }
        BrigadierParser::ScoreHolder { allows_multiple } => {
            text == ['*'] && *allows_multiple
                || text != ['*']
                    && selector_exact(text, |kind| *allows_multiple || !matches!(kind, 'a' | 'e'))
        }
        BrigadierParser::Objective | BrigadierParser::Team => {
            !text.is_empty() && text.iter().all(|&c| is_unquoted_char(c))
        }
        BrigadierParser::Identifier => identifier(text),
        _ => false,
    }
}

fn selector_exact(text: &[char], allowed: impl Fn(char) -> bool) -> bool {
    match text {
        ['@', kind] => allowed(*kind),
        ['@', ..] => false,
        _ => true,
    }
}

fn identifier(text: &[char]) -> bool {
    let path_char = |c: char| {
        c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '_' | '.' | '-' | '/')
    };
    let (namespace, path) = match text.iter().position(|&c| c == ':') {
        Some(i) => (&text[..i], &text[i + 1..]),
        None => (&[][..], text),
    };
    !text.is_empty()
        && namespace.iter().all(|&c| path_char(c) && c != '/')
        && path.iter().all(|&c| path_char(c))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::play::commands::Node;
    use azalea_protocol::packets::game::c_commands::BrigadierString;

    fn node(kind: NodeKind, children: Vec<usize>, executable: bool, restricted: bool) -> Node {
        Node {
            kind,
            children,
            redirect: None,
            executable,
            restricted,
        }
    }

    fn arg(name: &str, parser: BrigadierParser) -> NodeKind {
        NodeKind::Argument {
            name: name.into(),
            parser,
            provider: crate::play::commands::Provider::Argument,
        }
    }

    fn tree() -> CommandTree {
        let mut nodes = vec![
            node(NodeKind::Root, vec![1, 3, 5, 7, 9], false, false),
            node(NodeKind::Literal("trigger".into()), vec![2], false, false),
            node(
                arg("objective", BrigadierParser::Objective),
                vec![],
                true,
                false,
            ),
            node(NodeKind::Literal("say".into()), vec![4], false, false),
            node(
                arg("message", BrigadierParser::Message),
                vec![],
                true,
                false,
            ),
            node(NodeKind::Literal("op".into()), vec![6], false, true),
            node(
                arg("n", BrigadierParser::String(BrigadierString::SingleWord)),
                vec![],
                true,
                true,
            ),
            node(NodeKind::Literal("give".into()), vec![8], false, false),
            node(arg("item", BrigadierParser::ItemStack), vec![], true, false),
            node(NodeKind::Literal("execute".into()), vec![10], false, false),
            node(NodeKind::Literal("run".into()), vec![], false, false),
        ];
        nodes[10].redirect = Some(0);
        CommandTree { nodes, root: 0 }
    }

    #[test]
    fn a_plain_trigger_goes_straight_out() {
        assert_eq!(verify(&tree(), "trigger vote"), Check::NoIssues);
    }

    #[test]
    fn unknown_or_partial_commands_are_parse_errors() {
        assert_eq!(verify(&tree(), "nope"), Check::ParseErrors);
        assert_eq!(verify(&tree(), "trigger"), Check::ParseErrors);
    }

    #[test]
    fn a_message_argument_needs_a_signature() {
        assert_eq!(verify(&tree(), "say hi there"), Check::SignatureRequired);
    }

    #[test]
    fn a_message_past_a_root_redirect_is_not_visited() {
        assert_eq!(verify(&tree(), "execute run say hi"), Check::NoIssues);
    }

    #[test]
    fn a_restricted_node_asks_for_permissions() {
        assert_eq!(verify(&tree(), "op someone"), Check::PermissionsRequired);
    }

    #[test]
    fn an_approximate_reader_fails_toward_prompting() {
        assert_eq!(verify(&tree(), "give diamond"), Check::ParseErrors);
    }

    #[test]
    fn identifiers_follow_identifier_read() {
        let ok = |s: &str| identifier(&s.chars().collect::<Vec<_>>());
        assert!(ok("minecraft:stone"));
        assert!(ok("stone/block"));
        assert!(!ok("Stone"));
        assert!(!ok("a/b:c"));
    }
}
