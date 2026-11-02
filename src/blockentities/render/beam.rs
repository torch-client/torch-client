use std::f32::consts::PI;

use azalea_registry::builtin::BlockEntityKind;
use bevy::asset::RenderAssetUsages;
use bevy::mesh::{PrimitiveTopology, VertexAttributeValues};
use bevy::prelude::*;

use crate::blockentities::feed::BlockEntityData;
use crate::blockentities::sim::Anim;
use crate::blockentities::sim::beacon::Section;
use crate::blockentities::{BeRegistry, BeSpec, BeState, BuiltTexture};
use crate::entities::Blend;
use crate::entities::quads::quad_indices;
use crate::gui::atlas::GuiAtlas;
use crate::renderer::entity_material::LightMode;
use crate::util::dye::DYES;
use crate::util::mth::rgb_to_linear;

const MAX_RENDER_Y: i32 = 2048;
const BEAM_SCALE_THRESHOLD: f32 = 96.0;
const BEACON_VIEW_DISTANCE: f32 = 512.0;
const GATEWAY_VIEW_DISTANCE: f32 = 256.0;
const GLOW_ALPHA: f32 = 32.0 / 255.0;
const MAGENTA: usize = 2;
const PURPLE: usize = 10;

pub fn register(registry: &mut BeRegistry) {
    let beacons = [BlockEntityKind::Beacon, BlockEntityKind::TestInstanceBlock];
    registry.add_many(
        &beacons,
        beam_spec("beacon_beam", beacon_texture, core_mesh, refresh_core)
            .with_transform(core_transform)
            .with_view_distance(BEACON_VIEW_DISTANCE),
    );
    registry.add_many(
        &beacons,
        beam_spec("beacon_beam_glow", beacon_texture, glow_mesh, refresh_glow)
            .with_transform(glow_transform)
            .with_blend(Blend::Translucent)
            .with_tint(glow_tint)
            .with_view_distance(BEACON_VIEW_DISTANCE),
    );
    registry.add(
        BlockEntityKind::EndGateway,
        beam_spec("end_gateway_beam", gateway_texture, core_mesh, refresh_core)
            .with_transform(core_transform)
            .with_view_distance(GATEWAY_VIEW_DISTANCE),
    );
    registry.add(
        BlockEntityKind::EndGateway,
        beam_spec(
            "end_gateway_beam_glow",
            gateway_texture,
            glow_mesh,
            refresh_glow,
        )
        .with_transform(glow_transform)
        .with_blend(Blend::Translucent)
        .with_tint(glow_tint)
        .with_view_distance(GATEWAY_VIEW_DISTANCE),
    );
}

fn beacon_texture(_st: &BeState) -> String {
    "entity/beacon/beacon_beam".to_string()
}

fn gateway_texture(_st: &BeState) -> String {
    "entity/end_portal/end_gateway_beam".to_string()
}

fn beam_spec(
    name: &'static str,
    texture: fn(&BeState) -> String,
    build: fn(&BeState, &GuiAtlas) -> Option<Mesh>,
    refresh: fn(&BeState, &mut Mesh),
) -> BeSpec {
    BeSpec::built(name, segment_count, build, BuiltTexture::Tiled(texture))
        .with_refresh(refresh)
        .with_full_bright(|_| true)
        .with_light(LightMode::Emissive)
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Segment {
    start: i32,
    height: i32,
    color: u32,
}

enum Segments<'a> {
    Sections(&'a [Section]),
    One(Segment),
}

impl Segments<'_> {
    fn len(&self) -> usize {
        match self {
            Segments::Sections(sections) => sections.len(),
            Segments::One(_) => 1,
        }
    }

    fn for_each(&self, mut f: impl FnMut(Segment)) {
        match self {
            Segments::Sections(sections) => {
                let mut start = 0;
                for (i, section) in sections.iter().enumerate() {
                    let height = if i + 1 == sections.len() {
                        MAX_RENDER_Y
                    } else {
                        section.height
                    };
                    f(Segment {
                        start,
                        height,
                        color: section.color,
                    });
                    start += section.height;
                }
            }
            Segments::One(segment) => f(*segment),
        }
    }
}

struct Beam<'a> {
    segments: Segments<'a>,
    scale: f32,
    solid_radius: f32,
    glow_radius: f32,
}

impl<'a> Beam<'a> {
    fn of(st: &'a BeState) -> Option<Beam<'a>> {
        match (&st.anim, &*st.data) {
            (Anim::Beacon(beacon), _) if !beacon.sections().is_empty() => {
                Some(beacon_beam(st, Segments::Sections(beacon.sections())))
            }
            (_, BlockEntityData::Bounds(data)) if st.kind == BlockEntityKind::TestInstanceBlock => {
                let color = crate::blockentities::sim::bounds::beam_color(data)?;
                Some(beacon_beam(
                    st,
                    Segments::One(Segment {
                        start: 0,
                        height: MAX_RENDER_Y,
                        color,
                    }),
                ))
            }
            (Anim::Gateway(gateway), _) => {
                let spawning = gateway.is_spawning();
                if !spawning && !gateway.is_cooling_down() {
                    return None;
                }
                let (percent, distance, dye) = if spawning {
                    let max_y = crate::renderer::dimension::current().y_range().1 - 1;
                    (gateway.spawn_percent(st.partial), max_y as f32, MAGENTA)
                } else {
                    (gateway.cooldown_percent(st.partial), 50.0, PURPLE)
                };
                let scale = (percent * PI).sin();
                let height = (scale * distance).floor() as i32;
                Some(Beam {
                    segments: Segments::One(Segment {
                        start: -height,
                        height: height * 2,
                        color: 0xFF00_0000 | DYES[dye].texture_diffuse,
                    }),
                    scale,
                    solid_radius: 0.15,
                    glow_radius: 0.175,
                })
            }
            _ => None,
        }
    }
}

fn beacon_beam<'a>(st: &BeState, segments: Segments<'a>) -> Beam<'a> {
    let centre = st.origin.block_centre_to_render(st.pos);
    let center = Vec2::new(centre.x, centre.z);
    let camera = Vec2::new(st.camera.position.x, st.camera.position.z);
    let radius_scale = (center.distance(camera) / BEAM_SCALE_THRESHOLD).max(1.0);
    Beam {
        segments,
        scale: 1.0,
        solid_radius: 0.2 * radius_scale,
        glow_radius: 0.25 * radius_scale,
    }
}

fn segment_count(st: &BeState) -> Option<u64> {
    Beam::of(st).map(|beam| beam.segments.len() as u64)
}

fn animation_time(st: &BeState) -> f32 {
    (st.game_time % 40) as f32 + st.partial
}

fn tex_v_off(animation_time: f32, height: i32) -> f32 {
    let scroll = if height < 0 {
        animation_time
    } else {
        -animation_time
    };
    let v = scroll * 0.2 - (scroll * 0.1).floor();
    v - v.floor()
}

#[derive(Clone, Copy)]
enum Part {
    Core,
    Glow,
}

impl Part {
    fn corners(self, beam: &Beam) -> [[f32; 2]; 4] {
        match self {
            Part::Core => {
                let r = beam.solid_radius;
                [[0.0, r], [r, 0.0], [-r, 0.0], [0.0, -r]]
            }
            Part::Glow => {
                let r = beam.glow_radius;
                [[-r, -r], [r, -r], [-r, r], [r, r]]
            }
        }
    }

    fn v_span(self, beam: &Beam, height: i32) -> f32 {
        let span = height as f32 * beam.scale;
        match self {
            Part::Core => span * (0.5 / beam.solid_radius),
            Part::Glow => span,
        }
    }
}

const QUADS: [(usize, usize); 4] = [(0, 1), (3, 2), (1, 3), (2, 0)];
const VERTICES_PER_SEGMENT: usize = QUADS.len() * 4;

fn write(st: &BeState, part: Part, mesh: &mut Mesh) {
    let Some(beam) = Beam::of(st) else {
        return;
    };
    let mut positions = match mesh.remove_attribute(Mesh::ATTRIBUTE_POSITION) {
        Some(VertexAttributeValues::Float32x3(values)) => values,
        _ => Vec::new(),
    };
    let mut uvs = match mesh.remove_attribute(Mesh::ATTRIBUTE_UV_0) {
        Some(VertexAttributeValues::Float32x2(values)) => values,
        _ => Vec::new(),
    };
    let mut colors = match mesh.remove_attribute(Mesh::ATTRIBUTE_COLOR) {
        Some(VertexAttributeValues::Float32x4(values)) => values,
        _ => Vec::new(),
    };
    positions.clear();
    uvs.clear();
    colors.clear();
    let corners = part.corners(&beam);
    let time = animation_time(st);
    beam.segments.for_each(|segment| {
        let start = segment.start as f32;
        let end = (segment.start + segment.height) as f32;
        let vv2 = -1.0 + tex_v_off(time, segment.height);
        let vv1 = part.v_span(&beam, segment.height) + vv2;
        let color = rgb_to_linear(segment.color);
        for (a, b) in QUADS {
            let [ax, az] = corners[a];
            let [bx, bz] = corners[b];
            positions.extend_from_slice(&[
                [ax, end, az],
                [ax, start, az],
                [bx, start, bz],
                [bx, end, bz],
            ]);
            uvs.extend_from_slice(&[[1.0, vv1], [1.0, vv2], [0.0, vv2], [0.0, vv1]]);
            colors.extend_from_slice(&[color; 4]);
        }
    });
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
}

fn mesh(st: &BeState, part: Part) -> Option<Mesh> {
    let segments = Beam::of(st)?.segments.len();
    let vertices = segments * VERTICES_PER_SEGMENT;
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0f32, 1.0, 0.0]; vertices]);
    mesh.insert_indices(quad_indices(vertices / 4));
    write(st, part, &mut mesh);
    Some(mesh)
}

fn core_mesh(st: &BeState, _atlas: &GuiAtlas) -> Option<Mesh> {
    mesh(st, Part::Core)
}

fn glow_mesh(st: &BeState, _atlas: &GuiAtlas) -> Option<Mesh> {
    mesh(st, Part::Glow)
}

fn refresh_core(st: &BeState, mesh: &mut Mesh) {
    write(st, Part::Core, mesh);
}

fn refresh_glow(st: &BeState, mesh: &mut Mesh) {
    write(st, Part::Glow, mesh);
}

fn core_transform(st: &BeState) -> Transform {
    let degrees = animation_time(st) * 2.25 - 45.0;
    glow_transform(st).with_rotation(Quat::from_rotation_y(degrees.to_radians()))
}

fn glow_transform(_st: &BeState) -> Transform {
    Transform::from_xyz(0.5, 0.0, 0.5)
}

fn glow_tint(_st: &BeState) -> [f32; 4] {
    [1.0, 1.0, 1.0, GLOW_ALPHA]
}

#[cfg(test)]
fn lit_beacon() -> BeState {
    use crate::blockentities::sim::beacon::Beacon;
    let mut st = super::test_state("beacon", &[]);
    st.anim = Anim::Beacon(Beacon::lit(&[
        Section {
            color: 0xFF00_0000 | DYES[0].texture_diffuse,
            height: 1,
        },
        Section {
            color: 0xFF00_0000 | DYES[14].texture_diffuse,
            height: 3,
        },
        Section {
            color: 0xFF00_0000 | DYES[11].texture_diffuse,
            height: 2,
        },
    ]));
    st
}

#[cfg(test)]
pub(super) fn test_states(out: &mut Vec<BeState>) {
    use crate::blockentities::sim::gateway::Gateway;
    out.push(super::test_state("beacon", &[]));
    out.push(lit_beacon());
    out.push(super::test_state("end_gateway", &[]));
    let mut cooling = super::test_state("end_gateway", &[]);
    cooling.anim = Anim::Gateway(Gateway::cooling(20));
    out.push(cooling);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tex_v_off_scrolls_and_wraps() {
        assert_eq!(tex_v_off(0.0, 1), 0.0);
        assert!((tex_v_off(1.0, 1) - 0.8).abs() < 1e-5);
        assert!((tex_v_off(1.0, -1) - 0.2).abs() < 1e-5);
        for t in [0.0, 3.5, 17.25, 39.9] {
            let v = tex_v_off(t, 4);
            assert!((0.0..1.0).contains(&v), "{t} -> {v}");
        }
    }

    #[test]
    fn sections_stack_and_the_last_runs_to_the_ceiling() {
        let st = lit_beacon();
        let beam = Beam::of(&st).unwrap();
        let mut segments = Vec::new();
        beam.segments
            .for_each(|s| segments.push((s.start, s.height)));
        assert_eq!(segments, vec![(0, 1), (1, 3), (4, MAX_RENDER_Y)]);
    }

    #[test]
    fn an_unlit_beacon_draws_nothing() {
        assert_eq!(
            segment_count(&super::super::test_state("beacon", &[])),
            None
        );
    }

    #[test]
    fn every_beam_mesh_carries_the_entity_vertex_layout() {
        let mut states = Vec::new();
        test_states(&mut states);
        let mut built = 0;
        for st in &states {
            for part in [Part::Core, Part::Glow] {
                let Some(mesh) = mesh(st, part) else { continue };
                let vertices = mesh.count_vertices();
                assert_eq!(vertices % VERTICES_PER_SEGMENT, 0);
                assert_eq!(
                    mesh.indices().map(bevy::mesh::Indices::len),
                    Some(vertices / 4 * 6)
                );
                assert!(mesh.attribute(Mesh::ATTRIBUTE_NORMAL).is_some());
                built += 1;
            }
        }
        assert_eq!(built, 6);
    }

    #[test]
    fn the_gateway_beam_spans_both_ways_from_the_block() {
        let mut st = super::super::test_state("end_gateway", &[]);
        st.partial = 1.0;
        let beam = Beam::of(&st).unwrap();
        let Segments::One(segment) = beam.segments else {
            panic!("one segment");
        };
        assert!(segment.height > 0);
        assert_eq!(segment.start, -segment.height / 2);
        assert_eq!(segment.color, 0xFF00_0000 | DYES[MAGENTA].texture_diffuse);
    }
}
