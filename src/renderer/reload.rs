use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, AtomicU64, Ordering};

use bevy::prelude::*;
use parking_lot::Mutex;

use crate::blocks::{self, Blocks};
use crate::entities::BlockRef;
use crate::gui::atlas::GuiAtlas;
use crate::platform::time::Instant;
use crate::renderer::AtlasTables;
use crate::renderer::item_assets::{ItemArgs, ItemAssets, ItemParts};
use crate::util::particle_assets::ParticleAtlasData;

pub(crate) struct Atlases {
    block_image: Image,
    blocks: Blocks,
    tables: AtlasTables,
    gui: GuiAtlas,
}

pub(crate) fn build_atlases() -> Atlases {
    let started = Instant::now();
    let gui_work = || {
        let t = Instant::now();
        let gui = crate::gui::atlas::build_gui_atlas(crate::assets_root());
        crate::log_info!("assets", "gui atlas + item icons: {:.0} ms", ms(t));
        gui
    };
    let block_work = || {
        let t = Instant::now();
        let textures_dir = crate::textures_dir().to_string_lossy().into_owned();
        let textures = crate::renderer::Textures::blocks(&textures_dir);
        let decoded = ms(t);
        let atlas = crate::renderer::build_block_atlas(&textures, &textures_dir);
        let blocks = Blocks::load(atlas.sprites);
        crate::log_info!(
            "assets",
            "block atlas: {:.0} ms ({decoded:.0} ms decoding {} block sprites)",
            ms(t),
            textures.block.len()
        );
        (atlas.image, atlas.tables, blocks)
    };
    #[cfg(target_arch = "wasm32")]
    let (gui, (block_image, tables, blocks)) = (gui_work(), block_work());
    #[cfg(not(target_arch = "wasm32"))]
    let (gui, (block_image, tables, blocks)) = std::thread::scope(|scope| {
        let gui = scope.spawn(gui_work);
        let block = block_work();
        (gui.join().expect("the gui atlas thread panicked"), block)
    });
    crate::log_info!("assets", "atlases ready in {:.0} ms", ms(started));
    Atlases {
        block_image,
        blocks,
        tables,
        gui,
    }
}

impl Atlases {
    pub(crate) fn install(self) -> (Image, GuiAtlas) {
        self.tables.publish();
        blocks::install(self.blocks);
        (self.block_image, self.gui)
    }
}

fn ms(since: Instant) -> f32 {
    since.elapsed().as_secs_f32() * 1000.0
}

#[derive(Default)]
struct Inputs {
    images: Vec<(AssetId<Image>, Arc<Decode>)>,
    items: Vec<ItemArgs>,
    block_meshes: Vec<(BlockRef, AssetId<Mesh>)>,
    particles: bool,
}

struct Prepared {
    atlases: Atlases,
    images: Vec<(AssetId<Image>, image::RgbaImage)>,
    items: (
        crate::items::mesh::ItemMeshes,
        Vec<(ItemArgs, Option<ItemParts>)>,
    ),
    block_meshes: Vec<(BlockRef, AssetId<Mesh>, Option<Mesh>)>,
    particles: Option<ParticleAtlasData>,
    ticket: Option<u64>,
    started: Instant,
}

fn build(inputs: Inputs, ticket: Option<u64>) -> Prepared {
    let started = Instant::now();
    #[cfg(resource_packs)]
    crate::resourcepacks::mount();
    reset_lazy_caches();

    let Inputs {
        images,
        items,
        block_meshes,
        particles,
    } = inputs;
    let atlases = build_atlases();
    let images = images
        .into_iter()
        .filter_map(|(id, decode)| Some((id, decode()?)))
        .collect();
    let builder = crate::items::mesh::ItemMeshes::new(atlases.blocks.assets().clone());
    let items = ItemAssets::rebuild(&builder, items);
    let block_meshes = block_meshes
        .into_iter()
        .map(|(block, id)| {
            (
                block,
                id,
                crate::entities::blockmodel::mesh_in(&atlases.blocks, block),
            )
        })
        .collect();
    let particles = particles.then(crate::util::particle_assets::build_atlas);
    Prepared {
        atlases,
        images,
        items: (builder, items),
        block_meshes,
        particles,
        ticket,
        started,
    }
}

fn reset_lazy_caches() {
    crate::gui::tooltip::reset_lang();
    crate::text::texts::reset();
    crate::util::particle_assets::reset();
    crate::entities::render::humanoid::armor::reset();
    crate::entities::reset_sheet_animations();
    crate::gui::atlas::reset_sga();
}

type Decode = dyn Fn() -> Option<image::RgbaImage> + Send + Sync;

static TRACKED: std::sync::LazyLock<Mutex<HashMap<AssetId<Image>, Arc<Decode>>>> =
    std::sync::LazyLock::new(Default::default);

pub(crate) fn track(
    handle: &Handle<Image>,
    decode: impl Fn() -> Option<image::RgbaImage> + Send + Sync + 'static,
) {
    TRACKED.lock().insert(handle.id(), Arc::new(decode));
}

fn refill(images: &mut Assets<Image>, id: AssetId<Image>, pixels: image::RgbaImage) {
    use bevy::render::render_resource::{Extent3d, TextureDimension};
    let Some(old) = images.get(id) else { return };
    let size = Extent3d {
        width: pixels.width(),
        height: pixels.height(),
        depth_or_array_layers: 1,
    };
    let mut image = Image::new(
        size,
        TextureDimension::D2,
        pixels.into_raw(),
        old.texture_descriptor.format,
        old.asset_usage,
    );
    image.sampler = old.sampler.clone();
    let _ = images.insert(id, image);
}

static GENERATION: AtomicU32 = AtomicU32::new(0);

pub(crate) fn generation() -> u32 {
    GENERATION.load(Ordering::Relaxed)
}

static REQUESTED: AtomicBool = AtomicBool::new(false);
static BUILDING: AtomicBool = AtomicBool::new(false);
static READY: AtomicBool = AtomicBool::new(false);
static PREPARED: Mutex<Option<Prepared>> = Mutex::new(None);
static TICKETS: AtomicU64 = AtomicU64::new(0);
static COVERED: AtomicU64 = AtomicU64::new(0);

pub(crate) fn request() -> u64 {
    let ticket = TICKETS.fetch_add(1, Ordering::AcqRel) + 1;
    REQUESTED.store(true, Ordering::Release);
    ticket
}

pub(crate) fn covers(ticket: u64) -> bool {
    COVERED.load(Ordering::Acquire) >= ticket
}

pub(crate) fn install_downloaded() {
    finish(build(Inputs::default(), None));
}

fn finish(prepared: Prepared) {
    *PREPARED.lock() = Some(prepared);
    READY.store(true, Ordering::Release);
}

struct BuildGuard {
    #[cfg(resource_packs)]
    previous: Vec<Arc<crate::platform::assets::archive::OwnedLayer>>,
}

impl Drop for BuildGuard {
    fn drop(&mut self) {
        if !std::thread::panicking() {
            return;
        }
        crate::log_error!(
            "resourcepacks",
            "the reload failed; the previous packs stay"
        );
        #[cfg(resource_packs)]
        crate::platform::assets::archive::set_selected(std::mem::take(&mut self.previous));
        reset_lazy_caches();
        BUILDING.store(false, Ordering::Release);
    }
}

fn start(world: &mut World) {
    if BUILDING.load(Ordering::Acquire)
        || READY.load(Ordering::Acquire)
        || !world.contains_resource::<crate::renderer::systems::AssetsReady>()
        || !REQUESTED.swap(false, Ordering::AcqRel)
    {
        return;
    }
    let ticket = TICKETS.load(Ordering::Acquire);
    let mut inputs = Inputs::default();
    {
        let images = world.resource::<Assets<Image>>();
        let mut tracked = TRACKED.lock();
        tracked.retain(|id, _| images.contains(*id));
        inputs.images = tracked
            .iter()
            .map(|(id, decode)| (*id, decode.clone()))
            .collect();
    }
    if let Some(mut items) = world.get_resource_mut::<ItemAssets>() {
        inputs.items = items.reload_args();
    }
    if let Some(mut entities) = world.get_resource_mut::<crate::entities::EntityAssets>() {
        inputs.block_meshes = entities.block_mesh_keys();
    }
    inputs.particles = world
        .get_resource::<crate::util::particles::ParticleAtlas>()
        .is_some_and(|a| a.is_built());

    BUILDING.store(true, Ordering::Release);
    crate::log_info!("resourcepacks", "reloading resources");
    let work = move || {
        let _guard = BuildGuard {
            #[cfg(resource_packs)]
            previous: crate::platform::assets::archive::selected(),
        };
        finish(build(inputs, Some(ticket)));
    };
    #[cfg(target_arch = "wasm32")]
    work();
    #[cfg(not(target_arch = "wasm32"))]
    if let Err(e) = std::thread::Builder::new()
        .name("asset-reload".into())
        .spawn(work)
    {
        crate::log_error!("resourcepacks", "could not start the reload: {e}");
        BUILDING.store(false, Ordering::Release);
    }
}

fn adopt(world: &mut World) {
    let Some(prepared) = PREPARED.lock().take() else {
        return;
    };
    READY.store(false, Ordering::Release);
    BUILDING.store(false, Ordering::Release);
    let swap = Instant::now();
    let Prepared {
        atlases,
        images: decoded,
        items,
        block_meshes,
        particles,
        ticket,
        started,
    } = prepared;
    let Atlases {
        block_image,
        blocks: registry,
        tables,
        gui,
    } = atlases;
    tables.publish();
    blocks::install(registry);

    world.resource_scope(|world, mut images: Mut<Assets<Image>>| {
        if let Some(block) = world.get_resource::<crate::renderer::systems::BlockAtlasHandle>() {
            let _ = images.insert(&block.0, block_image);
        }
        if let Some(mut assets) = world.get_resource_mut::<crate::gui::render::GuiAssets>() {
            let (image, unihex) = crate::gui::render::atlas_textures(&gui);
            let _ = images.insert(&assets.image, image);
            let _ = images.insert(&assets.unihex_image, unihex);
            assets.atlas = Arc::new(gui);
        }
        for (id, pixels) in decoded {
            refill(&mut images, id, pixels);
        }
        world.resource_scope(|world, mut meshes: Mut<Assets<Mesh>>| {
            let (builder, rebuilt) = items;
            if let Some(mut items) = world.get_resource_mut::<ItemAssets>() {
                items.adopt(builder, rebuilt, &mut meshes, &mut images);
            }
            if let Some(mut entities) = world.get_resource_mut::<crate::entities::EntityAssets>() {
                for (block, id, mesh) in block_meshes {
                    match mesh {
                        Some(mesh) => {
                            let _ = meshes.insert(id, mesh);
                        }
                        None => entities.forget_block_mesh(&block),
                    }
                }
            }
        });
        if let (Some(built), Some(mut atlas)) = (
            particles,
            world.get_resource_mut::<crate::util::particles::ParticleAtlas>(),
        ) {
            atlas.adopt(built, &mut images);
        }
    });

    if let Some(mut writes) = world.get_resource_mut::<crate::gui::atlas_writes::AtlasWrites>() {
        writes.replaced();
    }
    crate::gui::ping::requeue_all();
    crate::gui::banner_icons::reset();
    #[cfg(feature = "skins")]
    if let Some(mut faces) = world.get_resource_mut::<crate::gui::player_faces::PlayerFaces>() {
        faces.reset();
    }
    if let Some(mut overlay) = world.get_resource_mut::<crate::renderer::overlays::BreakOverlay>() {
        overlay.forget_shown();
    }
    GENERATION.fetch_add(1, Ordering::Relaxed);
    TOUCH.store(2, Ordering::Relaxed);
    world.write_message(AssetsReloaded);

    #[cfg(all(resource_packs, target_arch = "wasm32"))]
    crate::client::mesh_worker::forward_packs();
    if let Some(shared) = world.get_resource::<crate::renderer::systems::Shared>() {
        shared.0.lock().unwrap().remesh_chunks_requested = true;
    }
    match ticket {
        Some(ticket) => COVERED.store(ticket, Ordering::Release),
        None => finish_first_install(world),
    }
    crate::log_info!(
        "resourcepacks",
        "resources reloaded in {:.0} ms ({:.1} ms on the main thread)",
        ms(started),
        ms(swap)
    );
}

fn finish_first_install(world: &mut World) {
    world.insert_resource(crate::renderer::systems::AssetsReady);
    let next = world
        .resource::<crate::gui::render::AfterDownloadScreen>()
        .0;
    world.resource_mut::<crate::gui::GuiState>().screen = next;
    crate::log_info!("assets", "the downloaded set is live");
}

#[derive(Message)]
pub(crate) struct AssetsReloaded;

static TOUCH: AtomicU8 = AtomicU8::new(0);

fn touch_due() -> bool {
    TOUCH.load(Ordering::Relaxed) == 1
}

fn count_down_touch() {
    let _ = TOUCH.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_sub(1));
}

fn touch<M: Asset>(mut materials: ResMut<Assets<M>>) {
    for _ in materials.iter_mut() {}
}

pub(crate) fn touch_on_reload<M: Asset>(app: &mut App) {
    app.add_systems(Update, touch::<M>.run_if(touch_due));
}

pub struct ReloadPlugin;

impl Plugin for ReloadPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<AssetsReloaded>()
            .add_systems(
                Update,
                (
                    start.run_if(|| REQUESTED.load(Ordering::Relaxed)),
                    adopt.run_if(|| READY.load(Ordering::Relaxed)),
                ),
            )
            .add_systems(Last, count_down_touch);
        touch_on_reload::<StandardMaterial>(app);
        #[cfg(resource_packs)]
        app.add_systems(Update, super::input::reload_resource_packs);
    }
}
