use azalea_protocol::packets::game::s_set_structure_block::{Mirror, Rotation, StructureMode};
use azalea_protocol::packets::game::s_set_test_block::TestBlockMode;
use simdnbt::owned::{Nbt, NbtCompound};

use crate::text::Span;

pub const MAX_OFFSET: i32 = 48;
pub const MAX_SIZE: i32 = 48;
const DEFAULT_OFFSET: [i32; 3] = [0, 1, 0];
pub const DEFAULT_INTEGRITY: f32 = 1.0;
const EMPTY_ID: &str = "minecraft:empty";
const DEFAULT_FINAL_STATE: &str = "minecraft:air";
const DEFAULT_NAMESPACE: &str = "minecraft";

const ROTATIONS: [(Rotation, &str, &str); 4] = [
    (Rotation::None, "NONE", "none"),
    (Rotation::Clockwise90, "CLOCKWISE_90", "clockwise_90"),
    (Rotation::Clockwise180, "CLOCKWISE_180", "180"),
    (
        Rotation::Counterclockwise90,
        "COUNTERCLOCKWISE_90",
        "counterclockwise_90",
    ),
];

pub const ROTATION_VALUES: [Rotation; 4] = [
    ROTATIONS[0].0,
    ROTATIONS[1].0,
    ROTATIONS[2].0,
    ROTATIONS[3].0,
];

const MIRRORS: [(Mirror, &str); 3] = [
    (Mirror::None, "NONE"),
    (Mirror::LeftRight, "LEFT_RIGHT"),
    (Mirror::FrontBack, "FRONT_BACK"),
];

pub const MIRROR_VALUES: [Mirror; 3] = [MIRRORS[0].0, MIRRORS[1].0, MIRRORS[2].0];

const STRUCTURE_MODES: [(StructureMode, &str, &str); 4] = [
    (StructureMode::Save, "SAVE", "save"),
    (StructureMode::Load, "LOAD", "load"),
    (StructureMode::Corner, "CORNER", "corner"),
    (StructureMode::Data, "DATA", "data"),
];

const TEST_BLOCK_MODES: [(TestBlockMode, &str); 4] = [
    (TestBlockMode::Start, "start"),
    (TestBlockMode::Log, "log"),
    (TestBlockMode::Fail, "fail"),
    (TestBlockMode::Accept, "accept"),
];

pub const TEST_BLOCK_MODE_VALUES: [TestBlockMode; 4] = [
    TEST_BLOCK_MODES[0].0,
    TEST_BLOCK_MODES[1].0,
    TEST_BLOCK_MODES[2].0,
    TEST_BLOCK_MODES[3].0,
];

pub fn rotation_legacy(name: &str) -> Option<Rotation> {
    ROTATIONS.iter().find(|r| r.1 == name).map(|r| r.0)
}

pub fn rotation_named(name: &str) -> Option<Rotation> {
    ROTATIONS.iter().find(|r| r.2 == name).map(|r| r.0)
}

pub fn mirror_legacy(name: &str) -> Option<Mirror> {
    MIRRORS.iter().find(|m| m.1 == name).map(|m| m.0)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Joint {
    Rollable,
    Aligned,
}

impl Joint {
    pub const VALUES: [Joint; 2] = [Joint::Rollable, Joint::Aligned];

    pub fn name(self) -> &'static str {
        match self {
            Joint::Rollable => "rollable",
            Joint::Aligned => "aligned",
        }
    }

    fn from_name(name: &str) -> Option<Joint> {
        Joint::VALUES.into_iter().find(|j| j.name() == name)
    }
}

fn valid_path_char(c: char) -> bool {
    matches!(c, '_' | '-' | 'a'..='z' | '0'..='9' | '/' | '.')
}

fn valid_namespace_char(c: char) -> bool {
    matches!(c, '_' | '-' | 'a'..='z' | '0'..='9' | '.')
}

pub fn parse_identifier(s: &str) -> Option<String> {
    let (namespace, path) = match s.find(':') {
        Some(0) => (DEFAULT_NAMESPACE, &s[1..]),
        Some(i) => (&s[..i], &s[i + 1..]),
        None => (DEFAULT_NAMESPACE, s),
    };
    let namespace_ok = namespace != ".." && namespace.chars().all(valid_namespace_char);
    (namespace_ok && path.chars().all(valid_path_char)).then(|| format!("{namespace}:{path}"))
}

fn identifier_or(tag: &NbtCompound, key: &str, fallback: &str) -> String {
    tag.string(key)
        .and_then(|s| parse_identifier(&s.to_string()))
        .unwrap_or_else(|| fallback.to_owned())
}

fn string_or(tag: &NbtCompound, key: &str, fallback: &str) -> String {
    tag.string(key)
        .map(|s| s.to_string())
        .unwrap_or_else(|| fallback.to_owned())
}

pub(crate) fn axes(
    tag: &NbtCompound,
    prefix: &str,
    default: [i32; 3],
    lo: i32,
    hi: i32,
) -> [i32; 3] {
    let mut out = [0; 3];
    for (i, axis) in ["X", "Y", "Z"].iter().enumerate() {
        out[i] = tag
            .int(&format!("{prefix}{axis}"))
            .unwrap_or(default[i])
            .clamp(lo, hi);
    }
    out
}

pub(crate) fn vec3i(tag: &NbtCompound, key: &str) -> Option<[i32; 3]> {
    match tag.int_array(key) {
        Some(&[x, y, z]) => Some([x, y, z]),
        Some(_) => None,
        None => match tag.list(key)?.ints()?.as_slice() {
            &[x, y, z] => Some([x, y, z]),
            _ => None,
        },
    }
}

fn bool_or(tag: &NbtCompound, key: &str, fallback: bool) -> bool {
    tag.byte(key).map_or(fallback, |b| b != 0)
}

#[derive(Clone, Debug)]
pub struct StructureEdit {
    pub name: String,
    pub metadata: String,
    pub offset: [i32; 3],
    pub size: [i32; 3],
    pub rotation: Rotation,
    pub mirror: Mirror,
    pub mode: StructureMode,
    pub ignore_entities: bool,
    pub strict: bool,
    pub show_air: bool,
    pub show_bounding_box: bool,
    pub integrity: f32,
    pub seed: i64,
}

impl StructureEdit {
    pub fn decode(tag: Option<&Nbt>, block_mode: Option<&str>) -> StructureEdit {
        let Some(tag) = tag else {
            let mode = block_mode
                .and_then(|name| STRUCTURE_MODES.iter().find(|m| m.2 == name).map(|m| m.0))
                .unwrap_or(StructureMode::Data);
            return StructureEdit {
                name: String::new(),
                metadata: String::new(),
                offset: DEFAULT_OFFSET,
                size: [0; 3],
                rotation: Rotation::None,
                mirror: Mirror::None,
                mode,
                ignore_entities: true,
                strict: false,
                show_air: false,
                show_bounding_box: true,
                integrity: DEFAULT_INTEGRITY,
                seed: 0,
            };
        };
        let tag: &NbtCompound = tag;
        let named = |key: &str| tag.string(key).map(|s| s.to_string());
        StructureEdit {
            name: named("name")
                .filter(|n| !n.is_empty())
                .and_then(|n| parse_identifier(&n))
                .unwrap_or_default(),
            metadata: string_or(tag, "metadata", ""),
            offset: axes(tag, "pos", DEFAULT_OFFSET, -MAX_OFFSET, MAX_OFFSET),
            size: axes(tag, "size", [0; 3], 0, MAX_SIZE),
            rotation: named("rotation")
                .and_then(|r| rotation_legacy(&r))
                .unwrap_or(Rotation::None),
            mirror: named("mirror")
                .and_then(|m| mirror_legacy(&m))
                .unwrap_or(Mirror::None),
            mode: named("mode")
                .and_then(|m| STRUCTURE_MODES.iter().find(|e| e.1 == m).map(|e| e.0))
                .unwrap_or(StructureMode::Data),
            ignore_entities: bool_or(tag, "ignoreEntities", true),
            strict: bool_or(tag, "strict", false),
            show_air: bool_or(tag, "showair", false),
            show_bounding_box: bool_or(tag, "showboundingbox", true),
            integrity: tag.float("integrity").unwrap_or(DEFAULT_INTEGRITY),
            seed: tag.long("seed").unwrap_or(0),
        }
    }
}

#[derive(Clone, Debug)]
pub struct JigsawEdit {
    pub pool: String,
    pub name: String,
    pub target: String,
    pub final_state: String,
    pub joint: Joint,
    pub selection_priority: i32,
    pub placement_priority: i32,
    pub vertical: bool,
}

impl JigsawEdit {
    pub fn decode(tag: Option<&Nbt>, orientation: &str) -> JigsawEdit {
        let vertical = orientation.starts_with("up_") || orientation.starts_with("down_");
        let Some(tag) = tag else {
            return JigsawEdit {
                pool: EMPTY_ID.to_owned(),
                name: EMPTY_ID.to_owned(),
                target: EMPTY_ID.to_owned(),
                final_state: DEFAULT_FINAL_STATE.to_owned(),
                joint: Joint::Rollable,
                selection_priority: 0,
                placement_priority: 0,
                vertical,
            };
        };
        let tag: &NbtCompound = tag;
        JigsawEdit {
            pool: identifier_or(tag, "pool", EMPTY_ID),
            name: identifier_or(tag, "name", EMPTY_ID),
            target: identifier_or(tag, "target", EMPTY_ID),
            final_state: string_or(tag, "final_state", DEFAULT_FINAL_STATE),
            joint: tag
                .string("joint")
                .and_then(|j| Joint::from_name(&j.to_string()))
                .unwrap_or(if vertical {
                    Joint::Rollable
                } else {
                    Joint::Aligned
                }),
            selection_priority: tag.int("selection_priority").unwrap_or(0),
            placement_priority: tag.int("placement_priority").unwrap_or(0),
            vertical,
        }
    }
}

#[derive(Clone, Debug)]
pub struct TestBlockEdit {
    pub mode: TestBlockMode,
    pub message: String,
}

impl TestBlockEdit {
    pub fn decode(tag: Option<&Nbt>, block_mode: Option<&str>) -> TestBlockEdit {
        let by_name = |name: &str| TEST_BLOCK_MODES.iter().find(|m| m.1 == name).map(|m| m.0);
        let Some(tag) = tag else {
            return TestBlockEdit {
                mode: block_mode.and_then(by_name).unwrap_or(TestBlockMode::Start),
                message: String::new(),
            };
        };
        TestBlockEdit {
            mode: tag
                .string("mode")
                .and_then(|m| by_name(&m.to_string()))
                .unwrap_or(TestBlockMode::Fail),
            message: string_or(tag, "message", ""),
        }
    }
}

#[derive(Clone, Debug)]
pub struct TestInstanceEdit {
    pub test: Option<String>,
    pub size: [i32; 3],
    pub rotation: Rotation,
    pub ignore_entities: bool,
}

impl TestInstanceEdit {
    pub fn decode(tag: Option<&Nbt>) -> TestInstanceEdit {
        let mut out = TestInstanceEdit {
            test: None,
            size: [0; 3],
            rotation: Rotation::None,
            ignore_entities: false,
        };
        let Some(data) = tag.and_then(|tag| tag.compound("data")) else {
            return out;
        };
        out.test = data
            .string("test")
            .and_then(|t| parse_identifier(&t.to_string()));
        if let Some(size) = vec3i(data, "size") {
            out.size = size;
        }
        out.rotation = data
            .string("rotation")
            .and_then(|r| rotation_named(&r.to_string()))
            .unwrap_or(Rotation::None);
        out.ignore_entities = bool_or(data, "ignore_entities", false);
        out
    }
}

pub fn test_instance_error(tag: &Nbt) -> Option<Vec<Span>> {
    const RED: usize = 0xC;
    let error = tag.compound("data")?.get("error_message")?;
    let text = crate::client::chat_text::from_nbt_tag(error)?;
    Some(crate::client::chat_text::to_spans_styled(
        &text,
        crate::text::Style::colored(crate::text::FORMAT_COLORS[RED]),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifiers_parse_like_vanilla() {
        assert_eq!(
            parse_identifier("stone").as_deref(),
            Some("minecraft:stone")
        );
        assert_eq!(
            parse_identifier(":stone").as_deref(),
            Some("minecraft:stone")
        );
        assert_eq!(parse_identifier("mod:a/b.c").as_deref(), Some("mod:a/b.c"));
        assert_eq!(parse_identifier("").as_deref(), Some("minecraft:"));
        assert_eq!(parse_identifier("Stone"), None);
        assert_eq!(parse_identifier("a:b:c"), None);
        assert_eq!(parse_identifier("a/b:c"), None);
        assert_eq!(parse_identifier("..:c"), None);
    }

    #[test]
    fn an_undescribed_structure_block_reads_its_block_state() {
        let edit = StructureEdit::decode(None, Some("load"));
        assert_eq!(edit.mode, StructureMode::Load);
        assert_eq!(edit.offset, DEFAULT_OFFSET);
        assert!(edit.ignore_entities && edit.show_bounding_box);
        assert_eq!(edit.integrity, DEFAULT_INTEGRITY);
    }

    #[test]
    fn jigsaw_joint_defaults_follow_the_front() {
        assert!(JigsawEdit::decode(None, "up_north").vertical);
        assert!(!JigsawEdit::decode(None, "north_up").vertical);
        assert_eq!(JigsawEdit::decode(None, "north_up").joint, Joint::Rollable);
    }

    #[test]
    fn rotation_spellings() {
        assert_eq!(
            rotation_legacy("CLOCKWISE_180"),
            Some(Rotation::Clockwise180)
        );
        assert_eq!(rotation_named("180"), Some(Rotation::Clockwise180));
        assert_eq!(rotation_named("CLOCKWISE_180"), None);
    }
}
