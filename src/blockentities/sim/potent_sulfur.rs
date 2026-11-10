use azalea::entity::particle::{GeyserParticle, Particle};
use azalea_core::position::BlockPos;

use super::TickCtx;
use crate::util::level_events::{Bus, play};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Sulfur {
    eruption_tick: Option<u64>,
}

impl Sulfur {
    pub fn event(&mut self, game_time: u64) {
        self.eruption_tick = Some(game_time);
    }

    pub fn tick(&mut self, ctx: &TickCtx) {
        let eruption_tick = *self.eruption_tick.get_or_insert(ctx.game_time);
        let state = ctx
            .state
            .props
            .get("potent_sulfur_state")
            .map(String::as_str);
        let sound = match state {
            Some("wet" | "dormant") => {
                if ctx.game_time % 20 == 0
                    && let Some(source) = source(ctx)
                {
                    let at = [
                        source.x as f64 + 0.5,
                        source.y as f64 + 0.5,
                        source.z as f64 + 0.5,
                    ];
                    ctx.add_particle(Particle::NoxiousGasCloud, at, [0.0; 3]);
                }
                return;
            }
            Some("erupting") => "block.potent_sulfur.geyser_eruption_active",
            Some("continuous") => "block.potent_sulfur.geyser_continuous_eruption_active",
            _ => return,
        };
        let Some(source) = source(ctx) else {
            return;
        };
        let time = ctx.game_time as i64 - eruption_tick as i64;
        if time % 20 == 0 {
            let water_blocks = source.y - ctx.pos[1] - 1;
            let at = [
                source.x as f64 + 0.5,
                source.y as f64,
                source.z as f64 + 0.5,
            ];
            ctx.add_particle(
                Particle::Geyser(GeyserParticle { water_blocks }),
                at,
                [0.0; 3],
            );
        }
        if time % 40 == 0 {
            let at = [
                source.x as f64 + 0.5,
                source.y as f64 + 0.5,
                source.z as f64 + 0.5,
            ];
            play(sound, Bus::Blocks, at, 1.0, 1.0);
        }
    }
}

fn source(ctx: &TickCtx) -> Option<BlockPos> {
    crate::play::geyser::source_block(ctx.world, BlockPos::new(ctx.pos[0], ctx.pos[1], ctx.pos[2]))
}
