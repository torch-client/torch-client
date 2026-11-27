use azalea_protocol::packets::game::c_commands::{BrigadierNodeStub, BrigadierParser, NodeType};

use super::parse::read_while;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Provider {
    Argument,
    AskServer,
    AvailableSounds,
    SummonableEntities,
}

#[derive(Clone, Debug)]
pub enum NodeKind {
    Root,
    Literal(String),
    Argument {
        name: String,
        parser: BrigadierParser,
        provider: Provider,
    },
}

#[derive(Clone, Debug)]
pub struct Node {
    pub kind: NodeKind,
    pub children: Vec<usize>,
    pub redirect: Option<usize>,
    pub executable: bool,
    pub restricted: bool,
}

impl Node {
    pub(super) fn is_literal(&self) -> bool {
        matches!(self.kind, NodeKind::Literal(_))
    }

    pub(super) fn usage_text(&self) -> String {
        match &self.kind {
            NodeKind::Root => String::new(),
            NodeKind::Literal(name) => name.clone(),
            NodeKind::Argument { name, .. } => format!("<{name}>"),
        }
    }
}

#[derive(Clone, Debug)]
pub struct CommandTree {
    pub nodes: Vec<Node>,
    pub root: usize,
}

impl CommandTree {
    pub fn from_stubs(entries: &[BrigadierNodeStub], root_index: u32) -> CommandTree {
        let n = entries.len();
        let nodes: Vec<Node> = entries
            .iter()
            .map(|stub| Node {
                kind: match &stub.node_type {
                    NodeType::Root => NodeKind::Root,
                    NodeType::Literal { name } => NodeKind::Literal(name.clone()),
                    NodeType::Argument {
                        name,
                        parser,
                        suggestions_type,
                    } => NodeKind::Argument {
                        name: name.clone(),
                        parser: parser.clone(),
                        provider: match suggestions_type {
                            None => Provider::Argument,
                            Some(id) => match (id.namespace(), id.path()) {
                                ("minecraft", "available_sounds") => Provider::AvailableSounds,
                                ("minecraft", "summonable_entities") => {
                                    Provider::SummonableEntities
                                }
                                _ => Provider::AskServer,
                            },
                        },
                    },
                },
                children: stub
                    .children
                    .iter()
                    .map(|c| *c as usize)
                    .filter(|c| *c < n)
                    .collect(),
                redirect: stub.redirect_node.map(|r| r as usize).filter(|r| *r < n),
                executable: stub.is_executable,
                restricted: stub.is_restricted,
            })
            .collect();
        let root = (root_index as usize).min(n.saturating_sub(1));
        CommandTree { nodes, root }
    }

    pub(super) fn node(&self, i: usize) -> &Node {
        &self.nodes[i]
    }

    pub(super) fn relevant_children(&self, i: usize, input: &[char], cursor: usize) -> Vec<usize> {
        let node = self.node(i);
        let mut end = cursor;
        let word = read_while(input, &mut end, |c| c != ' ');
        let literal = node
            .children
            .iter()
            .copied()
            .find(|&c| match &self.node(c).kind {
                NodeKind::Literal(name) => name.chars().eq(word.iter().copied()),
                _ => false,
            });
        if let Some(c) = literal {
            return vec![c];
        }
        node.children
            .iter()
            .copied()
            .filter(|&c| !self.node(c).is_literal())
            .collect()
    }
}
