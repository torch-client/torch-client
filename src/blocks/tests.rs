use crate::platform::time::Instant;
use std::collections::HashMap;
use std::sync::Once;

use super::bake::{bake_resolved, tile_uv_rect};
use super::rand::position_seed;
use super::rotation;
use super::state::{Rot, StateDef};
use crate::items::json::Json;
use crate::items::model::{Assets, Resolved};
use crate::util::javarandom::JavaRandom;

pub(crate) fn atlas_once() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let dir = crate::textures_dir().to_string_lossy().into_owned();
        let textures = crate::renderer::Textures::blocks(&dir);
        let (_image, map) = crate::renderer::build_block_atlas(&textures, &dir);
        let rows = (1 + map.len() as u32).div_ceil(crate::renderer::ATLAS_COLS);
        crate::TEXTURE_MAP.set(map).ok();
        crate::ATLAS_ROWS.set(rows).ok();
    });
}

fn assets() -> Assets {
    Assets::new(&crate::assets_root())
}

fn blockstate(name: &str) -> StateDef {
    let text =
        std::fs::read_to_string(crate::assets_root().join(format!("blockstates/{}.json", name)))
            .unwrap_or_else(|e| panic!("read blockstates/{}.json: {}", name, e));
    StateDef::parse(&Json::parse(&text).expect("parse")).expect("definition")
}

fn props<'a>(pairs: &[(&'a str, &'a str)]) -> HashMap<&'a str, &'a str> {
    pairs.iter().copied().collect()
}

fn resolve_inline(json: &str) -> Resolved {
    let node = Json::parse(json).expect("parse model");
    let mut textures = HashMap::new();
    if let Some(Json::Obj(fields)) = node.get("textures") {
        for (key, value) in fields {
            textures.insert(
                key.clone(),
                value.as_str().expect("texture path").to_string(),
            );
        }
    }
    Resolved {
        textures,
        elements: crate::items::model::parse_elements(node.get("elements").expect("elements")),
        ..Resolved::default()
    }
}

#[test]
fn variants_predicate_picks_the_matching_model() {
    let def = blockstate("oak_stairs");
    let picked = |p: &[(&str, &str)]| {
        let props = props(p);
        let slots = def.select(&props);
        assert_eq!(slots.len(), 1, "a variants file selects exactly one slot");
        let r = &slots[0].choices[0];
        (r.model.clone(), r.rot)
    };

    let (model, rot) = picked(&[
        ("facing", "east"),
        ("half", "bottom"),
        ("shape", "straight"),
        ("waterlogged", "false"),
    ]);
    assert_eq!(model, "block/oak_stairs");
    assert_eq!(
        rot,
        Rot {
            x: 0,
            y: 0,
            z: 0,
            uvlock: false
        }
    );

    let (model, rot) = picked(&[
        ("facing", "north"),
        ("half", "top"),
        ("shape", "straight"),
        ("waterlogged", "false"),
    ]);
    assert_eq!(model, "block/oak_stairs");
    assert_eq!(
        rot,
        Rot {
            x: 2,
            y: 3,
            z: 0,
            uvlock: true
        }
    );

    let (model, _) = picked(&[
        ("facing", "east"),
        ("half", "bottom"),
        ("shape", "inner_left"),
        ("waterlogged", "false"),
    ]);
    assert_eq!(model, "block/oak_stairs_inner");
}

#[test]
fn empty_variant_key_matches_every_state() {
    let def = blockstate("stone");
    let slots = def.select(&props(&[]));
    assert_eq!(slots.len(), 1);
    assert_eq!(slots[0].choices.len(), 4);
    assert_eq!(slots[0].total, 4, "weight defaults to 1 per choice");
    assert_eq!(slots[0].choices[2].rot.y, 2);
}

#[test]
fn multipart_fence_contributes_one_part_per_connection() {
    let def = blockstate("oak_fence");
    let count = |p: &[(&str, &str)]| def.select(&props(p)).len();

    assert_eq!(
        count(&[
            ("north", "false"),
            ("south", "false"),
            ("east", "false"),
            ("west", "false"),
            ("waterlogged", "false"),
        ]),
        1
    );
    assert_eq!(
        count(&[
            ("north", "true"),
            ("south", "false"),
            ("east", "true"),
            ("west", "false"),
            ("waterlogged", "false"),
        ]),
        3
    );
    assert_eq!(
        count(&[
            ("north", "true"),
            ("south", "true"),
            ("east", "true"),
            ("west", "true"),
            ("waterlogged", "false"),
        ]),
        5
    );
}

#[test]
fn multipart_wall_distinguishes_low_from_tall() {
    let def = blockstate("cobblestone_wall");
    let base = [
        ("up", "true"),
        ("north", "none"),
        ("south", "none"),
        ("east", "none"),
        ("west", "none"),
        ("waterlogged", "false"),
    ];
    let models = |p: &[(&str, &str)]| -> Vec<String> {
        def.select(&props(p))
            .iter()
            .map(|s| s.choices[0].model.clone())
            .collect()
    };

    assert_eq!(models(&base), vec!["block/cobblestone_wall_post"]);

    let mut low = base;
    low[1] = ("north", "low");
    assert_eq!(
        models(&low),
        vec!["block/cobblestone_wall_post", "block/cobblestone_wall_side"]
    );

    let mut tall = base;
    tall[1] = ("north", "tall");
    assert_eq!(
        models(&tall),
        vec![
            "block/cobblestone_wall_post",
            "block/cobblestone_wall_side_tall"
        ]
    );

    let mut no_post = tall;
    no_post[0] = ("up", "false");
    assert_eq!(models(&no_post), vec!["block/cobblestone_wall_side_tall"]);
}

#[test]
fn multipart_or_condition_matches_any_term() {
    let def = blockstate("redstone_wire");
    let models = |p: &[(&str, &str)]| -> Vec<String> {
        def.select(&props(p))
            .iter()
            .map(|s| s.choices[0].model.clone())
            .collect()
    };
    let wire = |n: &'static str, e: &'static str| {
        [
            ("north", n),
            ("east", e),
            ("south", "none"),
            ("west", "none"),
            ("power", "0"),
        ]
    };

    assert_eq!(
        models(&wire("none", "none")),
        vec!["block/redstone_dust_dot"]
    );
    assert_eq!(
        models(&wire("side", "none")),
        vec!["block/redstone_dust_side0"]
    );
    assert_eq!(
        models(&wire("side", "side")),
        vec![
            "block/redstone_dust_dot",
            "block/redstone_dust_side0",
            "block/redstone_dust_side_alt1",
        ]
    );
    assert!(models(&wire("up", "none")).contains(&"block/redstone_dust_up".to_string()));
}

#[test]
fn negated_and_alternative_terms_parse() {
    let def = StateDef::parse(
        &Json::parse(
            r#"{"multipart":[
                 {"when":{"face":"floor|ceiling"},"apply":{"model":"a"}},
                 {"when":{"face":"!wall"},"apply":{"model":"b"}},
                 {"when":{"AND":[{"face":"wall"},{"powered":"true"}]},"apply":{"model":"c"}}
               ]}"#,
        )
        .unwrap(),
    )
    .unwrap();
    let models = |p: &[(&str, &str)]| -> Vec<String> {
        def.select(&props(p))
            .iter()
            .map(|s| s.choices[0].model.clone())
            .collect()
    };
    assert_eq!(
        models(&[("face", "floor"), ("powered", "false")]),
        vec!["a", "b"]
    );
    assert_eq!(models(&[("face", "wall"), ("powered", "true")]), vec!["c"]);
    assert_eq!(
        models(&[("face", "ceiling"), ("powered", "true")]),
        vec!["a", "b"]
    );
}

const NORTH_FACE_ONLY: &str = r##"{
  "textures": {"t": "block/stone"},
  "elements": [{
    "from": [0, 0, 0], "to": [16, 16, 16],
    "faces": {"north": {"texture": "#t", "cullface": "north"}}
  }]
}"##;

#[test]
fn a_rotated_quad_keeps_face_info_vertex_order() {
    atlas_once();
    let model = resolve_inline(NORTH_FACE_ONLY);
    for rot in [
        Rot {
            x: 0,
            y: 1,
            z: 0,
            uvlock: false,
        },
        Rot {
            x: 0,
            y: 2,
            z: 0,
            uvlock: false,
        },
        Rot {
            x: 0,
            y: 3,
            z: 0,
            uvlock: false,
        },
        Rot {
            x: 1,
            y: 0,
            z: 0,
            uvlock: false,
        },
        Rot {
            x: 3,
            y: 2,
            z: 0,
            uvlock: false,
        },
    ] {
        let quad = &bake_resolved(&model, rot, 16).quads[0];
        let expected = crate::items::model::face_positions(
            [0.0, 0.0, 0.0],
            [1.0, 1.0, 1.0],
            quad.face as usize,
        );
        for (i, (got, want)) in quad.pos.iter().zip(expected.iter()).enumerate() {
            for axis in 0..3 {
                assert!(
                    (got[axis] - want[axis]).abs() < 1e-5,
                    "{rot:?} vertex {i} axis {axis}: {got:?} against {want:?}",
                );
            }
        }
    }
}

#[test]
fn model_rotation_moves_positions_and_cullface() {
    atlas_once();
    let model = resolve_inline(NORTH_FACE_ONLY);

    let plain = bake_resolved(&model, Rot::default(), 16);
    assert_eq!(plain.quads.len(), 1);
    assert_eq!(plain.quads[0].cull, Some(2));
    assert!(
        plain.quads[0].pos.iter().all(|p| p[2] == 0.0),
        "north face sits at z=0"
    );

    let turned = bake_resolved(
        &model,
        Rot {
            x: 0,
            y: 1,
            z: 0,
            uvlock: false,
        },
        16,
    );
    assert_eq!(turned.quads[0].cull, Some(5), "north rotates to east");
    assert!(
        turned.quads[0].pos.iter().all(|p| p[0] == 1.0),
        "and its plane to x=1"
    );
    assert!((turned.quads[0].normal[0] - 1.0).abs() < 1e-5);

    let tipped = bake_resolved(
        &model,
        Rot {
            x: 1,
            y: 0,
            z: 0,
            uvlock: false,
        },
        16,
    );
    assert_eq!(tipped.quads[0].cull, Some(0), "north rotates to down");
    assert!(tipped.quads[0].pos.iter().all(|p| p[1] == 0.0));
}

#[test]
fn uvlock_keeps_the_texture_still_while_rotation_moves_it() {
    atlas_once();
    let model = resolve_inline(
        r##"{
          "textures": {"t": "block/stone"},
          "elements": [{
            "from": [0, 0, 0], "to": [16, 16, 16],
            "faces": {"up": {"texture": "#t", "uv": [0, 0, 16, 16]}}
          }]
        }"##,
    );

    let rot_y90 = Rot {
        x: 0,
        y: 1,
        z: 0,
        uvlock: false,
    };
    let plain = &bake_resolved(&model, Rot::default(), 16).quads[0];
    let turned = &bake_resolved(&model, rot_y90, 16).quads[0];
    let locked = &bake_resolved(
        &model,
        Rot {
            uvlock: true,
            ..rot_y90
        },
        16,
    )
    .quads[0];

    let corners = |q: &super::BakedQuad| {
        let mut c: Vec<[i32; 3]> = q
            .pos
            .iter()
            .map(|p| {
                [
                    (p[0] * 16.0) as i32,
                    (p[1] * 16.0) as i32,
                    (p[2] * 16.0) as i32,
                ]
            })
            .collect();
        c.sort();
        c
    };
    assert_eq!(corners(plain), corners(turned));
    assert_eq!(corners(plain), corners(locked));

    let mapping = |q: &super::BakedQuad| {
        let mut m: Vec<([i32; 3], [i32; 2])> = (0..4)
            .map(|i| {
                (
                    [
                        (q.pos[i][0] * 16.0).round() as i32,
                        (q.pos[i][1] * 16.0).round() as i32,
                        (q.pos[i][2] * 16.0).round() as i32,
                    ],
                    [
                        (q.uv[i][0] * 1e4).round() as i32,
                        (q.uv[i][1] * 1e4).round() as i32,
                    ],
                )
            })
            .collect();
        m.sort();
        m
    };
    assert_eq!(
        mapping(plain),
        mapping(locked),
        "uvlock pins the texture in world space"
    );
    assert_ne!(
        mapping(plain),
        mapping(turned),
        "plain rotation turns the texture"
    );
}

#[test]
fn per_face_uv_and_rotation_land_where_the_maths_says() {
    atlas_once();
    let model = resolve_inline(
        r##"{
          "textures": {"t": "block/stone"},
          "elements": [{
            "from": [0, 0, 0], "to": [16, 16, 16],
            "faces": {"up": {"texture": "#t", "uv": [0, 8, 8, 16], "rotation": 90}}
          }]
        }"##,
    );
    let baked = bake_resolved(&model, Rot::default(), 16);
    let quad = &baked.quads[0];

    let tile = *crate::TEXTURE_MAP.get().unwrap().get("stone").unwrap();
    let [au0, av0, au1, av1] = tile_uv_rect(tile, 16);
    let at = |u: f32, v: f32| [au0 + (au1 - au0) * u, av0 + (av1 - av0) * v];

    let expected = [at(0.0, 1.0), at(0.5, 1.0), at(0.5, 0.5), at(0.0, 0.5)];
    for (got, want) in quad.uv.iter().zip(expected.iter()) {
        assert!(
            (got[0] - want[0]).abs() < 1e-6 && (got[1] - want[1]).abs() < 1e-6,
            "uv {:?} != {:?}",
            got,
            want
        );
    }
}

#[test]
fn element_rotation_rescale_stretches_the_plane_out_to_the_cell() {
    atlas_once();
    let model = resolve_inline(
        r##"{
          "textures": {"t": "block/short_grass"},
          "elements": [{
            "from": [0, 0, 8], "to": [16, 16, 8],
            "rotation": {"origin": [8, 8, 8], "axis": "y", "angle": 45, "rescale": true},
            "faces": {
              "north": {"texture": "#t"},
              "south": {"texture": "#t"}
            }
          }]
        }"##,
    );
    let baked = bake_resolved(&model, Rot::default(), 16);
    assert_eq!(
        baked.quads.len(),
        2,
        "the terrain material is backface-culled, so both coincident faces of a \
         zero-thickness element must survive to be visible from either side"
    );
    for quad in &baked.quads {
        for p in &quad.pos {
            assert!(
                (p[0] - 0.5).abs() > 0.49 && (p[2] - 0.5).abs() > 0.49,
                "{:?}",
                p
            );
            assert!(p[0] >= -1e-4 && p[0] <= 1.0 + 1e-4);
        }
    }
}

#[test]
fn a_full_opaque_cube_occludes_and_glass_does_not() {
    atlas_once();
    let assets = assets();
    let stone = super::bake::bake_model(&assets, "block/stone", Rot::default(), 16);
    assert!(stone.occludes, "stone is a full cube of opaque sprites");
    let glass = super::bake::bake_model(&assets, "block/glass", Rot::default(), 16);
    assert!(
        !glass.occludes,
        "glass is a full cube, but its sprite has holes"
    );
    let leaves = super::bake::bake_model(&assets, "block/oak_leaves", Rot::default(), 16);
    assert!(!leaves.occludes);
    let slab = super::bake::bake_model(&assets, "block/stone_slab", Rot::default(), 16);
    assert!(
        !slab.occludes,
        "a half-height element cannot occlude the cell"
    );
}

#[test]
fn weighted_choice_walks_the_cumulative_weights() {
    let def = StateDef::parse(
        &Json::parse(
            r#"{"variants":{"":[{"model":"a","weight":3},{"model":"b"},{"model":"c","weight":2}]}}"#,
        )
        .unwrap(),
    )
    .unwrap();
    let slots = def.select(&props(&[]));
    let slot = slots[0];
    assert_eq!(slot.total, 6);
    let names: Vec<&str> = slot.choices.iter().map(|c| c.model.as_str()).collect();
    assert_eq!(names, vec!["a", "b", "c"]);
}

#[test]
fn shape_offset_follows_block_behaviour_get_offset() {
    use super::rand::ShapeOffset;
    let grass = ShapeOffset::for_block("short_grass");
    assert_eq!(grass.at(0, 0), [-0.25, -0.2, -0.25]);
    assert_eq!(
        ShapeOffset::for_block("poppy").at(0, 0),
        [-0.25, 0.0, -0.25]
    );
    assert_eq!(
        ShapeOffset::for_block("pointed_dripstone").at(0, 0),
        [-0.125, 0.0, -0.125]
    );
    assert_eq!(ShapeOffset::for_block("grass_block"), ShapeOffset::None);
    assert_eq!(ShapeOffset::for_block("grass_block").at(3, 9), [0.0; 3]);
    for x in -8..8 {
        for z in -8..8 {
            let [ox, oy, oz] = grass.at(x, z);
            assert!((-0.25..=0.25).contains(&ox) && (-0.25..=0.25).contains(&oz));
            assert!((-0.2..=0.0).contains(&oy));
        }
    }
}

#[test]
fn position_seeded_random_matches_the_java_generator() {
    assert_eq!(position_seed(0, 0, 0), 0);
    let a = position_seed(1, 2, 3);
    let b = position_seed(1, 2, 4);
    assert_ne!(a, b, "neighbouring cells get different seeds");
    for bound in [2u32, 3, 4, 6, 7, 16] {
        let mut random = JavaRandom::new(position_seed(12, 34, 56));
        for _ in 0..64 {
            assert!(random.next_int(bound) < bound);
        }
    }
    let mut x = JavaRandom::new(a);
    let mut y = JavaRandom::new(a);
    assert_eq!(x.next_int(4), y.next_int(4));
}

#[test]
fn uv_transform_is_a_quarter_turn_for_every_rotation_and_face() {
    for x in 0..4u8 {
        for y in 0..4u8 {
            let matrix = rotation::model_matrix(Rot {
                x,
                y,
                z: 0,
                uvlock: true,
            });
            for dir in 0..6 {
                let t = rotation::uv_transform(&matrix, dir);
                let det = t[0][0] * t[1][1] - t[0][1] * t[1][0];
                assert!(
                    (det - 1.0).abs() < 1e-6,
                    "x={} y={} dir={} det={}",
                    x,
                    y,
                    dir,
                    det
                );
                for (u, v) in [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)] {
                    let [ru, rv] = rotation::apply_uv(&t, u, v);
                    assert!(ru.abs() < 1e-6 || (ru - 1.0).abs() < 1e-6, "{}", ru);
                    assert!(rv.abs() < 1e-6 || (rv - 1.0).abs() < 1e-6, "{}", rv);
                }
            }
        }
    }
}

#[test]
fn the_mesher_culls_a_face_against_a_solid_neighbour() {
    use crate::renderer::{Occupancy, build_section_mesh};
    use crate::util::block_model::block_visual;
    use azalea::block::BlockState;
    use azalea::registry::builtin::BlockKind;

    atlas_once();
    let stone = block_visual(BlockState::from(BlockKind::Stone));
    assert!(stone.is_solid);

    let quads = |occupied: &[(i32, i32, i32)]| -> usize {
        let mut occ = Occupancy::new(0, 0, -1, 18);
        for (x, y, z) in occupied {
            occ.set_solid(*x, *y, *z);
        }
        let blocks = vec![(0, 0, 0, stone.clone())];
        let (opaque, water) = build_section_mesh(&blocks, 0, 0, 0, 16, &occ);
        assert!(water.is_empty());
        assert_eq!(opaque.verts.len() % 4, 0);
        assert_eq!(
            opaque.idx.len() + opaque.cutout_idx.len(),
            opaque.verts.len() / 4 * 6
        );
        opaque.verts.len() / 4
    };

    assert_eq!(quads(&[(0, 0, 0)]), 6, "a lone cube shows all six faces");
    assert_eq!(
        quads(&[(0, 0, 0), (0, 1, 0)]),
        5,
        "a block above hides the top"
    );
    assert_eq!(
        quads(&[
            (0, 0, 0),
            (0, 1, 0),
            (0, -1, 0),
            (1, 0, 0),
            (-1, 0, 0),
            (0, 0, 1),
            (0, 0, -1)
        ]),
        0,
        "enclosed on all six sides, nothing shows"
    );
}

#[test]
fn every_block_state_bakes() {
    use azalea::block::{BlockState, BlockTrait};

    atlas_once();
    let registry = super::registry();
    let started = Instant::now();

    let mut states = 0u32;
    let mut quads = 0u64;
    let mut solid = 0u32;
    let mut randomized = 0u32;
    let mut empty: Vec<String> = Vec::new();
    let mut no_definition: Vec<String> = Vec::new();
    let mut block_entities: std::collections::BTreeSet<&str> = Default::default();
    let mut unselected: std::collections::BTreeSet<&str> = Default::default();
    let mut seen_blocks = std::collections::HashSet::new();

    for id in 0..=BlockState::MAX_STATE {
        let Some(state) = BlockState::try_from(id).ok() else {
            continue;
        };
        let block: Box<dyn BlockTrait> = Box::<dyn BlockTrait>::from(state);
        let name = block.id();
        if matches!(
            name,
            "air" | "cave_air" | "void_air" | "water" | "lava" | "bubble_column"
        ) {
            continue;
        }
        seen_blocks.insert(name);
        if registry.definition_for_test(name).is_none() {
            if no_definition.last().map(String::as_str) != Some(name) {
                no_definition.push(name.to_string());
            }
            continue;
        }
        let baked = registry.baked(state.id(), name, &block.property_map());
        states += 1;
        quads += baked
            .parts
            .iter()
            .map(|p| p.choices[0].quads.len() as u64)
            .sum::<u64>();
        solid += baked.is_solid as u32;
        randomized += baked.randomized as u32;
        let drew_nothing = baked
            .parts
            .iter()
            .all(|p| p.choices.iter().all(|m| m.quads.is_empty()));
        let block_entity = !baked.parts.is_empty()
            && baked
                .parts
                .iter()
                .all(|p| p.choices.iter().all(|m| !m.had_elements));
        if baked.parts.is_empty() {
            unselected.insert(name);
        } else if drew_nothing && block_entity {
            block_entities.insert(name);
        } else if drew_nothing && empty.len() < 40 {
            empty.push(format!("{} {:?}", name, block.property_map()));
        }
    }

    let elapsed = started.elapsed().as_secs_f32() * 1000.0;
    let (missing_defs, missing_textures) = registry.misses();
    println!(
        "[sweep] {} blocks, {} states baked in {:.0} ms; {} quads, {} solid, {} randomized",
        seen_blocks.len(),
        states,
        elapsed,
        quads,
        solid,
        randomized
    );
    println!(
        "[sweep] {} definitions loaded, {} states with no definition, \
         {} states asked for with none, {} faces without an atlas sprite",
        registry.definition_count(),
        no_definition.len(),
        missing_defs,
        missing_textures
    );
    println!(
        "[sweep] {} block-entity blocks draw no model geometry: {:?}",
        block_entities.len(),
        block_entities
    );
    println!(
        "[sweep] {} blocks have states no multipart selector matches: {:?}",
        unselected.len(),
        unselected
    );
    if !no_definition.is_empty() {
        println!("[sweep] no blockstate file: {:?}", no_definition);
    }
    if !empty.is_empty() {
        println!("[sweep] empty geometry: {:?}", empty);
    }

    assert!(
        empty.is_empty(),
        "{} block states baked to nothing",
        empty.len()
    );
    assert!(
        no_definition.is_empty(),
        "blocks with no blockstate file: {:?}",
        no_definition
    );
    assert!(states > 20_000, "only {} states swept", states);
}

#[test]
fn every_blockstate_file_parses() {
    let assets = assets();
    let dir = crate::assets_root().join("blockstates");
    let mut total = 0;
    let mut variants = 0;
    let mut multipart = 0;
    for entry in std::fs::read_dir(&dir).expect("blockstates dir").flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let name = path.file_stem().unwrap().to_str().unwrap();
        let json = assets
            .json(&format!("blockstates/{}.json", name))
            .unwrap_or_else(|| panic!("{} did not parse as JSON", name));
        let def = StateDef::parse(&json).unwrap_or_else(|| panic!("{} has no models", name));
        match def {
            StateDef::Variants(_) => variants += 1,
            StateDef::Multipart(_) => multipart += 1,
        }
        total += 1;
    }
    println!(
        "[sweep] {} blockstate files: {} variants, {} multipart",
        total, variants, multipart
    );
    assert!(total > 1000);
    assert!(multipart > 50, "the multipart path has to be exercised");
}
