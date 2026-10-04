use azalea_registry::builtin::EntityKind;

use crate::entities::TexturePath;
use crate::entities::models::monsters::spider;
use crate::entities::registry::Registry;
use crate::entities::state::EntityState;
use crate::entities::{Blend, RenderSpec, RootPose, setup_rotations};

pub fn register(registry: &mut Registry) {
    registry.add(
        EntityKind::Spider,
        RenderSpec::new("spider", spider::layer, texture, spider::setup_anim).with_root(root),
    );
    registry.add(
        EntityKind::Spider,
        RenderSpec::new(
            "spider_eyes",
            spider::layer,
            eyes_texture,
            spider::setup_anim,
        )
        .with_root(root)
        .with_blend(Blend::Additive),
    );
    registry.add(
        EntityKind::CaveSpider,
        RenderSpec::new(
            "cave_spider",
            spider::cave_spider_layer,
            cave_spider_texture,
            spider::setup_anim,
        )
        .with_root(root),
    );
    registry.add(
        EntityKind::CaveSpider,
        RenderSpec::new(
            "cave_spider_eyes",
            spider::cave_spider_layer,
            eyes_texture,
            spider::setup_anim,
        )
        .with_root(root)
        .with_blend(Blend::Additive),
    );
}

fn texture(_st: &EntityState) -> TexturePath {
    "entity/spider/spider".into()
}

fn cave_spider_texture(_st: &EntityState) -> TexturePath {
    "entity/spider/cave_spider".into()
}

fn eyes_texture(_st: &EntityState) -> TexturePath {
    "entity/spider/spider_eyes".into()
}

fn root(st: &EntityState) -> RootPose {
    setup_rotations(st, 180.0)
}
