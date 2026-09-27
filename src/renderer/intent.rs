use bevy::prelude::*;

use crate::renderer::systems::Shared;

#[derive(Resource, Default)]
pub struct Intent {
    move_flags: u8,
    attack_held: bool,
    use_held: bool,
    attack_clicked: bool,
    use_clicked: bool,
    pick_clicked: bool,
    pick_include_data: bool,
}

impl Intent {
    pub fn walk(&mut self, flags: u8) {
        self.move_flags |= flags;
    }

    pub fn hold(&mut self, attack: bool, use_item: bool) {
        self.attack_held |= attack;
        self.use_held |= use_item;
    }

    pub fn click(&mut self, attack: bool, use_item: bool) {
        self.attack_clicked |= attack;
        self.use_clicked |= use_item;
    }

    pub fn pick(&mut self, include_data: bool) {
        self.pick_clicked = true;
        self.pick_include_data |= include_data;
    }
}

pub(crate) fn publish(mut intent: ResMut<Intent>, shared: Res<Shared>) {
    let mut s = shared.0.lock().unwrap();
    s.move_flags = intent.move_flags;
    if crate::modules::hooks::shapes_movement() {
        s.move_flags = crate::modules::hooks::move_flags(&s, s.move_flags);
    }
    s.session.attack_held = intent.attack_held;
    s.session.use_held = intent.use_held;
    s.session.attack_clicked |= intent.attack_clicked;
    s.session.use_clicked |= intent.use_clicked;
    if intent.pick_clicked {
        s.session.pick_clicked = true;
        s.session.pick_include_data = intent.pick_include_data;
    }
    drop(s);
    *intent = Intent::default();
}
