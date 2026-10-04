pub mod anim;
mod atlas;
pub mod clouds;
pub mod dimension;
pub mod entity_material;
pub mod environment;
pub mod esp_material;
pub mod frame_view;
pub mod hand;
pub mod input;
pub mod intent;
pub mod item_assets;
pub mod lighter;
pub mod lightmap;
pub mod maps;
mod mesh;
pub mod occlusion;
mod occupancy;
pub mod overlays;
#[cfg(feature = "shader_support")]
pub mod packdraw;
#[cfg(feature = "shader_support")]
pub mod packvertex;
pub mod panorama;
pub mod player_model;
pub mod post;
mod screenshot;
pub mod skin;
pub mod sky;
pub mod ssaa;
pub mod systems;
pub mod terrain;
pub mod terrain_pool;
pub mod timeline;
pub mod visgraph;
pub mod world_text;

pub use anim::{AnimInput, Armor, HeldItem, HumanoidAnim};
pub use atlas::{Textures, build_block_atlas, placeholder_atlas, texture_is_opaque};
pub use frame_view::{FrameView, FrameViewSystems};
pub use mesh::{
    MeshBuf, build_section_mesh, lighting_enabled, set_lighting_enabled, set_smooth_lighting,
    smooth_lighting_enabled,
};
pub use occupancy::Occupancy;
pub use systems::{Shared, run};

pub const ATLAS_COLS: u32 = 16;
pub const TILE_PX: u32 = 16;

pub fn atlas_rows(atlas: &bevy::image::Image) -> u32 {
    (atlas.height() * ATLAS_COLS / atlas.width().max(1)).max(1)
}

pub(crate) fn rgba_image(
    width: u32,
    height: u32,
    data: Vec<u8>,
    usage: bevy::asset::RenderAssetUsages,
) -> bevy::image::Image {
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

    let mut image = bevy::image::Image::new(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        usage,
    );
    image.sampler = bevy::image::ImageSampler::nearest();
    image
}

#[derive(Clone, Debug)]
pub enum BlockGeom {
    Model(std::sync::Arc<crate::blocks::BakedBlock>),
    Fluid {
        amount: u8,
        still: u32,
        flow: u32,
        lava: bool,
    },
}

pub fn fluid_surface(amount: u8) -> f32 {
    amount as f32 / 9.0
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum TintKind {
    #[default]
    None,
    Grass,
    Foliage,
    DryFoliage,
    Redstone(u8),
    Water,
}

#[derive(Clone)]
pub struct RenderedBlock {
    pub geom: BlockGeom,
    pub is_solid: bool,
    pub waterlogged: bool,
    pub full_cube: bool,
    pub emission: u8,
    pub emissive: bool,
    pub cull_group: u32,
    pub tint_kind: TintKind,
    pub offset: crate::blocks::rand::ShapeOffset,
    #[cfg(feature = "builtin_shaders")]
    pub sways: bool,
    #[cfg(feature = "shader_support")]
    pub state: u16,
    #[cfg(feature = "shader_support")]
    pub translucent: bool,
}

pub struct PendingSection {
    pub chunk_x: i32,
    pub chunk_z: i32,
    pub sec_y: i32,
    pub opaque: MeshBuf,
    pub water: MeshBuf,
    pub vis: visgraph::VisibilitySet,
}

#[derive(bevy::prelude::States, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AppState {
    #[default]
    Menu,
    InGame,
}
