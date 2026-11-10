use azalea::entity::particle::Particle;
use simdnbt::owned::Nbt;

use super::TickCtx;
use crate::blockentities::feed::BlockEntityData;
use crate::renderer::anim::HeldItem;
use crate::util::mth;

const DEFAULT_CONNECTED_PARTICLES_RANGE: f64 = 4.5;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Data {
    pub display_item: HeldItem,
    pub connected_players: Vec<u128>,
    pub connected_particles_range: f64,
}

pub fn decode(tag: &Nbt) -> Data {
    let shared = tag.compound("shared_data");
    Data {
        display_item: shared
            .and_then(|shared| shared.compound("display_item"))
            .map(super::items::stack)
            .unwrap_or_default(),
        connected_players: shared
            .and_then(|shared| shared.list("connected_players"))
            .and_then(|list| list.int_arrays())
            .map(|uuids| uuids.iter().filter_map(|ints| uuid_of(ints)).collect())
            .unwrap_or_default(),
        connected_particles_range: shared
            .and_then(|shared| shared.double("connected_particles_range"))
            .unwrap_or(DEFAULT_CONNECTED_PARTICLES_RANGE),
    }
}

fn uuid_of(ints: &[i32]) -> Option<u128> {
    let [a, b, c, d]: [i32; 4] = ints.try_into().ok()?;
    Some(
        [a, b, c, d]
            .iter()
            .fold(0u128, |acc, &i| (acc << 32) | u128::from(i as u32)),
    )
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vault {
    previous_spin: f32,
    current_spin: f32,
}

impl Vault {
    const ROTATION_SPEED: f32 = 10.0;

    pub fn tick(&mut self, ctx: &TickCtx) {
        self.update_display_item_spin();
        let BlockEntityData::Vault(data) = ctx.data else {
            return;
        };
        if ctx.game_time % 20 == 0 {
            connection_particles(ctx, data);
        }
        let active = !data.display_item.is_empty();
        let [x, y, z] = ctx.pos.map(f64::from);
        let at = ctx.random(|r| {
            (r.next_f32() <= 0.5).then(|| {
                [
                    x + super::next_double(r, 0.1, 0.9),
                    y + super::next_double(r, 0.25, 0.75),
                    z + super::next_double(r, 0.1, 0.9),
                ]
            })
        });
        if let Some(at) = at {
            ctx.add_particle(Particle::Smoke, at, [0.0; 3]);
            if active {
                let flame = if ctx.state.prop("ominous") == "true" {
                    Particle::SoulFireFlame
                } else {
                    Particle::SmallFlame
                };
                ctx.add_particle(flame, at, [0.0; 3]);
            }
        }
        if active {
            super::idle_sound(ctx, "block.vault.ambient");
        }
    }

    fn update_display_item_spin(&mut self) {
        self.previous_spin = self.current_spin;
        self.current_spin = mth::wrap_degrees(self.current_spin + Self::ROTATION_SPEED);
    }

    pub fn angle(&self, partial: f32) -> f32 {
        mth::rot_lerp(partial, self.previous_spin, self.current_spin)
    }
}

fn connection_particles(ctx: &TickCtx, data: &Data) {
    if data.connected_players.is_empty() {
        return;
    }
    let step = match ctx.state.prop("facing") {
        "south" => [0.0, 1.0],
        "west" => [-1.0, 0.0],
        "east" => [1.0, 0.0],
        _ => [0.0, -1.0],
    };
    let [x, y, z] = ctx.pos.map(f64::from);
    let keyhole = [x + 0.5 + step[0] * 0.5, y + 1.75, z + 0.5 + step[1] * 0.5];
    let range_sq = data.connected_particles_range * data.connected_particles_range;
    for &uuid in &data.connected_players {
        let Some(player) = ctx.player_by_uuid(uuid) else {
            continue;
        };
        let cell = player.feet.map(f64::floor);
        let dist_sq = (cell[0] - x).powi(2) + (cell[1] - y).powi(2) + (cell[2] - z).powi(2);
        if dist_sq > range_sq {
            continue;
        }
        let target = [
            player.feet[0],
            player.feet[1] + f64::from(player.height / 2.0),
            player.feet[2],
        ];
        let direction = [0, 1, 2].map(|i| target[i] - keyhole[i]);
        let directions = ctx.random(|r| {
            let count = 2 + r.next_int(4);
            (0..count)
                .map(|_| direction.map(|d| d + f64::from((r.next_f32() - 0.5) * 1.0)))
                .collect::<Vec<_>>()
        });
        for velocity in directions {
            ctx.add_particle(Particle::VaultConnection, keyhole, velocity);
        }
    }
}

#[cfg(test)]
mod tests {
    use simdnbt::owned::NbtTag;

    use super::*;
    use crate::blockentities::sim::test_nbt::{compound, item, root};

    #[test]
    fn the_display_item_comes_from_shared_data() {
        let shared = compound(vec![(
            "display_item",
            NbtTag::Compound(compound(item("minecraft:diamond", 2))),
        )]);
        let data = decode(&root(vec![("shared_data", NbtTag::Compound(shared))]));
        assert_eq!(
            (data.display_item.id, data.display_item.count),
            ("diamond", 2)
        );
        assert!(decode(&root(vec![])).display_item.is_empty());
    }

    #[test]
    fn the_spin_turns_ten_degrees_a_tick_the_short_way_round() {
        let mut vault = Vault {
            previous_spin: 0.0,
            current_spin: 175.0,
        };
        vault.update_display_item_spin();
        assert_eq!(vault.current_spin, -175.0);
        assert!((vault.angle(0.5) - 180.0).abs() < 1e-3 || (vault.angle(0.5) + 180.0).abs() < 1e-3);
    }
}
