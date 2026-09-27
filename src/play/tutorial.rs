use std::sync::Arc;

use crate::gui::toast::{ToastEvent, TutorialSlot};
use crate::session::{Gamemode, SharedMutex};

const MINIMUM_TIME_MOVED: u32 = 40;
const MOVE_HINT_DELAY: u32 = 100;
const LOOK_HINT_DELAY: u32 = 20;
const FIND_TREE_HINT_DELAY: u32 = 6000;
const PUNCH_TREE_HINT_DELAY: u32 = 600;
const OPEN_INVENTORY_HINT_DELAY: u32 = 600;
const CRAFT_PLANKS_HINT_DELAY: u32 = 1200;
const PUNCH_TREE_RESET_LIMIT: u32 = 3;
const TURN_EPSILON: f32 = 0.01;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Step {
    Movement,
    FindTree,
    PunchTree,
    OpenInventory,
    CraftPlanks,
    #[default]
    None,
}

impl Step {
    pub fn serialized_name(self) -> &'static str {
        match self {
            Step::Movement => "movement",
            Step::FindTree => "find_tree",
            Step::PunchTree => "punch_tree",
            Step::OpenInventory => "open_inventory",
            Step::CraftPlanks => "craft_planks",
            Step::None => "none",
        }
    }

    pub fn from_serialized_name(name: &str) -> Step {
        match name {
            "movement" => Step::Movement,
            "find_tree" => Step::FindTree,
            "punch_tree" => Step::PunchTree,
            "open_inventory" => Step::OpenInventory,
            "craft_planks" => Step::CraftPlanks,
            _ => Step::None,
        }
    }
}

pub struct Signals {
    pub survival: bool,
    pub moved: bool,
    pub turned: bool,
    pub looking_at_tree: bool,
    pub destroying: Option<(bool, f32)>,
    pub opened_inventory: bool,
    pub has_logs: bool,
    pub has_planks: bool,
    pub has_tree_items: bool,
}

#[derive(Default)]
pub struct Tutorial {
    step: Step,
    waiting: u32,
    time_moved: u32,
    time_looked: u32,
    move_completed: Option<u32>,
    look_completed: Option<u32>,
    reset_count: u32,
    shown: [bool; TutorialSlot::COUNT],
}

impl Tutorial {
    pub fn new(step: Step) -> Tutorial {
        Tutorial {
            step,
            ..Default::default()
        }
    }

    pub fn step(&self) -> Step {
        self.step
    }

    fn set_step(&mut self, step: Step, out: &mut Vec<ToastEvent>) {
        self.clear(out);
        *self = Tutorial::new(step);
    }

    fn clear(&mut self, out: &mut Vec<ToastEvent>) {
        for (i, shown) in self.shown.iter_mut().enumerate() {
            if std::mem::take(shown) {
                out.push(ToastEvent::TutorialHide {
                    slot: TutorialSlot::ALL[i],
                });
            }
        }
    }

    fn show(&mut self, slot: TutorialSlot, out: &mut Vec<ToastEvent>) {
        if std::mem::replace(&mut self.shown[slot as usize], true) {
            return;
        }
        out.push(ToastEvent::Tutorial { slot });
    }

    fn hide(&mut self, slot: TutorialSlot, out: &mut Vec<ToastEvent>) {
        if !std::mem::take(&mut self.shown[slot as usize]) {
            return;
        }
        out.push(ToastEvent::TutorialHide { slot });
    }

    fn progress(&self, slot: TutorialSlot, progress: f32, out: &mut Vec<ToastEvent>) {
        if !self.shown[slot as usize] {
            return;
        }
        out.push(ToastEvent::TutorialProgress { slot, progress });
    }

    pub fn tick(&mut self, s: &Signals, out: &mut Vec<ToastEvent>) {
        self.waiting += 1;
        if self.moved_this_tick(s) {
            self.time_moved += 1;
        }
        if s.turned {
            self.time_looked += 1;
        }

        if self.step != Step::Movement && self.step != Step::None && !s.survival {
            self.set_step(Step::None, out);
            return;
        }

        match self.step {
            Step::Movement => self.tick_movement(s, out),
            Step::FindTree => self.tick_find_tree(s, out),
            Step::PunchTree => self.tick_punch_tree(s, out),
            Step::OpenInventory => self.tick_open_inventory(s, out),
            Step::CraftPlanks => self.tick_craft_planks(s, out),
            Step::None => {}
        }
    }

    fn moved_this_tick(&self, s: &Signals) -> bool {
        self.step == Step::Movement && s.moved
    }

    fn tick_movement(&mut self, s: &Signals, out: &mut Vec<ToastEvent>) {
        if self.move_completed.is_none() && self.time_moved > MINIMUM_TIME_MOVED {
            self.hide(TutorialSlot::Move, out);
            self.move_completed = Some(self.waiting);
        }
        if self.look_completed.is_none() && self.time_looked > MINIMUM_TIME_MOVED {
            self.hide(TutorialSlot::Look, out);
            self.look_completed = Some(self.waiting);
        }

        if let (Some(_), Some(_)) = (self.move_completed, self.look_completed) {
            let next = if s.survival { Step::FindTree } else { Step::None };
            self.set_step(next, out);
            return;
        }

        self.progress(
            TutorialSlot::Move,
            self.time_moved as f32 / MINIMUM_TIME_MOVED as f32,
            out,
        );
        self.progress(
            TutorialSlot::Look,
            self.time_looked as f32 / MINIMUM_TIME_MOVED as f32,
            out,
        );

        if self.waiting < MOVE_HINT_DELAY {
            return;
        }
        match self.move_completed {
            None => self.show(TutorialSlot::Move, out),
            Some(done)
                if self.waiting - done >= LOOK_HINT_DELAY && self.look_completed.is_none() =>
            {
                self.show(TutorialSlot::Look, out)
            }
            Some(_) => {}
        }
    }

    fn tick_find_tree(&mut self, s: &Signals, out: &mut Vec<ToastEvent>) {
        if self.waiting == 1 && s.has_tree_items {
            self.set_step(Step::CraftPlanks, out);
            return;
        }
        if s.looking_at_tree {
            self.set_step(Step::PunchTree, out);
            return;
        }
        if self.waiting >= FIND_TREE_HINT_DELAY {
            self.show(TutorialSlot::FindTree, out);
        }
    }

    fn tick_punch_tree(&mut self, s: &Signals, out: &mut Vec<ToastEvent>) {
        if self.waiting == 1 && s.has_logs {
            self.set_step(Step::CraftPlanks, out);
            return;
        }

        match s.destroying {
            Some((true, progress)) if progress > 0.0 => {
                if self.shown[TutorialSlot::PunchTree as usize] {
                    self.progress(TutorialSlot::PunchTree, progress, out);
                } else {
                    self.reset_count += 1;
                }
                if progress >= 1.0 {
                    self.set_step(Step::OpenInventory, out);
                    return;
                }
            }
            _ => self.progress(TutorialSlot::PunchTree, 0.0, out),
        }

        if self.waiting >= PUNCH_TREE_HINT_DELAY || self.reset_count > PUNCH_TREE_RESET_LIMIT {
            self.show(TutorialSlot::PunchTree, out);
        }
    }

    fn tick_open_inventory(&mut self, s: &Signals, out: &mut Vec<ToastEvent>) {
        if s.opened_inventory {
            self.set_step(Step::CraftPlanks, out);
            return;
        }
        if self.waiting >= OPEN_INVENTORY_HINT_DELAY {
            self.show(TutorialSlot::OpenInventory, out);
        }
    }

    fn tick_craft_planks(&mut self, s: &Signals, out: &mut Vec<ToastEvent>) {
        if s.has_planks {
            self.set_step(Step::None, out);
            return;
        }
        if self.waiting >= CRAFT_PLANKS_HINT_DELAY {
            self.show(TutorialSlot::CraftPlanks, out);
        }
    }
}

pub fn stop(tutorial: &mut Tutorial, out: &mut Vec<ToastEvent>) {
    tutorial.clear(out);
}

pub fn publish(shared: &Arc<SharedMutex>, events: Vec<ToastEvent>) {
    if events.is_empty() {
        return;
    }
    crate::session::queue_toasts(shared, events);
}

pub fn is_survival(gamemode: Gamemode) -> bool {
    gamemode == Gamemode::Survival
}
