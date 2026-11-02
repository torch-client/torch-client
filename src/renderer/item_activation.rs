use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use super::hand::{
    Chain, HAND_LAYER, HandMaterial, HandRoot, apply_display_transform, cutout_material,
};
use crate::items::mesh::Transform as ItemTransform;
use crate::items::model::DisplayContext;
use crate::renderer::HeldItem;
use crate::renderer::frame_view::{FrameView, FrameViewSystems};
use crate::renderer::item_assets::ItemAssets;
use crate::util::javarandom::JavaRandom;

const LENGTH_TICKS: f32 = 40.0;
const SWING_SCALE: f32 = 0.3;
const START_Z: f32 = -10.0;
const APPROACH_Z: f32 = 9.0;
const SIZE: f32 = 0.8;
const SPIN_DEGREES: f32 = 900.0;
const WOBBLE_DEGREES: f32 = 6.0;
const WOBBLE_RATE: f32 = 8.0;

fn pose(progress: f32, swing: [f32; 2], aspect: f32) -> Chain {
    let pi_scale = ease(progress) * std::f32::consts::PI;
    let swing_out = (pi_scale * 2.0).sin().abs();
    let mut chain = Chain::new();
    chain.translate(
        swing[0] * SWING_SCALE * aspect * swing_out,
        swing[1] * SWING_SCALE * swing_out,
        START_Z + APPROACH_Z * pi_scale.sin(),
    );
    chain.scale(SIZE, SIZE, SIZE);
    chain.rot_y(SPIN_DEGREES * pi_scale.sin().abs());
    let wobble = WOBBLE_DEGREES * (progress * WOBBLE_RATE).cos();
    chain.rot_x(wobble);
    chain.rot_z(wobble);
    chain
}

fn ease(t: f32) -> f32 {
    let t2 = t * t;
    let t3 = t * t2;
    10.25 * t3 * t2 - 24.95 * t2 * t2 + 25.5 * t3 - 13.8 * t2 + 4.0 * t
}

struct Running {
    item: HeldItem,
    start: f32,
    swing: [f32; 2],
}

struct Nodes {
    pose: Entity,
    item: Entity,
    material: Handle<HandMaterial>,
}

struct Uploaded {
    item: HeldItem,
    display: Option<ItemTransform>,
}

#[derive(Resource)]
struct ItemActivationState {
    seen: u32,
    running: Option<Running>,
    nodes: Option<Nodes>,
    uploaded: Option<Uploaded>,
    rng: JavaRandom,
}

impl Default for ItemActivationState {
    fn default() -> Self {
        ItemActivationState {
            seen: 0,
            running: None,
            nodes: None,
            uploaded: None,
            rng: JavaRandom::time_seeded(),
        }
    }
}

#[derive(Component)]
struct ActivationNode;

#[derive(Component)]
struct ActivationItemNode;

pub struct ItemActivationPlugin;

impl Plugin for ItemActivationPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ItemActivationState>().add_systems(
            Update,
            sync_item_activation
                .after(FrameViewSystems)
                .run_if(in_state(crate::renderer::AppState::InGame)),
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn sync_item_activation(
    mut commands: Commands,
    view: Res<FrameView>,
    gui: Res<crate::gui::GuiState>,
    window: Query<&Window, With<PrimaryWindow>>,
    root: Query<Entity, With<HandRoot>>,
    mut state: ResMut<ItemActivationState>,
    mut item_assets: ResMut<ItemAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<HandMaterial>>,
    mut placed: Query<(&mut Transform, &mut Visibility), With<ActivationNode>>,
    mut item_mesh: Query<&mut Mesh3d, With<ActivationItemNode>>,
) {
    let state = &mut *state;
    let now = view.local.age_ticks;
    match &view.item_activation {
        None => state.running = None,
        Some(activation) if activation.serial != state.seen => {
            state.seen = activation.serial;
            let swing = [
                state.rng.next_f32() * 2.0 - 1.0,
                state.rng.next_f32() * 2.0 - 1.0,
            ];
            state.running = Some(Running {
                item: activation.item.clone(),
                start: now.floor(),
                swing,
            });
        }
        Some(_) => {}
    }
    let progress = state
        .running
        .as_ref()
        .map(|r| (now - r.start) / LENGTH_TICKS);
    if progress.is_some_and(|p| !(0.0..1.0).contains(&p)) {
        state.running = None;
    }

    let Some(nodes) = &state.nodes else {
        if state.running.is_some()
            && let Ok(root) = root.single()
        {
            state.nodes = Some(spawn_nodes(&mut commands, root, &mut materials));
        }
        return;
    };
    let shown = match (&state.running, progress) {
        (Some(running), Some(progress)) if !gui.hide_gui => Some((running, progress)),
        _ => None,
    };
    let Some((running, progress)) = shown else {
        hide(&mut placed, nodes.pose);
        return;
    };

    let cached = state
        .uploaded
        .as_ref()
        .filter(|up| up.item.same_model(&running.item))
        .map(|up| up.display);
    let display = match cached {
        Some(display) => display,
        None => {
            let gpu = item_assets.get(
                &running.item.model_key(),
                DisplayContext::Fixed.name(),
                false,
                &mut meshes,
                &mut images,
            );
            let display = gpu.map(|gpu| gpu.for_context(DisplayContext::Fixed));
            if let Some(gpu) = gpu {
                let Ok(mut mesh) = item_mesh.get_mut(nodes.item) else {
                    return;
                };
                mesh.0 = gpu.mesh.clone();
                if let Some(material) = materials.get_mut(&nodes.material) {
                    material.base.base_color_texture = Some(gpu.texture.clone());
                }
            }
            state.uploaded = Some(Uploaded {
                item: running.item.clone(),
                display,
            });
            display
        }
    };
    let Some(display) = display else {
        hide(&mut placed, nodes.pose);
        return;
    };

    let aspect = window
        .single()
        .map_or(1.0, |w| w.width() / w.height().max(1.0));
    let mut chain = pose(progress, running.swing, aspect);
    apply_display_transform(&mut chain, &display, false);
    let (pose_transform, item_transform) = chain.transforms();
    for (entity, transform) in [(nodes.pose, pose_transform), (nodes.item, item_transform)] {
        if let Ok((mut current, mut visibility)) = placed.get_mut(entity) {
            *current = transform;
            visibility.set_if_neq(Visibility::Inherited);
        }
    }
}

fn spawn_nodes(
    commands: &mut Commands,
    root: Entity,
    materials: &mut Assets<HandMaterial>,
) -> Nodes {
    let material = materials.add(cutout_material(None));
    let pose = commands
        .spawn((
            Transform::default(),
            Visibility::Hidden,
            ActivationNode,
            Name::new("item_activation_pose"),
        ))
        .id();
    let item = commands
        .spawn((
            Mesh3d::default(),
            MeshMaterial3d(material.clone()),
            Transform::default(),
            Visibility::Inherited,
            RenderLayers::layer(HAND_LAYER),
            NoFrustumCulling,
            NotShadowCaster,
            ActivationNode,
            ActivationItemNode,
            Name::new("item_activation_item"),
        ))
        .id();
    commands.entity(pose).add_child(item);
    commands.entity(root).add_child(pose);
    Nodes {
        pose,
        item,
        material,
    }
}

fn hide(
    placed: &mut Query<(&mut Transform, &mut Visibility), With<ActivationNode>>,
    entity: Entity,
) {
    if let Ok((_, mut visibility)) = placed.get_mut(entity) {
        visibility.set_if_neq(Visibility::Hidden);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn origin(chain: &Chain) -> Vec3 {
        let (parent, _) = chain.transforms();
        parent.translation
    }

    #[test]
    fn ease_runs_from_zero_to_one() {
        assert!(ease(0.0).abs() < 1e-6);
        assert!((ease(1.0) - 1.0).abs() < 1e-4);
    }

    #[test]
    fn starts_and_ends_far_and_centred() {
        for progress in [0.0, 1.0] {
            let at = origin(&pose(progress, [1.0, -1.0], 16.0 / 9.0));
            assert!((at.z - START_Z).abs() < 1e-3, "z {} at {progress}", at.z);
            assert!(at.x.abs() < 1e-3 && at.y.abs() < 1e-3, "{at} at {progress}");
        }
    }

    #[test]
    fn comes_close_to_the_camera_on_the_way() {
        let nearest = (1..40)
            .map(|tick| origin(&pose(tick as f32 / LENGTH_TICKS, [0.0, 0.0], 1.0)).z)
            .fold(f32::NEG_INFINITY, f32::max);
        assert!(nearest > -1.1 && nearest < -0.9, "nearest z {nearest}");
    }

    #[test]
    fn swing_scales_with_aspect_on_x_only() {
        let progress = 0.1;
        let narrow = origin(&pose(progress, [1.0, 1.0], 1.0));
        let wide = origin(&pose(progress, [1.0, 1.0], 2.0));
        assert!((wide.x - 2.0 * narrow.x).abs() < 1e-5);
        assert!((wide.y - narrow.y).abs() < 1e-6);
    }
}
