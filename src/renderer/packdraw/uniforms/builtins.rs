use bevy::math::{IVec2, IVec3, Mat4, Vec3, Vec4};

use super::values::FrameValues;
use crate::shaderpack::expressions::Resolved;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Value {
    Float(f32),
    Int(i32),
    Vec3(Vec3),
    Vec4(Vec4),
    IVec2(IVec2),
    IVec3(IVec3),
    Mat4(Mat4),
}

impl Value {
    pub(super) fn write(self, out: &mut [u8]) {
        let mut put = |bytes: &[u8]| out[..bytes.len()].copy_from_slice(bytes);
        match self {
            Value::Float(v) => put(bytemuck::bytes_of(&v)),
            Value::Int(v) => put(bytemuck::bytes_of(&v)),
            Value::Vec3(v) => put(bytemuck::bytes_of(&v)),
            Value::Vec4(v) => put(bytemuck::bytes_of(&v)),
            Value::IVec2(v) => put(bytemuck::bytes_of(&v)),
            Value::IVec3(v) => put(bytemuck::bytes_of(&v)),
            Value::Mat4(v) => put(bytemuck::bytes_of(&v)),
        }
    }

    fn components(self) -> Option<usize> {
        match self {
            Value::Float(_) | Value::Int(_) => None,
            Value::IVec2(_) => Some(2),
            Value::Vec3(_) | Value::IVec3(_) => Some(3),
            Value::Vec4(_) => Some(4),
            Value::Mat4(_) => Some(0),
        }
    }

    pub(super) fn has(self, index: Option<usize>) -> bool {
        match (self.components(), index) {
            (None, None) => true,
            (Some(n), Some(i)) => i < n,
            _ => false,
        }
    }

    pub(super) fn component(self, index: Option<usize>) -> f32 {
        if !self.has(index) {
            return 0.0;
        }
        let i = index.unwrap_or(0);
        match self {
            Value::Float(v) => v,
            Value::Int(v) => v as f32,
            Value::Vec3(v) => v[i],
            Value::Vec4(v) => v[i],
            Value::IVec2(v) => v[i] as f32,
            Value::IVec3(v) => v[i] as f32,
            Value::Mat4(_) => 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Builtin {
    ModelView,
    ModelViewInverse,
    Projection,
    ProjectionInverse,
    PreviousModelView,
    PreviousProjection,
    CameraPosition,
    PreviousCameraPosition,
    CameraPositionInt,
    CameraPositionFract,
    PreviousCameraPositionInt,
    PreviousCameraPositionFract,
    EyeAltitude,
    RelativeEyePosition,
    ViewWidth,
    ViewHeight,
    AspectRatio,
    Near,
    Far,
    FrameTimeCounter,
    FrameTime,
    FrameCounter,
    WorldTime,
    WorldDay,
    MoonPhase,
    SunAngle,
    ShadowAngle,
    SunPosition,
    MoonPosition,
    ShadowLightPosition,
    UpPosition,
    SkyColor,
    FogColor,
    RainStrength,
    Wetness,
    ThunderStrength,
    EyeBrightness,
    EyeBrightnessSmooth,
    IsEyeInWater,
    Blindness,
    NightVision,
    DarknessFactor,
    HideGui,
    HeldItemId,
    HeldItemId2,
    HeldBlockLightValue,
    HeldBlockLightValue2,
    HeldBlockLightColor,
    FogStart,
    FogEnd,
    ScreenBrightness,
    PlayerBodyVector,
    Rainfall,
    Temperature,
    BiomePrecipitation,
    RenderStage,
    LightningBoltPosition,
    NoId,
    NoEntityColor,
    AtlasSize,
    Biome,
    ShadowModelView,
    ShadowModelViewInverse,
    ShadowProjection,
    ShadowProjectionInverse,
    BedrockLevel,
    HeightLimit,
    LogicalHeightLimit,
    SeaLevel,
    HasCeiling,
    HasSkylight,
    CurrentPlayerHealth,
    MaxPlayerHealth,
    CurrentPlayerHunger,
    MaxPlayerHunger,
    CurrentPlayerAir,
    MaxPlayerAir,
    CurrentPlayerArmor,
    MaxPlayerArmor,
    IsSpectator,
    FirstPersonCamera,
    IsRiding,
    IsElytraFlying,
    InSwimmingAnimation,
    FeetInWater,
    EyePosition,
    PlayerLookVector,
    VehicleId,
    VehicleInWater,
    VehicleLookVector,
    RelativeVehiclePosition,
}

pub(crate) const BUILTINS: &[(&str, &str, Builtin)] = &[
    ("gbufferModelView", "mat4", Builtin::ModelView),
    ("gbufferModelViewInverse", "mat4", Builtin::ModelViewInverse),
    ("gbufferProjection", "mat4", Builtin::Projection),
    (
        "gbufferProjectionInverse",
        "mat4",
        Builtin::ProjectionInverse,
    ),
    (
        "gbufferPreviousModelView",
        "mat4",
        Builtin::PreviousModelView,
    ),
    (
        "gbufferPreviousProjection",
        "mat4",
        Builtin::PreviousProjection,
    ),
    ("cameraPosition", "vec3", Builtin::CameraPosition),
    (
        "previousCameraPosition",
        "vec3",
        Builtin::PreviousCameraPosition,
    ),
    ("cameraPositionInt", "ivec3", Builtin::CameraPositionInt),
    ("cameraPositionFract", "vec3", Builtin::CameraPositionFract),
    (
        "previousCameraPositionInt",
        "ivec3",
        Builtin::PreviousCameraPositionInt,
    ),
    (
        "previousCameraPositionFract",
        "vec3",
        Builtin::PreviousCameraPositionFract,
    ),
    ("eyeAltitude", "float", Builtin::EyeAltitude),
    ("relativeEyePosition", "vec3", Builtin::RelativeEyePosition),
    ("viewWidth", "float", Builtin::ViewWidth),
    ("viewHeight", "float", Builtin::ViewHeight),
    ("aspectRatio", "float", Builtin::AspectRatio),
    ("near", "float", Builtin::Near),
    ("far", "float", Builtin::Far),
    ("frameTimeCounter", "float", Builtin::FrameTimeCounter),
    ("frameTime", "float", Builtin::FrameTime),
    ("frameCounter", "int", Builtin::FrameCounter),
    ("worldTime", "int", Builtin::WorldTime),
    ("worldDay", "int", Builtin::WorldDay),
    ("moonPhase", "int", Builtin::MoonPhase),
    ("sunAngle", "float", Builtin::SunAngle),
    ("shadowAngle", "float", Builtin::ShadowAngle),
    ("sunPosition", "vec3", Builtin::SunPosition),
    ("moonPosition", "vec3", Builtin::MoonPosition),
    ("shadowLightPosition", "vec3", Builtin::ShadowLightPosition),
    ("upPosition", "vec3", Builtin::UpPosition),
    ("skyColor", "vec3", Builtin::SkyColor),
    ("fogColor", "vec3", Builtin::FogColor),
    ("rainStrength", "float", Builtin::RainStrength),
    ("wetness", "float", Builtin::Wetness),
    ("thunderStrength", "float", Builtin::ThunderStrength),
    ("eyeBrightness", "ivec2", Builtin::EyeBrightness),
    ("eyeBrightnessSmooth", "ivec2", Builtin::EyeBrightnessSmooth),
    ("isEyeInWater", "int", Builtin::IsEyeInWater),
    ("blindness", "float", Builtin::Blindness),
    ("nightVision", "float", Builtin::NightVision),
    ("darknessFactor", "float", Builtin::DarknessFactor),
    ("hideGUI", "int", Builtin::HideGui),
    ("heldItemId", "int", Builtin::HeldItemId),
    ("heldItemId2", "int", Builtin::HeldItemId2),
    ("heldBlockLightValue", "int", Builtin::HeldBlockLightValue),
    ("heldBlockLightValue2", "int", Builtin::HeldBlockLightValue2),
    ("heldBlockLightColor", "vec3", Builtin::HeldBlockLightColor),
    ("heldBlockLightColor2", "vec3", Builtin::HeldBlockLightColor),
    ("fogStart", "float", Builtin::FogStart),
    ("fogEnd", "float", Builtin::FogEnd),
    ("screenBrightness", "float", Builtin::ScreenBrightness),
    ("playerBodyVector", "vec3", Builtin::PlayerBodyVector),
    ("rainfall", "float", Builtin::Rainfall),
    ("temperature", "float", Builtin::Temperature),
    ("biome_precipitation", "int", Builtin::BiomePrecipitation),
    ("renderStage", "int", Builtin::RenderStage),
    (
        "lightningBoltPosition",
        "vec4",
        Builtin::LightningBoltPosition,
    ),
    ("entityId", "int", Builtin::NoId),
    ("blockEntityId", "int", Builtin::NoId),
    ("currentRenderedItemId", "int", Builtin::NoId),
    ("entityColor", "vec4", Builtin::NoEntityColor),
    ("atlasSize", "ivec2", Builtin::AtlasSize),
    ("biome", "int", Builtin::Biome),
    ("shadowModelView", "mat4", Builtin::ShadowModelView),
    (
        "shadowModelViewInverse",
        "mat4",
        Builtin::ShadowModelViewInverse,
    ),
    ("shadowProjection", "mat4", Builtin::ShadowProjection),
    (
        "shadowProjectionInverse",
        "mat4",
        Builtin::ShadowProjectionInverse,
    ),
    ("bedrockLevel", "int", Builtin::BedrockLevel),
    ("heightLimit", "int", Builtin::HeightLimit),
    ("logicalHeightLimit", "int", Builtin::LogicalHeightLimit),
    ("seaLevel", "int", Builtin::SeaLevel),
    ("hasCeiling", "bool", Builtin::HasCeiling),
    ("hasSkylight", "bool", Builtin::HasSkylight),
    ("currentPlayerHealth", "float", Builtin::CurrentPlayerHealth),
    ("maxPlayerHealth", "float", Builtin::MaxPlayerHealth),
    ("currentPlayerHunger", "float", Builtin::CurrentPlayerHunger),
    ("maxPlayerHunger", "float", Builtin::MaxPlayerHunger),
    ("currentPlayerAir", "float", Builtin::CurrentPlayerAir),
    ("maxPlayerAir", "float", Builtin::MaxPlayerAir),
    ("currentPlayerArmor", "float", Builtin::CurrentPlayerArmor),
    ("maxPlayerArmor", "float", Builtin::MaxPlayerArmor),
    ("isSpectator", "bool", Builtin::IsSpectator),
    ("firstPersonCamera", "bool", Builtin::FirstPersonCamera),
    ("isRiding", "bool", Builtin::IsRiding),
    ("isElytraFlying", "bool", Builtin::IsElytraFlying),
    ("inSwimmingAnimation", "bool", Builtin::InSwimmingAnimation),
    ("feetInWater", "bool", Builtin::FeetInWater),
    ("eyePosition", "vec3", Builtin::EyePosition),
    ("playerLookVector", "vec3", Builtin::PlayerLookVector),
    ("vehicleId", "int", Builtin::VehicleId),
    ("vehicleInWater", "bool", Builtin::VehicleInWater),
    ("vehicleLookVector", "vec3", Builtin::VehicleLookVector),
    (
        "relativeVehiclePosition",
        "vec3",
        Builtin::RelativeVehiclePosition,
    ),
];

pub(super) fn builtin(name: &str, ty: &str) -> Option<Builtin> {
    BUILTINS
        .iter()
        .find(|(n, t, _)| *n == name && *t == ty)
        .map(|(_, _, b)| *b)
}

fn input_id(index: usize, member: Option<usize>) -> u32 {
    (index * 5 + member.map_or(0, |m| m + 1)) as u32
}

pub(crate) fn resolve_input(name: &str, member: Option<usize>) -> Option<Resolved> {
    if member.is_none()
        && let Some(biome) = name.strip_prefix("BIOME_")
    {
        let row = crate::util::biome_color::biome_index(&biome.to_ascii_lowercase());
        return Some(Resolved::Constant(row.map_or(-1.0, f32::from)));
    }
    let index = BUILTINS.iter().position(|(n, _, _)| *n == name)?;
    FrameValues::default()
        .get(BUILTINS[index].2)
        .has(member)
        .then(|| Resolved::Input(input_id(index, member)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_inputs_resolve_by_name_and_member() {
        assert!(matches!(
            resolve_input("sunAngle", None),
            Some(Resolved::Input(_))
        ));
        assert!(matches!(
            resolve_input("cameraPosition", Some(1)),
            Some(Resolved::Input(_))
        ));
        assert_eq!(
            resolve_input("cameraPosition", None),
            None,
            "a vector is read through a member"
        );
        assert_eq!(
            resolve_input("sunAngle", Some(0)),
            None,
            "a scalar has no members"
        );
        assert!(builtin("hasCeiling", "bool").is_some());
        assert_eq!(
            builtin("hasCeiling", "int"),
            None,
            "the declared type has to match"
        );
        assert!(matches!(
            resolve_input("isRiding", None),
            Some(Resolved::Input(_))
        ));
        assert_eq!(FrameValues::default().get(Builtin::IsRiding), Value::Int(0));
        assert!(
            matches!(resolve_input("BIOME_NOT_A_BIOME", None), Some(Resolved::Constant(v)) if v == -1.0)
        );
    }
}
