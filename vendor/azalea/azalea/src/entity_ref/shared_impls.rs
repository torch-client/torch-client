use azalea_core::{entity_id::MinecraftEntityId, position::Vec3};
use azalea_entity::{
    Attributes, Dead, Physics, Position, dimensions::EntityDimensions, metadata::Health,
};
use azalea_world::WorldName;

use super::EntityRef;
use crate::{Client, client_impl::error::AzaleaResult};

macro_rules! impl_entity_functions {
    ( $(
        Client:
        $(#[$client_doc:meta])*
        EntityRef:
        $(#[$entityref_doc:meta])*
        pub fn $fn_name:ident (&$fn_self:ident) -> $fn_returns:ty $fn_impl:block
    )* ) => {
        $(
            impl Client {
                $(#[$client_doc])*
                pub fn $fn_name(&$fn_self) -> $fn_returns $fn_impl
            }
            impl EntityRef {
                $(#[$entityref_doc])*
                pub fn $fn_name(&$fn_self) -> $fn_returns $fn_impl
            }
        )*
    };
}

impl_entity_functions! {
    Client:
    EntityRef:
    pub fn position(&self) -> AzaleaResult<Vec3> {
        Ok(**self.component::<Position>()?)
    }

    Client:
    EntityRef:
    pub fn dimensions(&self) -> AzaleaResult<EntityDimensions> {
        Ok(self.component::<EntityDimensions>()?.clone())
    }

    Client:
    EntityRef:
    pub fn eye_position(&self) -> AzaleaResult<Vec3> {
        self.query_self::<(&Position, &EntityDimensions), _>(|(pos, dim)| {
            pos.up(dim.eye_height as f64)
        })
    }

    Client:
    EntityRef:
    pub fn health(&self) -> AzaleaResult<f32> {
        Ok(**self.component::<Health>()?)
    }

    Client:
    EntityRef:
    pub fn minecraft_id(&self) -> AzaleaResult<MinecraftEntityId> {
        Ok(*self.component::<MinecraftEntityId>()?)
    }

    Client:
    EntityRef:
    pub fn attributes(&self) -> AzaleaResult<Attributes> {
        Ok(self.component::<Attributes>()?.clone())
    }

    Client:
    #[doc(hidden)]
    #[deprecated = "renamed to `world_name`."]
    EntityRef:
    #[doc(hidden)]
    #[deprecated = "renamed to `world_name`."]
    pub fn instance_name(&self) -> AzaleaResult<WorldName> {
        self.world_name()
    }

    Client:
    #[doc(alias("dimension_name"))]
    EntityRef:
    #[doc(alias("dimension_name"))]
    pub fn world_name(&self) -> AzaleaResult<WorldName> {
        Ok((*self.component::<WorldName>()?).clone())
    }

    Client:
    EntityRef:
    pub fn is_alive(&self) -> bool {
        self.query_self::<Option<&Dead>, _>(|dead| dead.is_none()).unwrap_or(false)
    }

    Client:
    EntityRef:
    pub fn exists(&self) -> bool {
        self.query_self::<Option<&MinecraftEntityId>, _>(|entity_id| entity_id.is_some()).unwrap_or(false)
    }

    Client:
    EntityRef:
    pub fn physics(&self) -> AzaleaResult<Physics> {
        Ok(self.component::<Physics>()?.clone())
    }
}
