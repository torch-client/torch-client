use bevy_color::Color;
use bevy_derive::{Deref, DerefMut};
use bevy_ecs::prelude::*;
use bevy_reflect::prelude::*;
use derive_more::derive::From;
use serde::{Deserialize, Serialize};

#[derive(Reflect, Serialize, Deserialize, Copy, Clone, Debug, Default, From)]
#[reflect(Serialize, Deserialize, Default, Clone)]
pub enum ClearColorConfig {
    #[default]
    Default,
    Custom(Color),
    None,
}

#[derive(Reflect, Serialize, Deserialize, Copy, Clone, Debug, Default, PartialEq, Eq)]
#[reflect(Serialize, Deserialize, Default, Clone)]
pub enum MsaaWriteback {
    Off,
    #[default]
    Auto,
    Always,
}

#[derive(Resource, Clone, Debug, Deref, DerefMut, Reflect)]
#[reflect(Resource, Default, Debug, Clone)]
pub struct ClearColor(pub Color);

impl Default for ClearColor {
    fn default() -> Self {
        Self(Color::srgb_u8(43, 44, 47))
    }
}
