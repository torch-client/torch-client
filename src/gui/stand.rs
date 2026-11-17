use std::f32::consts::PI;
use std::sync::{Arc, OnceLock};

use bevy::prelude::*;

use crate::entities::geom::{PartPose, PartState};
use crate::gui::atlas::{PUPPET_STAND_ARMOR_SLOT, PUPPET_STAND_SLOT, Region};
use crate::gui::painter::Painter;
use crate::gui::puppet::{self, ARMOR_SHEET_H, Part, Placed, SHEET_PX, Sheet, Shell};
use crate::renderer::player_model::{BodyDraw, MODEL_Y_OFFSET, model_transform};
use crate::session::SlotStack;

const TEXTURE: &str = "entity/armorstand/armorstand";

const BOX: [f32; 4] = [121.0, 20.0, 161.0, 80.0];

const SCALE: f32 = 25.0;

const TRANSLATION: Vec3 = Vec3::new(0.0, 1.0, 0.0);

const ANGLE_X: f32 = 0.436_332_32;

const BODY_ROT: f32 = 210.0;

const POSE: [[f32; 3]; 6] = [
    [0.0, 0.0, 0.0],
    [0.0, 0.0, 0.0],
    [-15.0, 0.0, 10.0],
    [-10.0, 0.0, -10.0],
    [1.0, 0.0, 1.0],
    [-1.0, 0.0, -1.0],
];

const PART_NAMES: [&str; 6] = [
    "head",
    "body",
    "right_arm",
    "left_arm",
    "right_leg",
    "left_leg",
];

const BODY_NODES: [&[&str]; 6] = [
    &["head"],
    &[
        "right_body_stick",
        "left_body_stick",
        "shoulder_stick",
        "body",
    ],
    &["right_arm"],
    &["left_arm"],
    &["right_leg"],
    &["left_leg"],
];

#[derive(Default)]
pub struct Stand {
    sheet: Option<Option<Arc<Vec<u8>>>>,
    seen: Option<(&'static str, Option<Arc<azalea_inventory::ItemStack>>)>,
    piece: Option<(usize, Arc<Vec<u8>>)>,
    assets_seen: u32,
}

impl Stand {
    pub fn draw(&mut self, p: &mut Painter, left: f32, top: f32, result: &SlotStack) {
        self.sync(result);
        let Some(Some(sheet)) = &self.sheet else {
            return;
        };
        puppet::show(PUPPET_STAND_SLOT, sheet);
        if let Some((_, pixels)) = &self.piece {
            puppet::show(PUPPET_STAND_ARMOR_SLOT, pixels);
        }
        let Some(stand) = p.atlas.server_icon(PUPPET_STAND_SLOT) else {
            return;
        };
        let piece: Option<(usize, Region)> = self.piece.as_ref().and_then(|(slot, _)| {
            let cell = p.atlas.server_icon(PUPPET_STAND_ARMOR_SLOT)?;
            Some((
                *slot,
                Region {
                    h: ARMOR_SHEET_H,
                    ..cell
                },
            ))
        });

        let view = Quat::from_rotation_x(ANGLE_X) * Quat::from_rotation_z(PI);
        let outer = view * Quat::from_rotation_y((180.0 - BODY_ROT).to_radians());
        let mirror = Quat::from_rotation_z(PI);
        let at = TRANSLATION + outer * (mirror * Vec3::new(0.0, MODEL_Y_OFFSET, 0.0));
        let [x0, y0, x1, y1] = BOX;
        let placed = Placed {
            parts: [(outer * mirror, at); 6],
            k: 1.0 / 16.0,
            centre: Vec2::new(left + (x0 + x1) / 2.0, top + (y0 + y1) / 2.0),
            size: SCALE,
        };
        let region_of = |sheet: Sheet| match sheet {
            Sheet::Skin => Some(stand),
            Sheet::Overlay(_) => None,
            Sheet::Armor(slot) => piece
                .filter(|(worn, _)| *worn == slot)
                .map(|(_, region)| region),
        };
        puppet::paint(p, geometry(), &placed, region_of, true, BodyDraw::Shown);
    }

    fn sync(&mut self, result: &SlotStack) {
        let generation = crate::renderer::reload::generation();
        if self.assets_seen != generation {
            self.assets_seen = generation;
            self.sheet = None;
            self.seen = None;
        }
        if self.sheet.is_none() {
            let pixels = crate::entities::decode_entity_texture(TEXTURE)
                .map(|image| Arc::new(puppet::fit(image, SHEET_PX, SHEET_PX).into_raw()));
            self.sheet = Some(pixels);
        }
        let changed = self
            .seen
            .as_ref()
            .is_none_or(|(item, raw)| *item != result.item || *raw != result.raw);
        if changed {
            self.seen = Some((result.item, result.raw.clone()));
            self.piece = piece(result);
        }
    }
}

fn piece(result: &SlotStack) -> Option<(usize, Arc<Vec<u8>>)> {
    use std::str::FromStr;

    use azalea_inventory::components::{EquipmentSlot, Equippable};
    use azalea_inventory::default_components::get_default_component;
    use azalea_registry::builtin::ItemKind;

    if result.item.is_empty() {
        return None;
    }
    let (slot, has_asset) = match &result.raw {
        Some(raw) => raw
            .as_present()?
            .get_unless_removed::<Equippable>()
            .map(|e| (e.slot, e.asset_id.is_some()))?,
        None => ItemKind::from_str(result.item)
            .ok()
            .and_then(get_default_component::<Equippable>)
            .map(|e| (e.slot, e.asset_id.is_some()))?,
    };
    let slot = match slot {
        EquipmentSlot::Head => 0,
        EquipmentSlot::Chest => 1,
        EquipmentSlot::Legs => 2,
        EquipmentSlot::Feet => 3,
        _ => return None,
    };
    if !has_asset {
        return None;
    }
    let trim = result.raw.as_deref().and_then(|raw| {
        let registries = crate::items::tips::registries();
        crate::entities::render::humanoid::trim::resolve(raw, result.item, registries.as_deref())
    });
    Some((slot, puppet::armor_cell(slot, result.item, trim)?))
}

fn geometry() -> &'static [Part; 6] {
    static GEOMETRY: OnceLock<[Part; 6]> = OnceLock::new();
    GEOMETRY.get_or_init(build)
}

fn build() -> [Part; 6] {
    use crate::entities::models::humanoid::armor_stand::armor_stand_layer;
    use crate::entities::render::humanoid::armor::ARMOR_STAND;

    let layer = armor_stand_layer();
    let sheet = [layer.tex_w as f32, layer.tex_h as f32];
    let mut parts: [Part; 6] = std::array::from_fn(|_| Part {
        centre: Vec3::ZERO,
        shells: Vec::new(),
    });
    for (part, names) in BODY_NODES.iter().enumerate() {
        for &name in *names {
            let Some(node) = layer.mesh.find(name) else {
                continue;
            };
            let pose = posed(node.pose(), part);
            for cube in node.cubes() {
                if name == PART_NAMES[part] {
                    let centre = Vec3::from(cube.origin) + Vec3::from(cube.size) / 2.0;
                    parts[part].centre = pose.transform_point(centre);
                }
                parts[part].shells.push(Shell {
                    sheet: Sheet::Skin,
                    faces: puppet::faces(cube, sheet, &pose),
                });
            }
        }
    }
    puppet::push_armor(&mut parts, &ARMOR_STAND, |mesh, part| {
        posed(
            mesh.find(PART_NAMES[part])
                .map_or(PartPose::ZERO, |node| node.pose()),
            part,
        )
    });
    parts
}

fn posed(rest: PartPose, part: usize) -> Transform {
    let mut state = PartState::from(rest);
    let [x, y, z] = POSE[part];
    state.set_rotation(x.to_radians(), y.to_radians(), z.to_radians());
    model_transform(&state)
}
