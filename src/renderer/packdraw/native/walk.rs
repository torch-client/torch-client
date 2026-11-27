use crate::shaderpack::native::api::{Group, ScreenOutput};
use crate::shaderpack::native::build::{Built, BuiltGroup, BuiltPass, BuiltTarget};
use crate::shaderpack::native::manifest::Anchor;

pub(super) fn bit(bits: u32, t: usize) -> usize {
    ((bits >> t) & 1) as usize
}

pub(super) fn with_bit(bits: u32, t: usize, which: usize) -> u32 {
    (bits & !(1 << t)) | ((which as u32) << t)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Walk {
    pub(super) start: u32,
    pub(super) front: u32,
}

impl Walk {
    pub(super) fn new(start: u32) -> Walk {
        Walk {
            start,
            front: start,
        }
    }

    pub(super) fn current(&self, t: usize) -> usize {
        bit(self.front, t)
    }

    pub(super) fn previous(&self, t: usize) -> usize {
        bit(self.start, t)
    }

    pub(super) fn clear(&mut self, targets: &[BuiltTarget], t: usize) -> usize {
        let target = &targets[t];
        let which = if !target.double() {
            0
        } else if target.history {
            1 - bit(self.start, t)
        } else {
            bit(self.front, t)
        };
        self.front = with_bit(self.front, t, which);
        which
    }

    pub(super) fn pass_target(&self, targets: &[BuiltTarget], t: usize) -> usize {
        let target = &targets[t];
        if !target.double() {
            0
        } else if target.history {
            1 - bit(self.start, t)
        } else {
            1 - bit(self.front, t)
        }
    }

    pub(super) fn group_target(&self, targets: &[BuiltTarget], t: usize) -> usize {
        let target = &targets[t];
        if !target.double() {
            0
        } else if target.history {
            1 - bit(self.start, t)
        } else {
            bit(self.front, t)
        }
    }

    pub(super) fn wrote(&mut self, t: usize, which: usize) {
        self.front = with_bit(self.front, t, which);
    }

    pub(super) fn clears(&mut self, targets: &[BuiltTarget]) -> Vec<(usize, usize)> {
        (0..targets.len())
            .filter(|&t| targets[t].clear.is_some())
            .map(|t| (t, self.clear(targets, t)))
            .collect()
    }

    pub(super) fn pass_writes(
        &self,
        targets: &[BuiltTarget],
        pass: &BuiltPass,
    ) -> Vec<(usize, usize)> {
        match &pass.output {
            ScreenOutput::Screen => Vec::new(),
            ScreenOutput::Targets(writes) => writes
                .iter()
                .map(|&t| (t, self.pass_target(targets, t)))
                .collect(),
        }
    }

    pub(super) fn group_writes(
        &self,
        targets: &[BuiltTarget],
        group: &BuiltGroup,
    ) -> Vec<(usize, usize)> {
        group
            .writes
            .iter()
            .map(|&t| (t, self.group_target(targets, t)))
            .collect()
    }

    pub(super) fn commit(&mut self, writes: &[(usize, usize)]) {
        for &(t, which) in writes {
            self.wrote(t, which);
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Step {
    Pass(usize),
    Shadow,
    Opaque(Option<usize>),
    Translucent(Option<usize>),
}

pub(super) fn steps(built: &Built) -> Vec<Step> {
    if built.geometry.is_empty() {
        return (0..built.passes.len()).map(Step::Pass).collect();
    }
    let group = |kind: Group| built.geometry.iter().position(|g| g.group == kind);
    let mut steps: Vec<Step> = anchored(built, Anchor::Begin).collect();
    if built.shadow.is_some() {
        steps.push(Step::Shadow);
    }
    steps.push(Step::Opaque(group(Group::Opaque)));
    steps.extend(anchored(built, Anchor::Deferred));
    steps.push(Step::Translucent(group(Group::Translucent)));
    steps.extend(anchored(built, Anchor::Composite));
    steps.extend(anchored(built, Anchor::Final));
    steps
}

fn anchored(built: &Built, anchor: Anchor) -> impl Iterator<Item = Step> + '_ {
    built
        .passes
        .iter()
        .enumerate()
        .filter(move |(_, pass)| pass.anchor == anchor)
        .map(|(index, _)| Step::Pass(index))
}

pub(super) struct Walked {
    pub(super) groups: Vec<Walk>,
    pub(super) shadow: Walk,
    pub(super) end: u32,
}

pub(super) fn walk_frame(built: &Built, start: u32) -> Walked {
    let mut walk = Walk::new(start);
    walk.clears(&built.targets);
    let mut groups = vec![walk; built.geometry.len()];
    let mut shadow = walk;
    for step in steps(built) {
        match step {
            Step::Pass(index) => {
                let writes = walk.pass_writes(&built.targets, &built.passes[index]);
                walk.commit(&writes);
            }
            Step::Shadow => shadow = walk,
            Step::Opaque(Some(index)) | Step::Translucent(Some(index)) => {
                groups[index] = walk;
                let writes = walk.group_writes(&built.targets, &built.geometry[index]);
                walk.commit(&writes);
            }
            Step::Opaque(None) | Step::Translucent(None) => {}
        }
    }
    Walked {
        groups,
        shadow,
        end: walk.front,
    }
}
