use azalea_registry::builtin::{BlockKind, EntityKind};
use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::math::DVec3;
use bevy::mesh::{Indices, PrimitiveTopology, VertexAttributeValues};
use bevy::prelude::*;

use super::quads::QUAD_INDICES;
use crate::renderer::dimension::Dimension;
use crate::renderer::entity_material::{EntityMaterial, LightMode, Lit};
use crate::renderer::lightmap::levels_in;

const FADE_DISTANCE_SQ: f64 = 256.0;
const MAX_RADIUS: f32 = 32.0;
const MIN_BRIGHTNESS: i32 = 3;
const NIGHT_FACTOR: f32 = 0.24;
const NIGHT_LEVEL_TRACK: f32 = 0.266_666_68;
const DEPTH_BIAS: i32 = 256;
const TEXTURE: &str = "misc/shadow";

#[derive(Component)]
pub(crate) struct ShadowNode;

struct Piece {
    at: Vec3,
    rel: [f32; 3],
    radius: f32,
    alpha: f32,
}

impl Piece {
    fn corners(&self) -> [[f32; 3]; 4] {
        let [x0, y, z0] = self.rel;
        let (x1, z1) = (x0 + 1.0, z0 + 1.0);
        [[x0, y, z0], [x0, y, z1], [x1, y, z1], [x1, y, z0]]
            .map(|[x, y, z]| (self.at + Vec3::new(x, y, z)).to_array())
    }

    fn uvs(&self) -> [[f32; 2]; 4] {
        let uv = |x: f32, z: f32| [-x / 2.0 / self.radius + 0.5, -z / 2.0 / self.radius + 0.5];
        let (x0, z0) = (self.rel[0], self.rel[2]);
        let (x1, z1) = (x0 + 1.0, z0 + 1.0);
        [uv(x0, z0), uv(x0, z1), uv(x1, z1), uv(x1, z0)]
    }
}

struct Caster {
    pos: [f64; 3],
    radius: f32,
    strength: f32,
}

#[derive(Resource, Default)]
pub(crate) struct EntityShadows {
    node: Option<(Entity, Handle<Mesh>)>,
    casters: Vec<Caster>,
    pieces: Vec<Piece>,
}

pub(crate) fn shadow(kind: EntityKind) -> (f32, f32) {
    use EntityKind as K;
    match kind {
        K::Item | K::ExperienceOrb => (0.15, 0.75),

        K::Tadpole => (0.14, 1.0),
        K::TropicalFish => (0.15, 1.0),
        K::Pufferfish => (0.2, 1.0),
        K::Bat | K::Slime | K::MagmaCube => (0.25, 1.0),

        K::Chicken
        | K::Cod
        | K::Endermite
        | K::Frog
        | K::Parrot
        | K::Silverfish
        | K::Vex
        | K::Rabbit => (0.3, 1.0),

        K::Allay | K::Armadillo | K::Bee | K::Cat | K::Fox | K::Ocelot | K::Salmon => (0.4, 1.0),

        K::Zombie
        | K::Husk
        | K::Drowned
        | K::ZombieVillager
        | K::ZombifiedPiglin
        | K::Skeleton
        | K::Stray
        | K::Bogged
        | K::Parched
        | K::WitherSkeleton
        | K::Piglin
        | K::PiglinBrute
        | K::Blaze
        | K::Breeze
        | K::Creeper
        | K::Enderman
        | K::Evoker
        | K::Illusioner
        | K::Pillager
        | K::Vindicator
        | K::CopperGolem
        | K::Villager
        | K::WanderingTrader
        | K::Witch
        | K::SnowGolem
        | K::Strider
        | K::Axolotl
        | K::Wolf
        | K::Guardian
        | K::Player
        | K::Mannequin
        | K::EndCrystal
        | K::Tnt
        | K::FallingBlock
        | K::EnderDragon => (0.5, 1.0),

        K::CaveSpider => (0.56, 1.0),
        K::Creaking => (0.6, 1.0),

        K::Camel
        | K::CamelHusk
        | K::Cow
        | K::Mooshroom
        | K::Dolphin
        | K::Goat
        | K::Hoglin
        | K::Zoglin
        | K::IronGolem
        | K::Llama
        | K::TraderLlama
        | K::Nautilus
        | K::ZombieNautilus
        | K::Pig
        | K::Sheep
        | K::Squid
        | K::GlowSquid
        | K::Turtle
        | K::Minecart
        | K::ChestMinecart
        | K::CommandBlockMinecart
        | K::FurnaceMinecart
        | K::HopperMinecart
        | K::SpawnerMinecart
        | K::TntMinecart => (0.7, 1.0),

        K::Horse | K::Donkey | K::Mule | K::SkeletonHorse | K::ZombieHorse | K::Phantom => {
            (0.75, 1.0)
        }

        K::Spider
        | K::OakBoat
        | K::OakChestBoat
        | K::SpruceBoat
        | K::SpruceChestBoat
        | K::BirchBoat
        | K::BirchChestBoat
        | K::JungleBoat
        | K::JungleChestBoat
        | K::AcaciaBoat
        | K::AcaciaChestBoat
        | K::CherryBoat
        | K::CherryChestBoat
        | K::DarkOakBoat
        | K::DarkOakChestBoat
        | K::PaleOakBoat
        | K::PaleOakChestBoat
        | K::MangroveBoat
        | K::MangroveChestBoat
        | K::BambooRaft
        | K::BambooChestRaft => (0.8, 1.0),

        K::Panda | K::PolarBear | K::Warden => (0.9, 1.0),
        K::Wither => (1.0, 1.0),
        K::Ravager | K::Sniffer => (1.1, 1.0),
        K::ElderGuardian => (1.2, 1.0),
        K::Ghast => (1.5, 1.0),
        K::HappyGhast => (2.0, 1.0),
        K::Giant => (3.0, 1.0),

        _ => (0.0, 1.0),
    }
}

fn age_scaled(kind: EntityKind) -> bool {
    use EntityKind as K;
    !matches!(
        kind,
        K::Player
            | K::Mannequin
            | K::ArmorStand
            | K::Item
            | K::ExperienceOrb
            | K::EndCrystal
            | K::Tnt
            | K::FallingBlock
            | K::EnderDragon
            | K::Minecart
            | K::ChestMinecart
            | K::CommandBlockMinecart
            | K::FurnaceMinecart
            | K::HopperMinecart
            | K::SpawnerMinecart
            | K::TntMinecart
            | K::OakBoat
            | K::OakChestBoat
            | K::SpruceBoat
            | K::SpruceChestBoat
            | K::BirchBoat
            | K::BirchChestBoat
            | K::JungleBoat
            | K::JungleChestBoat
            | K::AcaciaBoat
            | K::AcaciaChestBoat
            | K::CherryBoat
            | K::CherryChestBoat
            | K::DarkOakBoat
            | K::DarkOakChestBoat
            | K::PaleOakBoat
            | K::PaleOakChestBoat
            | K::MangroveBoat
            | K::MangroveChestBoat
            | K::BambooRaft
            | K::BambooChestRaft
    )
}

fn shadow_radius(
    kind: EntityKind,
    base: f32,
    scale: f32,
    age_scale: f32,
    shared: &super::state::ExtrasShared,
) -> f32 {
    use EntityKind as K;
    match kind {
        K::Slime | K::MagmaCube => return shared.size as f32 * 0.25,
        K::Pufferfish => return 0.1 + 0.1 * shared.puff_state as f32,
        _ => {}
    }
    let mut radius = base * scale;
    if age_scaled(kind) {
        radius *= age_scale;
    }
    if shared.is_baby {
        radius *= match kind {
            K::Villager | K::Strider => 0.5,
            K::Turtle => 0.83,
            _ => 1.0,
        };
    }
    radius
}

fn ambient_light(dim: Dimension) -> f32 {
    match dim {
        Dimension::Overworld => 0.0,
        Dimension::Nether => 0.1,
        Dimension::End => 0.25,
    }
}

fn lightmap_brightness(level: i32, ambient: f32) -> f32 {
    let curved = crate::renderer::lightmap::get_brightness(level as f32 / 15.0);
    curved + ambient * (1.0 - curved)
}

pub(crate) fn sky_darken(dim: Dimension, day_ticks: f64) -> i32 {
    if !matches!(dim, Dimension::Overworld) {
        return 0;
    }
    let factor = crate::renderer::timeline::sample(day_ticks).sky_light_factor;
    let night = ((factor - NIGHT_FACTOR) / (1.0 - NIGHT_FACTOR)).clamp(0.0, 1.0);
    let track = NIGHT_LEVEL_TRACK + night * (1.0 - NIGHT_LEVEL_TRACK);
    (15.0 - 15.0 * track) as i32
}

fn pack_active() -> bool {
    #[cfg(feature = "shader_support")]
    {
        crate::renderer::packvertex::active()
    }
    #[cfg(not(feature = "shader_support"))]
    {
        false
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "the frame's constants, passed rather than recomputed"
)]
fn cast(
    caster: &Caster,
    camera: DVec3,
    origin: crate::renderer::RenderOrigin,
    world: &azalea_world::World,
    light: &crate::lighting::level::LightMap,
    darken: i32,
    ambient: f32,
    out: &mut Vec<Piece>,
) {
    let radius = caster.radius.min(MAX_RADIUS);
    if radius <= 0.0 {
        return;
    }
    let pos = DVec3::from_array(caster.pos);
    let pow = ((1.0 - (pos - camera).length_squared() / FADE_DISTANCE_SQ)
        * f64::from(caster.strength)) as f32;
    if pow <= 0.0 {
        return;
    }
    let r = f64::from(radius);
    let (x0, x1) = ((pos.x - r).floor() as i32, (pos.x + r).floor() as i32);
    let (z0, z1) = ((pos.z - r).floor() as i32, (pos.z + r).floor() as i32);
    let depth = (pow / 0.5 - 1.0).min(radius);
    let (y0, y1) = (
        (pos.y - f64::from(depth)).floor() as i32,
        pos.y.floor() as i32,
    );
    let at = origin.to_render_array(caster.pos);
    for z in z0..=z1 {
        for x in x0..=x1 {
            for y in y0..=y1 {
                let power = pow - (pos.y - f64::from(y)) as f32 * 0.5;
                let Some(below) = crate::util::particles::block_at(world, [x, y - 1, z]) else {
                    continue;
                };
                {
                    use azalea::physics::collision::BlockWithShape;
                    if !below.is_collision_shape_full()
                        || BlockKind::from(below) == BlockKind::Barrier
                    {
                        continue;
                    }
                }
                let (block, sky) = levels_in(light, [x, y, z]);
                let brightness = (i32::from(sky) - darken).max(i32::from(block));
                if brightness <= MIN_BRIGHTNESS {
                    continue;
                }
                let alpha =
                    (power * 0.5 * lightmap_brightness(brightness, ambient)).clamp(0.0, 1.0);
                if alpha <= 0.0 {
                    continue;
                }
                out.push(Piece {
                    at,
                    rel: [
                        (f64::from(x) - pos.x) as f32,
                        (f64::from(y) - pos.y) as f32,
                        (f64::from(z) - pos.z) as f32,
                    ],
                    radius,
                    alpha,
                });
            }
        }
    }
}

fn empty_mesh() -> Mesh {
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, Vec::<[f32; 3]>::new());
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, Vec::<[f32; 3]>::new());
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, Vec::<[f32; 2]>::new());
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, Vec::<[f32; 4]>::new());
    mesh.insert_indices(Indices::U32(Vec::new()));
    mesh
}

fn write_mesh(mesh: &mut Mesh, pieces: &[Piece]) {
    if let Some(VertexAttributeValues::Float32x3(v)) = mesh.attribute_mut(Mesh::ATTRIBUTE_POSITION)
    {
        v.clear();
        v.extend(pieces.iter().flat_map(Piece::corners));
    }
    if let Some(VertexAttributeValues::Float32x3(v)) = mesh.attribute_mut(Mesh::ATTRIBUTE_NORMAL) {
        v.clear();
        v.resize(pieces.len() * 4, [0.0, 1.0, 0.0]);
    }
    if let Some(VertexAttributeValues::Float32x2(v)) = mesh.attribute_mut(Mesh::ATTRIBUTE_UV_0) {
        v.clear();
        v.extend(pieces.iter().flat_map(Piece::uvs));
    }
    if let Some(VertexAttributeValues::Float32x4(v)) = mesh.attribute_mut(Mesh::ATTRIBUTE_COLOR) {
        v.clear();
        v.extend(pieces.iter().flat_map(|p| [[1.0, 1.0, 1.0, p.alpha]; 4]));
    }
    if let Some(Indices::U32(v)) = mesh.indices_mut() {
        v.clear();
        v.extend((0..pieces.len() as u32).flat_map(|quad| QUAD_INDICES.map(|i| quad * 4 + i)));
    }
}

fn collect_casters(
    out: &mut Vec<Caster>,
    view: &crate::renderer::FrameView,
    shared: &crate::renderer::systems::Shared,
    detached: bool,
) {
    let partial = view.partial;
    let camera = view.camera_world;
    let near =
        |pos: [f64; 3]| (DVec3::from_array(pos) - camera).length_squared() < FADE_DISTANCE_SQ;
    for anim in view.entities.iter() {
        let (base, strength) = shadow(anim.kind);
        if base <= 0.0 || anim.is_invisible() {
            continue;
        }
        let pos = anim.position(partial);
        if !near(pos) {
            continue;
        }
        let radius = shadow_radius(
            anim.kind,
            base,
            anim.scale(),
            anim.age_scale(),
            anim.shared(),
        );
        out.push(Caster {
            pos,
            radius,
            strength,
        });
    }
    let (base, strength) = shadow(EntityKind::Player);
    let mut player = |pos: [f64; 3], body: &crate::session::PlayerBody| {
        if !body.invisible && near(pos) {
            out.push(Caster {
                pos,
                radius: base * body.scale,
                strength,
            });
        }
    };
    {
        let s = shared.0.lock().unwrap();
        for p in &s.session.other_players {
            player(p.anim.position(partial), &p.body);
        }
    }
    if detached {
        player(view.player_lerp, &view.local_body);
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "one parameter per input and store it writes"
)]
pub(crate) fn sync_shadows(
    mut commands: Commands,
    view: Res<crate::renderer::FrameView>,
    shared: Res<crate::renderer::systems::Shared>,
    gui: Res<crate::gui::GuiState>,
    camera_view: Res<crate::renderer::input::CameraView>,
    origin: Res<crate::renderer::RenderOrigin>,
    mut state: ResMut<EntityShadows>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<EntityMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut entity_images: ResMut<super::EntityImages>,
    mut nodes: Query<&mut Visibility, With<ShadowNode>>,
) {
    crate::prof_span!("render:sync_shadows");
    let EntityShadows {
        node,
        casters,
        pieces,
    } = &mut *state;
    pieces.clear();
    casters.clear();
    if gui.options.entity_shadows && !pack_active() {
        collect_casters(casters, &view, &shared, camera_view.detached());
        if !casters.is_empty() {
            let world = crate::client::tracking::current_world()
                .lock()
                .unwrap()
                .clone();
            if let Some(world) = world.as_deref() {
                let world = world.read();
                let light = crate::client::worldsync::light_map().read();
                let dim = crate::renderer::dimension::current();
                let (darken, ambient) = (sky_darken(dim, view.day_ticks), ambient_light(dim));
                for caster in casters.iter() {
                    cast(
                        caster,
                        view.camera_world,
                        *origin,
                        &world,
                        &light,
                        darken,
                        ambient,
                        pieces,
                    );
                }
            }
        }
    }
    if pieces.is_empty() {
        set_visible(node.as_ref(), false, &mut nodes);
        return;
    }

    if node.is_none() {
        let Some(texture) = entity_images.get(TEXTURE, &mut images) else {
            return;
        };
        let lit = Lit::vertex_lit();
        let material = materials.add(EntityMaterial::new(
            Some(texture),
            LightMode::Text,
            AlphaMode::Blend,
            0.0,
            lit,
            DEPTH_BIAS,
        ));
        let mesh = meshes.add(empty_mesh());
        let mut spawned = commands.spawn((
            Mesh3d(mesh.clone()),
            MeshMaterial3d(material),
            Transform::default(),
            Visibility::Inherited,
            NoFrustumCulling,
            ShadowNode,
            Name::new("entity shadows"),
        ));
        #[cfg(feature = "builtin_shaders")]
        spawned.insert(bevy::light::NotShadowCaster);
        *node = Some((spawned.id(), mesh));
    }
    if let Some((_, handle)) = node.as_ref()
        && let Some(mesh) = meshes.get_mut(handle)
    {
        write_mesh(mesh, pieces);
    }
    set_visible(node.as_ref(), true, &mut nodes);
}

fn set_visible(
    node: Option<&(Entity, Handle<Mesh>)>,
    visible: bool,
    nodes: &mut Query<&mut Visibility, With<ShadowNode>>,
) {
    if let Some((entity, _)) = node
        && let Ok(mut vis) = nodes.get_mut(*entity)
    {
        vis.set_if_neq(if visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
    }
}

pub(crate) fn enabled(gui: Res<crate::gui::GuiState>) -> bool {
    gui.options.entity_shadows
}

pub(crate) fn hide_shadows(
    state: Res<EntityShadows>,
    mut nodes: Query<&mut Visibility, With<ShadowNode>>,
) {
    set_visible(state.node.as_ref(), false, &mut nodes);
}
