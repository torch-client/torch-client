use std::collections::HashMap;
use std::sync::Arc;

use azalea::local_player::WorldHolder;
use azalea_protocol::packets::game::ClientboundGamePacket;
use azalea_protocol::packets::game::c_update_advancements::{
    AdvancementHolder, AdvancementProgress, FrameType,
};
use azalea_registry::identifier::Identifier;

use crate::client::chat_text::to_spans_styled;
use crate::gui::toast::{AdvancementFrame, ToastEvent};
use crate::session::{SessionState, SharedMutex};
use crate::text::{Span, Style};

const WINDOW_TITLE: u32 = 0x404040;

#[derive(Clone, Default)]
pub struct AdvancementTree {
    pub nodes: Vec<Node>,
    pub generation: u32,
}

#[derive(Clone)]
pub struct Node {
    pub id: Identifier,
    pub parent: Option<u32>,
    pub display: Option<Arc<Display>>,
    requirements: Arc<[Box<[Box<str>]>]>,
    pub completed: Option<u16>,
}

pub struct Display {
    pub title: Vec<Span>,
    pub window_title: Option<Vec<Span>>,
    pub description: Vec<Span>,
    pub icon: String,
    pub frame: AdvancementFrame,
    pub show_toast: bool,
    pub hidden: bool,
    pub background: Option<Identifier>,
    pub x: f32,
    pub y: f32,
}

impl Node {
    pub fn groups(&self) -> u16 {
        self.requirements.len() as u16
    }

    pub fn is_done(&self) -> bool {
        self.groups() > 0 && self.completed == Some(self.groups())
    }

    pub fn progress(&self) -> Option<(u16, u16)> {
        let has_criteria = self.requirements.iter().any(|g| !g.is_empty());
        self.completed
            .filter(|_| has_criteria)
            .map(|c| (c, self.groups()))
    }

    pub fn percent(&self) -> f32 {
        self.progress().map_or(0.0, |(c, t)| c as f32 / t as f32)
    }

    fn count(&self, progress: &AdvancementProgress) -> u16 {
        self.requirements
            .iter()
            .filter(|group| {
                group
                    .iter()
                    .any(|name| progress.get(&**name).is_some_and(|c| c.date.is_some()))
            })
            .count() as u16
    }
}

struct Added {
    id: Identifier,
    parent: Option<Identifier>,
    display: Option<Arc<Display>>,
    requirements: Arc<[Box<[Box<str>]>]>,
}

fn convert(
    holder: &AdvancementHolder,
    registries: Option<&azalea_core::registry_holder::RegistryHolder>,
) -> Added {
    let a = &holder.value;
    let display = a.display.as_ref().map(|d| {
        let frame = match d.frame {
            FrameType::Task => AdvancementFrame::Task,
            FrameType::Challenge => AdvancementFrame::Challenge,
            FrameType::Goal => AdvancementFrame::Goal,
        };
        Arc::new(Display {
            title: to_spans_styled(&d.title, Style::colored(0xFFFFFF)),
            window_title: a
                .parent_id
                .is_none()
                .then(|| to_spans_styled(&d.title, Style::colored(WINDOW_TITLE))),
            description: to_spans_styled(&d.description, Style::colored(frame.chat_color())),
            icon: crate::play::inventory_bridge::slot_stack(&d.icon, registries, false)
                .model_key()
                .into_owned(),
            frame,
            show_toast: d.show_toast,
            hidden: d.hidden,
            background: d.background.clone(),
            x: d.x,
            y: d.y,
        })
    });
    Added {
        id: holder.id.clone(),
        parent: a.parent_id.clone(),
        display,
        requirements: a
            .requirements
            .iter()
            .map(|g| g.iter().map(|n| n.as_str().into()).collect())
            .collect(),
    }
}

pub(crate) fn packet(world: &WorldHolder, shared: &SharedMutex, packet: &ClientboundGamePacket) {
    match packet {
        ClientboundGamePacket::UpdateAdvancements(p) => {
            let added: Vec<Added> = if p.added.is_empty() {
                Vec::new()
            } else {
                let world = world.shared.read();
                p.added
                    .iter()
                    .map(|h| convert(h, Some(&world.registries)))
                    .collect()
            };
            let mut s = shared.lock().unwrap();
            let toasts = update(&mut s.session, p.reset, &p.removed, added, &p.progress);
            if p.show_advancements && !toasts.is_empty() {
                crate::session::push_toasts(&mut s.session, toasts);
            }
        }
        ClientboundGamePacket::SelectAdvancementsTab(p) => {
            let mut s = shared.lock().unwrap();
            let session = &mut s.session;
            session.advancement_tab = p
                .tab
                .as_ref()
                .and_then(|id| session.advancement_ids.get(id).copied());
        }
        ClientboundGamePacket::StartConfiguration(_) => {
            let mut s = shared.lock().unwrap();
            let session = &mut s.session;
            let tree = Arc::make_mut(&mut session.advancements);
            tree.nodes.clear();
            tree.generation = tree.generation.wrapping_add(1);
            session.advancement_ids.clear();
            session.advancement_tab = None;
        }
        _ => {}
    }
}

fn update<'a>(
    session: &mut SessionState,
    reset: bool,
    removed: &[Identifier],
    added: Vec<Added>,
    progress: impl IntoIterator<Item = (&'a Identifier, &'a AdvancementProgress)>,
) -> Vec<ToastEvent> {
    let reshaped = reset || !removed.is_empty() || !added.is_empty();
    let selected = session
        .advancement_tab
        .filter(|_| reshaped)
        .and_then(|i| session.advancements.nodes.get(i as usize))
        .map(|n| n.id.clone());
    let tree = Arc::make_mut(&mut session.advancements);
    let ids = &mut session.advancement_ids;

    if reset {
        tree.nodes.clear();
        ids.clear();
    }
    if !removed.is_empty() {
        remove(tree, ids, removed);
    }
    add_all(tree, ids, added);
    if reshaped {
        tree.generation = tree.generation.wrapping_add(1);
        session.advancement_tab = selected.and_then(|id| ids.get(&id).copied());
    }

    let mut toasts = Vec::new();
    for (id, p) in progress {
        let Some(&i) = ids.get(id) else {
            continue;
        };
        let node = &mut tree.nodes[i as usize];
        node.completed = Some(node.count(p));
        if !reset
            && node.is_done()
            && let Some(d) = node.display.as_ref().filter(|d| d.show_toast)
        {
            toasts.push(ToastEvent::Advancement {
                title: d.title.clone(),
                icon: d.icon.clone(),
                frame: d.frame,
            });
        }
    }
    toasts
}

fn remove(tree: &mut AdvancementTree, ids: &mut HashMap<Identifier, u32>, removed: &[Identifier]) {
    let mut dead = vec![false; tree.nodes.len()];
    for id in removed {
        if let Some(&i) = ids.get(id) {
            dead[i as usize] = true;
        }
    }
    for i in 0..tree.nodes.len() {
        if let Some(parent) = tree.nodes[i].parent
            && dead[parent as usize]
        {
            dead[i] = true;
        }
    }
    let mut remap = vec![u32::MAX; tree.nodes.len()];
    let mut next = 0u32;
    for (i, d) in dead.iter().enumerate() {
        if !d {
            remap[i] = next;
            next += 1;
        }
    }
    let mut i = 0;
    tree.nodes.retain(|_| {
        i += 1;
        !dead[i - 1]
    });
    ids.clear();
    for (i, node) in tree.nodes.iter_mut().enumerate() {
        node.parent = node.parent.map(|p| remap[p as usize]);
        ids.insert(node.id.clone(), i as u32);
    }
}

fn add_all(
    tree: &mut AdvancementTree,
    ids: &mut HashMap<Identifier, u32>,
    mut pending: Vec<Added>,
) {
    let mut deferred = Vec::new();
    while !pending.is_empty() {
        let before = pending.len();
        for a in pending.drain(..) {
            let parent = match &a.parent {
                None => None,
                Some(p) => match ids.get(p) {
                    Some(&i) => Some(i),
                    None => {
                        deferred.push(a);
                        continue;
                    }
                },
            };
            let slot = ids.get(&a.id).copied();
            let node = Node {
                id: a.id,
                parent,
                display: a.display,
                requirements: a.requirements,
                completed: None,
            };
            match slot {
                Some(i) => tree.nodes[i as usize] = node,
                None => {
                    ids.insert(node.id.clone(), tree.nodes.len() as u32);
                    tree.nodes.push(node);
                }
            }
        }
        std::mem::swap(&mut pending, &mut deferred);
        if pending.len() == before {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use azalea_protocol::packets::game::c_update_advancements::CriterionProgress;

    fn added(id: &str, parent: Option<&str>, groups: &[&[&str]]) -> Added {
        Added {
            id: Identifier::new(id),
            parent: parent.map(Identifier::new),
            display: None,
            requirements: groups
                .iter()
                .map(|g| g.iter().map(|n| (*n).into()).collect())
                .collect(),
        }
    }

    fn progress(id: &str, dated: &[&str]) -> Vec<(Identifier, AdvancementProgress)> {
        let p: AdvancementProgress = dated
            .iter()
            .map(|n| (n.to_string(), CriterionProgress { date: Some(1) }))
            .collect();
        [(Identifier::new(id), p)].into_iter().collect()
    }

    #[test]
    fn done_needs_every_group() {
        let mut s = SessionState::default();
        update(
            &mut s,
            true,
            &[],
            vec![added("a", None, &[&["x", "y"], &["z"]])],
            std::iter::empty(),
        );
        update(
            &mut s,
            false,
            &[],
            vec![],
            progress("a", &["y"]).iter().map(|(i, p)| (i, p)),
        );
        let n = &s.advancements.nodes[0];
        assert_eq!(n.progress(), Some((1, 2)));
        assert!(!n.is_done());
        update(
            &mut s,
            false,
            &[],
            vec![],
            progress("a", &["y", "z"]).iter().map(|(i, p)| (i, p)),
        );
        assert!(s.advancements.nodes[0].is_done());
    }

    #[test]
    fn no_requirements_is_never_done() {
        let mut s = SessionState::default();
        update(
            &mut s,
            true,
            &[],
            vec![added("a", None, &[])],
            progress("a", &[]).iter().map(|(i, p)| (i, p)),
        );
        let n = &s.advancements.nodes[0];
        assert!(!n.is_done());
        assert_eq!(n.percent(), 0.0);
        assert_eq!(n.progress(), None);
    }

    #[test]
    fn child_before_parent_lands_after_it() {
        let mut s = SessionState::default();
        let list = vec![added("c", Some("r"), &[]), added("r", None, &[])];
        update(&mut s, true, &[], list, std::iter::empty());
        let n = &s.advancements.nodes;
        assert_eq!(n[0].id, Identifier::new("r"));
        assert_eq!(n[1].parent, Some(0));
    }

    #[test]
    fn remove_cascades_and_remaps() {
        let mut s = SessionState::default();
        let list = vec![
            added("r1", None, &[]),
            added("a", Some("r1"), &[]),
            added("b", Some("a"), &[]),
            added("r2", None, &[]),
            added("c", Some("r2"), &[]),
        ];
        update(&mut s, true, &[], list, std::iter::empty());
        s.advancement_tab = Some(3);
        let generation = s.advancements.generation;
        update(
            &mut s,
            false,
            &[Identifier::new("a")],
            vec![],
            std::iter::empty(),
        );
        let n = &s.advancements.nodes;
        assert_eq!(n.len(), 3);
        assert_eq!(n[2].parent, Some(1));
        assert_eq!(s.advancement_ids[&Identifier::new("c")], 2);
        assert_eq!(s.advancement_tab, Some(1));
        assert_ne!(s.advancements.generation, generation);
    }
}
