use std::collections::HashMap;

use azalea::block::BlockState;
use azalea_registry::builtin::BlockKind;

use super::engine::{LightEngine, LightWorld};
use super::props;
use super::sources::SkySources;
use super::storage::Layer;
use super::{ColumnPos, SectionPos};

struct FlatWorld {
    surface: i32,
    extra: HashMap<(i32, i32, i32), BlockState>,
}

const MIN_Y: i32 = 0;
const HEIGHT: i32 = 64;
const SECTIONS: i32 = HEIGHT / 16;

impl FlatWorld {
    fn new(surface: i32) -> Self {
        Self {
            surface,
            extra: HashMap::new(),
        }
    }

    fn set(&mut self, x: i32, y: i32, z: i32, kind: BlockKind) {
        self.extra.insert((x, y, z), BlockState::from(kind));
    }
}

impl LightWorld for FlatWorld {
    fn block_state(&self, x: i32, y: i32, z: i32) -> BlockState {
        if let Some(state) = self.extra.get(&(x, y, z)) {
            return *state;
        }
        if y < MIN_Y || y >= MIN_Y + HEIGHT {
            return BlockState::AIR;
        }
        if y < self.surface {
            BlockState::from(BlockKind::Stone)
        } else {
            BlockState::AIR
        }
    }
}

struct Harness {
    block: LightEngine,
    sky: LightEngine,
    sources: SkySources,
}

impl Harness {
    fn light(world: &FlatWorld, columns: &[ColumnPos]) -> Self {
        atlas_once();
        let mut h = Harness {
            block: LightEngine::new(Layer::Block),
            sky: LightEngine::new(Layer::Sky),
            sources: SkySources::new(MIN_Y),
        };
        for col in columns {
            for sy in -1..=SECTIONS {
                let sec = SectionPos {
                    x: col.x,
                    y: sy,
                    z: col.z,
                };
                let base = sec.min_block_y();
                let empty = (0..16).all(|ly| {
                    (0..16).all(|lx| {
                        (0..16).all(|lz| {
                            world
                                .block_state(col.x * 16 + lx, base + ly, col.z * 16 + lz)
                                .is_air()
                        })
                    })
                });
                h.block.update_section_status(sec, empty);
                h.sky.update_section_status(sec, empty);
            }
        }
        for col in columns {
            let mut sources = h.sources.new_chunk_sources();
            sources.fill_from(MIN_Y + HEIGHT, |lx, y, lz| {
                props::of(world.block_state(col.x * 16 + lx as i32, y, col.z * 16 + lz as i32))
            });
            h.sources.insert(*col, sources);
        }
        for col in columns {
            h.sky.set_sky_light_enabled(&h.sources, *col);
            h.sky.propagate_sky_light_sources(&h.sources, *col);
            h.block
                .propagate_block_light_sources(world, *col, -1, SECTIONS);
        }
        h.block.run_light_updates(world, None);
        h.sky.run_light_updates(world, Some(&h.sources));
        h
    }

    fn sky_at(&self, x: i32, y: i32, z: i32) -> u8 {
        self.sky.storage.get_stored_level(x, y, z)
    }

    fn block_at(&self, x: i32, y: i32, z: i32) -> u8 {
        self.block.storage.get_stored_level(x, y, z)
    }
}

fn atlas_once() {
    crate::blocks::tests::atlas_once();
}

fn one_chunk() -> Vec<ColumnPos> {
    let mut cols = Vec::new();
    for x in -1..=1 {
        for z in -1..=1 {
            cols.push(ColumnPos { x, z });
        }
    }
    cols
}

#[test]
fn open_sky_is_full_and_stone_is_dark() {
    let world = FlatWorld::new(40);
    let h = Harness::light(&world, &one_chunk());
    assert_eq!(
        h.sky_at(0, 40, 0),
        15,
        "the first air block above the floor"
    );
    assert_eq!(h.sky_at(0, 63, 0), 15, "well above the floor");
    assert_eq!(h.sky_at(0, 39, 0), 0, "inside the stone");
}

#[test]
fn a_calibrated_sculk_sensor_emits_what_a_sculk_sensor_does() {
    atlas_once();
    let sensor = props::of(BlockState::from(BlockKind::SculkSensor)).emission;
    let calibrated = props::of(BlockState::from(BlockKind::CalibratedSculkSensor)).emission;
    assert_eq!(sensor, 1);
    assert_eq!(calibrated, sensor);
}

#[test]
fn light_falls_off_under_an_overhang() {
    let mut world = FlatWorld::new(40);
    for x in -3..=3 {
        for z in -3..=3 {
            world.set(x, 44, z, BlockKind::Stone);
        }
    }
    let h = Harness::light(&world, &one_chunk());
    assert_eq!(h.sky_at(6, 41, 0), 15, "outside the lid, still open sky");
    let edge = h.sky_at(3, 41, 0);
    let middle = h.sky_at(0, 41, 0);
    assert!(edge > middle, "edge {edge} should beat middle {middle}");
    assert!(middle < 15, "under a lid nothing is full brightness");
}

#[test]
fn a_torch_lights_its_neighbourhood() {
    let mut world = FlatWorld::new(40);
    world.set(0, 40, 0, BlockKind::Torch);
    let h = Harness::light(&world, &one_chunk());
    assert_eq!(h.block_at(0, 40, 0), 14, "the torch cell itself");
    assert_eq!(h.block_at(1, 40, 0), 13);
    assert_eq!(h.block_at(4, 40, 0), 10);
    assert_eq!(h.block_at(14, 40, 0), 0, "past the torch's reach");
}

#[test]
fn digging_down_carries_sky_light_with_it() {
    let mut world = FlatWorld::new(40);
    let mut h = Harness::light(&world, &one_chunk());
    assert_eq!(h.sky_at(0, 38, 0), 0);

    for y in [39, 38] {
        world.set(0, y, 0, BlockKind::Air);
    }
    if let Some(sources) = h.sources.column_mut(0, 0) {
        for y in [39, 38] {
            sources.update(0, y, 0, |lx, sy, lz| {
                props::of(world.block_state(lx as i32, sy, lz as i32))
            });
        }
    }
    for y in [39, 38] {
        h.sky.check_block(0, y, 0);
    }
    h.sky.run_light_updates(&world, Some(&h.sources));
    assert_eq!(h.sky_at(0, 39, 0), 15, "the shaft is still open sky");
    assert_eq!(h.sky_at(0, 38, 0), 15);
    assert_eq!(
        h.sky_at(0, 37, 0),
        0,
        "the floor under the shaft is untouched"
    );
}

#[test]
fn a_box_built_in_open_air_seals_out_the_sky() {
    let mut world = FlatWorld::new(20);
    let mut h = Harness::light(&world, &one_chunk());
    let sec = SectionPos { x: 0, y: 3, z: 0 };
    assert!(
        h.sky.storage.data_layer(sec).is_none(),
        "the section starts empty"
    );
    assert_eq!(h.sky_at(0, 51, 0), 0, "and unlit, because it has no layer");

    let mut placed = Vec::new();
    for x in -1..=1 {
        for y in 50..=52 {
            for z in -1..=1 {
                if (x, y, z) == (0, 51, 0) {
                    continue;
                }
                world.set(x, y, z, BlockKind::Stone);
                placed.push((x, y, z));
            }
        }
    }
    for (x, y, z) in placed {
        let empty = section_is_empty(&world, sec);
        h.block.update_section_status(sec, empty);
        h.sky.update_section_status(sec, empty);
        let (cx, cz) = (x >> 4, z >> 4);
        if let Some(sources) = h.sources.column_mut(cx, cz) {
            sources.update((x & 15) as usize, y, (z & 15) as usize, |lx, sy, lz| {
                props::of(world.block_state(cx * 16 + lx as i32, sy, cz * 16 + lz as i32))
            });
        }
        h.sky.check_block(x, y, z);
    }
    h.sky.run_light_updates(&world, Some(&h.sources));

    assert!(
        h.sky.storage.data_layer(sec).is_some(),
        "the section holds blocks now and has to hold light with them"
    );
    assert_eq!(
        h.sky_at(0, 51, 0),
        0,
        "the inside of a sealed box sees no sky"
    );
    assert_eq!(h.sky_at(5, 51, 0), 15, "the open air beside it still does");
}

fn section_is_empty(world: &FlatWorld, sec: SectionPos) -> bool {
    let base = sec.min_block_y();
    (0..16).all(|ly| {
        (0..16).all(|lx| {
            (0..16).all(|lz| {
                world
                    .block_state(sec.x * 16 + lx, base + ly, sec.z * 16 + lz)
                    .is_air()
            })
        })
    })
}
