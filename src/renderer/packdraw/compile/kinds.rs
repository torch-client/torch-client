use bevy::render::render_resource::{BlendFactor, BlendState};

use crate::shaderpack::programs::Geometry;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct AlphaTest {
    pub(crate) function: AlphaFunction,
    pub(crate) reference: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AlphaFunction {
    Never,
    Less,
    Equal,
    LessEqual,
    Greater,
    NotEqual,
    GreaterEqual,
}

impl AlphaFunction {
    pub(crate) fn operator(self) -> &'static str {
        match self {
            AlphaFunction::Never | AlphaFunction::Less => "<",
            AlphaFunction::Equal => "==",
            AlphaFunction::LessEqual => "<=",
            AlphaFunction::Greater => ">",
            AlphaFunction::NotEqual => "!=",
            AlphaFunction::GreaterEqual => ">=",
        }
    }
}

impl AlphaTest {
    const fn greater(reference: f32) -> AlphaTest {
        AlphaTest {
            function: AlphaFunction::Greater,
            reference,
        }
    }

    pub(crate) fn wrapper_reference(self) -> f32 {
        if self.function == AlphaFunction::Never {
            -1.0
        } else {
            self.reference
        }
    }
}

pub(crate) const ONE_TENTH_ALPHA: AlphaTest = AlphaTest::greater(0.1);
pub(crate) const HALF_ALPHA: AlphaTest = AlphaTest::greater(0.5);
pub(crate) const NON_ZERO_ALPHA: AlphaTest = AlphaTest::greater(1.0e-4);

pub(crate) fn geometry_alpha(geometry: Geometry, shadow: bool) -> Option<AlphaTest> {
    match geometry {
        Geometry::TerrainSolid => None,
        Geometry::TerrainCutout => Some(HALF_ALPHA),
        Geometry::Water if shadow => None,
        Geometry::Water => Some(NON_ZERO_ALPHA),
    }
}

pub(crate) fn alpha_for(
    program_override: Option<Option<AlphaTest>>,
    default: Option<AlphaTest>,
) -> Option<AlphaTest> {
    program_override.unwrap_or(default)
}

pub(super) fn alpha_tests(
    properties: &std::collections::HashMap<String, String>,
) -> std::collections::HashMap<String, Option<AlphaTest>> {
    properties
        .iter()
        .filter_map(|(key, value)| {
            let program = key.strip_prefix("alphaTest.")?;
            if value == "off" || value == "false" {
                return Some((program.to_owned(), None));
            }
            let parts: Vec<&str> = value.split(' ').collect();
            let [name, reference, ..] = parts[..] else {
                return None;
            };
            let function = match name {
                "ALWAYS" | "GL_ALWAYS" => None,
                "NEVER" => Some(AlphaFunction::Never),
                "LESS" => Some(AlphaFunction::Less),
                "EQUAL" => Some(AlphaFunction::Equal),
                "LEQUAL" => Some(AlphaFunction::LessEqual),
                "GREATER" => Some(AlphaFunction::Greater),
                "NOTEQUAL" => Some(AlphaFunction::NotEqual),
                "GEQUAL" => Some(AlphaFunction::GreaterEqual),
                _ => return None,
            };
            let reference: f32 = reference.parse().ok()?;
            Some((
                program.to_owned(),
                function.map(|function| AlphaTest {
                    function,
                    reference,
                }),
            ))
        })
        .collect()
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Blend {
    pub(crate) program: Option<Option<BlendState>>,
    pub(crate) buffers: Vec<(u8, Option<BlendState>)>,
}

impl Blend {
    pub(crate) fn for_buffer(
        &self,
        buffer: u8,
        default: Option<BlendState>,
        per_buffer: bool,
    ) -> Option<BlendState> {
        let program = self.program.unwrap_or(default);
        if !per_buffer {
            return program;
        }
        self.buffers
            .iter()
            .rev()
            .find(|(b, _)| *b == buffer)
            .map_or(program, |(_, mode)| *mode)
    }
}

pub(super) fn blends(
    properties: &std::collections::HashMap<String, String>,
    notes: &mut Vec<String>,
) -> std::collections::HashMap<String, Blend> {
    use crate::shaderpack::features::{Feature, supported};
    let mut out: std::collections::HashMap<String, Blend> = std::collections::HashMap::new();
    for (key, value) in properties {
        let Some(pass) = key.strip_prefix("blend.") else {
            continue;
        };
        let mode = if value.trim() == "off" {
            None
        } else {
            let Some(mode) = blend_mode(value) else {
                notes.push(format!(
                    "{key} skipped: {value:?} is not four blend factors"
                ));
                continue;
            };
            Some(mode)
        };
        match pass.split_once('.') {
            Some((program, buffer)) => {
                if !supported(Feature::PerBufferBlending) {
                    notes.push(format!(
                        "{key} skipped: {}",
                        Feature::PerBufferBlending.absence()
                    ));
                    continue;
                }
                let Some(index) = crate::shaderpack::directives::target_index(buffer) else {
                    notes.push(format!("{key} skipped: {buffer:?} names no target"));
                    continue;
                };
                out.entry(program.to_owned())
                    .or_default()
                    .buffers
                    .push((index as u8, mode));
            }
            None => out.entry(pass.to_owned()).or_default().program = Some(mode),
        }
    }
    out
}

fn blend_mode(value: &str) -> Option<BlendState> {
    let factors: Vec<BlendFactor> = value
        .split_whitespace()
        .map(|name| {
            Some(match name {
                "ZERO" => BlendFactor::Zero,
                "ONE" => BlendFactor::One,
                "SRC_COLOR" => BlendFactor::Src,
                "ONE_MINUS_SRC_COLOR" => BlendFactor::OneMinusSrc,
                "DST_COLOR" => BlendFactor::Dst,
                "ONE_MINUS_DST_COLOR" => BlendFactor::OneMinusDst,
                "SRC_ALPHA" => BlendFactor::SrcAlpha,
                "ONE_MINUS_SRC_ALPHA" => BlendFactor::OneMinusSrcAlpha,
                "DST_ALPHA" => BlendFactor::DstAlpha,
                "ONE_MINUS_DST_ALPHA" => BlendFactor::OneMinusDstAlpha,
                "SRC_ALPHA_SATURATE" => BlendFactor::SrcAlphaSaturated,
                _ => return None,
            })
        })
        .collect::<Option<_>>()?;
    let [src_color, dst_color, src_alpha, dst_alpha, ..] = factors[..] else {
        return None;
    };
    Some(crate::renderer::blend(
        src_color, dst_color, src_alpha, dst_alpha,
    ))
}

crate::util::indexed_enum! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub(crate) enum EntityKind {
        Entity,
        Block,
        Particle,
        SkyBasic,
        SkyTextured,
        Clouds,
        Hand,
        HandWater,
        ShadowCaster,
        EntityTranslucent,
        BlockTranslucent,
        ParticleTranslucent,
        DamagedBlock,
        Line,
        SpiderEyes,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum Segment {
    Shadow,
    Sky,
    World,
    HandSolid,
    WorldTranslucent,
    AfterTranslucent,
    HandTranslucent,
}

impl EntityKind {
    fn programs(self) -> &'static [&'static str] {
        match self {
            EntityKind::Entity => &[
                "gbuffers_entities",
                "gbuffers_textured_lit",
                "gbuffers_textured",
                "gbuffers_basic",
            ],
            EntityKind::Block => &[
                "gbuffers_block",
                "gbuffers_terrain",
                "gbuffers_textured_lit",
                "gbuffers_textured",
                "gbuffers_basic",
            ],
            EntityKind::Particle => &[
                "gbuffers_particles",
                "gbuffers_textured_lit",
                "gbuffers_textured",
                "gbuffers_basic",
            ],
            EntityKind::SkyBasic => &["gbuffers_skybasic", "gbuffers_basic"],
            EntityKind::SkyTextured => &[
                "gbuffers_skytextured",
                "gbuffers_textured",
                "gbuffers_basic",
            ],
            EntityKind::Clouds => &["gbuffers_clouds", "gbuffers_textured", "gbuffers_basic"],
            EntityKind::Hand => &[
                "gbuffers_hand",
                "gbuffers_textured_lit",
                "gbuffers_textured",
                "gbuffers_basic",
            ],
            EntityKind::HandWater => &[
                "gbuffers_hand_water",
                "gbuffers_hand",
                "gbuffers_textured_lit",
                "gbuffers_textured",
                "gbuffers_basic",
            ],
            EntityKind::ShadowCaster => &["shadow_entities", "shadow"],
            EntityKind::EntityTranslucent => &[
                "gbuffers_entities_translucent",
                "gbuffers_entities",
                "gbuffers_textured_lit",
                "gbuffers_textured",
                "gbuffers_basic",
            ],
            EntityKind::BlockTranslucent => &[
                "gbuffers_block_translucent",
                "gbuffers_block",
                "gbuffers_terrain",
                "gbuffers_textured_lit",
                "gbuffers_textured",
                "gbuffers_basic",
            ],
            EntityKind::ParticleTranslucent => &[
                "gbuffers_particles_translucent",
                "gbuffers_particles",
                "gbuffers_textured_lit",
                "gbuffers_textured",
                "gbuffers_basic",
            ],
            EntityKind::DamagedBlock => &[
                "gbuffers_damagedblock",
                "gbuffers_terrain",
                "gbuffers_textured_lit",
                "gbuffers_textured",
                "gbuffers_basic",
            ],
            EntityKind::Line => &["gbuffers_line", "gbuffers_basic"],
            EntityKind::SpiderEyes => {
                &["gbuffers_spidereyes", "gbuffers_textured", "gbuffers_basic"]
            }
        }
    }

    pub(crate) fn resolve(self, has: impl Fn(&str) -> bool) -> Option<&'static str> {
        self.programs().iter().copied().find(|n| has(n))
    }

    pub(crate) fn segment(self) -> Segment {
        match self {
            EntityKind::Entity | EntityKind::Block | EntityKind::Particle => Segment::World,
            EntityKind::SkyBasic | EntityKind::SkyTextured => Segment::Sky,
            EntityKind::Clouds => Segment::AfterTranslucent,
            EntityKind::Hand => Segment::HandSolid,
            EntityKind::HandWater => Segment::HandTranslucent,
            EntityKind::ShadowCaster => Segment::Shadow,
            EntityKind::EntityTranslucent
            | EntityKind::BlockTranslucent
            | EntityKind::ParticleTranslucent => Segment::WorldTranslucent,
            EntityKind::DamagedBlock | EntityKind::Line | EntityKind::SpiderEyes => Segment::World,
        }
    }

    pub(crate) fn render_stage(self) -> i32 {
        crate::shaderpack::features::render_stage(match self {
            EntityKind::Entity
            | EntityKind::ShadowCaster
            | EntityKind::EntityTranslucent
            | EntityKind::SpiderEyes => "ENTITIES",
            EntityKind::DamagedBlock => "DESTROY",
            EntityKind::Line => "OUTLINE",
            EntityKind::Block | EntityKind::BlockTranslucent => "BLOCK_ENTITIES",
            EntityKind::Particle | EntityKind::ParticleTranslucent => "PARTICLES",
            EntityKind::SkyBasic => "SKY",
            EntityKind::SkyTextured => "SUN",
            EntityKind::Clouds => "CLOUDS",
            EntityKind::Hand => "HAND_SOLID",
            EntityKind::HandWater => "HAND_TRANSLUCENT",
        })
    }

    pub(crate) fn fallback(self) -> Option<EntityKind> {
        match self {
            EntityKind::Block | EntityKind::Particle => Some(EntityKind::Entity),
            EntityKind::BlockTranslucent | EntityKind::ParticleTranslucent => {
                Some(EntityKind::EntityTranslucent)
            }
            _ => None,
        }
    }

    pub(crate) fn translucent(self) -> EntityKind {
        match self {
            EntityKind::Entity => EntityKind::EntityTranslucent,
            EntityKind::Block => EntityKind::BlockTranslucent,
            EntityKind::Particle => EntityKind::ParticleTranslucent,
            other => other,
        }
    }

    pub(crate) fn alpha_tests(self) -> [Option<AlphaTest>; 2] {
        match self {
            EntityKind::SkyBasic | EntityKind::SpiderEyes => [Some(NON_ZERO_ALPHA); 2],
            EntityKind::SkyTextured | EntityKind::Line => [None; 2],
            _ => [Some(ONE_TENTH_ALPHA); 2],
        }
    }

    pub(crate) fn is_hand(self) -> bool {
        matches!(self, EntityKind::Hand | EntityKind::HandWater)
    }
}

pub(super) fn true_key(
    properties: &std::collections::HashMap<String, String>,
    key: &str,
    default: bool,
) -> bool {
    properties.get(key).map_or(default, |v| v.trim() == "true")
}

pub(super) fn false_key(properties: &std::collections::HashMap<String, String>, key: &str) -> bool {
    properties.get(key).is_none_or(|v| v.trim() != "false")
}

pub(super) fn clouds_off(properties: &std::collections::HashMap<String, String>) -> bool {
    properties.get("clouds").is_some_and(|v| v.trim() == "off")
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum ShadowCulling {
    #[default]
    Default,
    Distance,
    Advanced,
    SafeZone,
}

impl ShadowCulling {
    pub(crate) fn mode(
        self,
        voxelizes: bool,
    ) -> crate::renderer::terrain_pool::cull::ShadowCullMode {
        use crate::renderer::terrain_pool::cull::ShadowCullMode;
        match self {
            ShadowCulling::Distance => ShadowCullMode::Distance,
            ShadowCulling::Advanced => ShadowCullMode::Advanced,
            ShadowCulling::SafeZone => ShadowCullMode::SafeZone,
            ShadowCulling::Default if voxelizes => ShadowCullMode::Distance,
            ShadowCulling::Default => ShadowCullMode::Advanced,
        }
    }

    pub(super) fn parse(value: Option<&str>) -> ShadowCulling {
        match value.map(str::trim) {
            Some("false") => ShadowCulling::Distance,
            Some("true") => ShadowCulling::Advanced,
            Some("reversed" | "safe_zone") => ShadowCulling::SafeZone,
            _ => ShadowCulling::Default,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ShadowCasters {
    pub(crate) entities: bool,
    pub(crate) player: bool,
    pub(crate) block_entities: bool,
}

impl ShadowCasters {
    pub(super) fn read(properties: &std::collections::HashMap<String, String>) -> ShadowCasters {
        ShadowCasters {
            entities: true_key(properties, "shadowEntities", true),
            player: true_key(properties, "shadowPlayer", false),
            block_entities: true_key(properties, "shadowBlockEntities", true),
        }
    }

    pub(crate) fn any(self) -> bool {
        self.entities || self.player || self.block_entities
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SkyParts {
    pub(crate) sky: bool,
    pub(crate) sun: bool,
    pub(crate) moon: bool,
    pub(crate) stars: bool,
}

impl SkyParts {
    pub(super) fn read(properties: &std::collections::HashMap<String, String>) -> SkyParts {
        SkyParts {
            sky: false_key(properties, "sky"),
            sun: false_key(properties, "sun"),
            moon: false_key(properties, "moon"),
            stars: false_key(properties, "stars"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_fall_back_as_iris_does() {
        let solas = [
            "gbuffers_basic",
            "gbuffers_textured",
            "gbuffers_hand",
            "gbuffers_entities",
            "gbuffers_block",
        ];
        let resolved = |kind: EntityKind, names: &[&str]| kind.resolve(|n| names.contains(&n));
        assert_eq!(
            resolved(EntityKind::SkyBasic, &solas),
            Some("gbuffers_basic")
        );
        assert_eq!(
            resolved(EntityKind::SkyTextured, &solas),
            Some("gbuffers_textured")
        );
        assert_eq!(
            resolved(EntityKind::Clouds, &solas),
            Some("gbuffers_textured")
        );
        assert_eq!(resolved(EntityKind::Hand, &solas), Some("gbuffers_hand"));
        assert_eq!(
            resolved(EntityKind::HandWater, &solas),
            Some("gbuffers_hand")
        );
        let minimal = ["gbuffers_terrain"];
        for kind in [
            EntityKind::SkyBasic,
            EntityKind::SkyTextured,
            EntityKind::Clouds,
            EntityKind::Hand,
            EntityKind::HandWater,
        ] {
            assert_eq!(resolved(kind, &minimal), None, "{kind:?}");
        }
        assert_eq!(EntityKind::Block.fallback(), Some(EntityKind::Entity));
        assert_eq!(EntityKind::Hand.fallback(), None);
        assert_eq!(
            resolved(EntityKind::ParticleTranslucent, &solas),
            Some("gbuffers_textured")
        );
        assert_eq!(
            resolved(EntityKind::EntityTranslucent, &solas),
            Some("gbuffers_entities")
        );
        assert_eq!(
            EntityKind::ALL.map(EntityKind::index),
            std::array::from_fn(|i| i)
        );
    }

    #[test]
    fn shadow_caster_keys_follow_iris() {
        let none = ShadowCasters::read(&std::collections::HashMap::new());
        assert_eq!(
            none,
            ShadowCasters {
                entities: true,
                player: false,
                block_entities: true
            }
        );
        let solas = std::collections::HashMap::from([
            ("shadowEntities".to_string(), "false".to_string()),
            ("shadowBlockEntities".to_string(), "false".to_string()),
            ("shadowPlayer".to_string(), "true".to_string()),
        ]);
        let casters = ShadowCasters::read(&solas);
        assert_eq!(
            casters,
            ShadowCasters {
                entities: false,
                player: true,
                block_entities: false
            }
        );
        assert!(casters.any());
        assert_eq!(EntityKind::ShadowCaster.segment(), Segment::Shadow);
        assert!(
            Segment::Shadow < Segment::Sky,
            "drawn first, in the shadow pass"
        );
    }

    #[test]
    fn shadow_culling_keys_follow_iris() {
        assert_eq!(
            ShadowCulling::parse(Some("reversed")),
            ShadowCulling::SafeZone
        );
        assert_eq!(
            ShadowCulling::parse(Some("safe_zone")),
            ShadowCulling::SafeZone
        );
        assert_eq!(ShadowCulling::parse(Some("false")), ShadowCulling::Distance);
        assert_eq!(ShadowCulling::parse(Some("true")), ShadowCulling::Advanced);
        assert_eq!(ShadowCulling::parse(None), ShadowCulling::Default);
        use crate::renderer::terrain_pool::cull::ShadowCullMode;
        assert_eq!(ShadowCulling::Default.mode(true), ShadowCullMode::Distance);
        assert_eq!(ShadowCulling::Default.mode(false), ShadowCullMode::Advanced);
        assert_eq!(ShadowCulling::SafeZone.mode(true), ShadowCullMode::SafeZone);
    }

    #[test]
    fn cloud_and_sky_keys_are_read() {
        let clouds = |value: &str| {
            clouds_off(&std::collections::HashMap::from([(
                "clouds".to_string(),
                value.to_string(),
            )]))
        };
        assert!(clouds(" off "));
        assert!(!clouds("fancy") && !clouds("sometimes"));
        assert!(!clouds_off(&std::collections::HashMap::new()));
        let props = std::collections::HashMap::from([("stars".to_string(), "false".to_string())]);
        let parts = SkyParts::read(&props);
        assert!(parts.sky && parts.sun && parts.moon && !parts.stars);
    }

    #[test]
    fn alpha_test_keys_are_read() {
        let props = std::collections::HashMap::from([
            (
                "alphaTest.gbuffers_water".to_string(),
                "GREATER 0.02".to_string(),
            ),
            ("alphaTest.gbuffers_terrain".to_string(), "off".to_string()),
            (
                "alphaTest.gbuffers_hand".to_string(),
                "LESS 0.5".to_string(),
            ),
            ("alphaTest.gbuffers_block".to_string(), "false".to_string()),
            ("alphaTest.gbuffers_basic".to_string(), "ALWAYS".to_string()),
            (
                "alphaTest.gbuffers_textured".to_string(),
                "GL_ALWAYS 0.1".to_string(),
            ),
            ("clouds".to_string(), "true".to_string()),
        ]);
        let tests = alpha_tests(&props);
        assert_eq!(
            tests.get("gbuffers_water"),
            Some(&Some(AlphaTest::greater(0.02)))
        );
        assert_eq!(tests.get("gbuffers_terrain"), Some(&None));
        assert_eq!(
            tests.get("gbuffers_block"),
            Some(&None),
            "Iris reads false as off"
        );
        assert_eq!(
            tests.get("gbuffers_hand"),
            Some(&Some(AlphaTest {
                function: AlphaFunction::Less,
                reference: 0.5
            }))
        );
        assert_eq!(
            tests.get("gbuffers_basic"),
            None,
            "a bare function is refused, as Iris refuses it"
        );
        assert_eq!(tests.get("gbuffers_textured"), Some(&None));
        assert_eq!(tests.len(), 5);
    }

    #[test]
    fn blend_keys_are_read_as_iris_reads_them() {
        let props = std::collections::HashMap::from([
            (
                "blend.gbuffers_water".to_owned(),
                "SRC_ALPHA ONE_MINUS_SRC_ALPHA ONE ZERO".to_owned(),
            ),
            (
                "blend.gbuffers_water.colortex2".to_owned(),
                "off".to_owned(),
            ),
            (
                "blend.gbuffers_water.gaux1".to_owned(),
                "ONE ONE ONE ONE".to_owned(),
            ),
            ("blend.composite".to_owned(), "off".to_owned()),
            (
                "blend.gbuffers_hand".to_owned(),
                "SRC_ALPHA NOT_A_FACTOR ONE ZERO".to_owned(),
            ),
        ]);
        let mut notes = Vec::new();
        let blends = blends(&props, &mut notes);
        let water = &blends["gbuffers_water"];
        let mode = blend_mode("SRC_ALPHA ONE_MINUS_SRC_ALPHA ONE ZERO");
        assert_eq!(water.for_buffer(0, None, true), mode);
        assert_eq!(
            water.for_buffer(2, Some(BlendState::ALPHA_BLENDING), true),
            None
        );
        assert_eq!(
            water.for_buffer(4, None, true),
            blend_mode("ONE ONE ONE ONE"),
            "gaux1 is colortex4"
        );
        assert_eq!(
            water.for_buffer(2, None, false),
            mode,
            "a composite ignores buffer keys"
        );
        assert_eq!(
            blends["composite"].for_buffer(0, Some(BlendState::ALPHA_BLENDING), false),
            None
        );
        assert!(!blends.contains_key("gbuffers_hand"));
        assert_eq!(notes.len(), 1, "{notes:?}");
        assert_eq!(
            Blend::default().for_buffer(1, Some(BlendState::ALPHA_BLENDING), true),
            Some(BlendState::ALPHA_BLENDING)
        );
    }
}
