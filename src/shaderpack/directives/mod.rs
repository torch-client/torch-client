mod targets;

pub(crate) use targets::{
    MAX_COLOR_TARGETS, MAX_SHADOW_COLORS, SampleKind, TargetFormat, TargetSettings, Targets,
    read_sizes, shadow_index, target_index,
};

use super::literal::{self, broadcast, const_declaration, constructor_args};

fn vec4_literal(value: &str) -> Option<[f32; 4]> {
    broadcast(
        constructor_args(value, "vec4")?
            .into_iter()
            .map(literal::float)
            .collect::<Option<_>>()?,
    )
}

pub(crate) fn read_targets<'a>(sources: impl Iterator<Item = &'a str>) -> (Targets, Vec<String>) {
    let mut targets = Targets::default();
    let mut unknown = Vec::new();
    for source in sources {
        for line in source.lines() {
            let Some((ty, name, value)) = const_declaration(line) else {
                continue;
            };
            for (suffix, expected) in [("ClearColor", "vec4"), ("Format", "int"), ("Clear", "bool")]
            {
                let Some(stem) = name.strip_suffix(suffix) else {
                    continue;
                };
                let settings = match (target_index(stem), shadow_index(stem)) {
                    (Some(target), _) => &mut targets.settings[target],
                    (None, Some(target)) => &mut targets.shadow[target],
                    (None, None) => continue,
                };
                if ty != expected {
                    continue;
                }
                match suffix {
                    "Format" => match TargetFormat::parse(value) {
                        Some(format) => settings.format = format,
                        None => unknown.push(format!("{name} = {value}")),
                    },
                    "Clear" => match value {
                        "true" => settings.clear = true,
                        "false" => settings.clear = false,
                        _ => unknown.push(format!("{name} = {value}")),
                    },
                    _ => match vec4_literal(value) {
                        Some(color) => settings.clear_color = Some(color),
                        None => unknown.push(format!("{name} = {value}")),
                    },
                }
                break;
            }
        }
    }
    (targets, unknown)
}

pub(crate) fn mipmapped<'a>(sources: impl IntoIterator<Item = &'a str>) -> u16 {
    let mut mask = 0u16;
    for source in sources {
        for line in source.lines() {
            let Some(("bool", name, value)) = const_declaration(line) else {
                continue;
            };
            let Some(target) = name.strip_suffix("MipmapEnabled").and_then(target_index) else {
                continue;
            };
            match value {
                "true" => mask |= 1 << target,
                "false" => mask &= !(1 << target),
                _ => {}
            }
        }
    }
    mask
}

pub(crate) fn work_groups_render(source: &str) -> Option<[f32; 2]> {
    source.lines().rev().find_map(|line| {
        let ("vec2", "workGroupsRender", value) = const_declaration(line)? else {
            return None;
        };
        let parts = constructor_args(value, "vec2")?
            .into_iter()
            .map(|p| literal::float(p).filter(|v| *v > 0.0))
            .collect::<Option<_>>()?;
        broadcast(parts)
    })
}

pub(crate) fn local_size(source: &str) -> [u32; 3] {
    let mut size = [1u32; 3];
    for line in source.lines().filter(|l| l.contains("local_size_")) {
        for (axis, key) in ["local_size_x", "local_size_y", "local_size_z"]
            .iter()
            .enumerate()
        {
            if let Some(rest) = line.split(key).nth(1)
                && let Some(value) = rest.trim_start().strip_prefix('=')
            {
                let digits: String = value
                    .trim_start()
                    .chars()
                    .take_while(char::is_ascii_digit)
                    .collect();
                if let Ok(n) = digits.parse::<u32>() {
                    size[axis] = n.max(1);
                }
            }
        }
    }
    size
}

pub(crate) fn work_groups(source: &str) -> Option<[u32; 3]> {
    source.lines().rev().find_map(|line| {
        let ("ivec3", "workGroups", value) = const_declaration(line)? else {
            return None;
        };
        let parts = constructor_args(value, "ivec3")?
            .into_iter()
            .map(|p| {
                literal::int(p)
                    .and_then(|n| u32::try_from(n).ok())
                    .filter(|n| *n > 0)
            })
            .collect::<Option<_>>()?;
        broadcast(parts)
    })
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Constants {
    pub(crate) sun_path_rotation: f32,
    pub(crate) shadow_distance: f32,
    pub(crate) shadow_interval: f32,
    pub(crate) wetness_half_life: f32,
    pub(crate) dryness_half_life: f32,
    pub(crate) eye_brightness_half_life: f32,
    pub(crate) shadow_near_plane: f32,
    pub(crate) shadow_far_plane: f32,
    pub(crate) noise_resolution: u32,
    pub(crate) shadow_map_resolution: u32,
    pub(crate) shadow_nearest: [bool; 2],
    pub(crate) voxel_distance: f32,
    pub(crate) shadow_distance_render_mul: f32,
}

impl Default for Constants {
    fn default() -> Self {
        Constants {
            sun_path_rotation: 0.0,
            shadow_distance: 160.0,
            shadow_interval: 2.0,
            wetness_half_life: 600.0,
            dryness_half_life: 200.0,
            eye_brightness_half_life: 10.0,
            shadow_near_plane: -100.05,
            shadow_far_plane: 156.0,
            noise_resolution: 256,
            shadow_map_resolution: 1024,
            shadow_nearest: [false; 2],
            voxel_distance: 0.0,
            shadow_distance_render_mul: -1.0,
        }
    }
}

pub(crate) fn read_constants<'a>(sources: impl Iterator<Item = &'a str>) -> Constants {
    let mut constants = Constants::default();
    for source in sources {
        for line in source.lines() {
            let Some((ty, name, value)) = const_declaration(line) else {
                continue;
            };
            if ty == "int" {
                let Some(size) = literal::int(value).and_then(|n| u32::try_from(n).ok()) else {
                    continue;
                };
                match name {
                    "noiseTextureResolution" if (1..=4096).contains(&size) => {
                        constants.noise_resolution = size
                    }
                    "shadowMapResolution" if (1..=16384).contains(&size) => {
                        constants.shadow_map_resolution = size
                    }
                    _ => {}
                }
                continue;
            }
            if ty == "bool" {
                let on = value == "true";
                match name {
                    "shadowtex0Nearest" => constants.shadow_nearest[0] = on,
                    "shadowtex1Nearest" => constants.shadow_nearest[1] = on,
                    _ => {}
                }
                continue;
            }
            if ty != "float" {
                continue;
            }
            let Some(value) = literal::float(value) else {
                continue;
            };
            match name {
                "sunPathRotation" => constants.sun_path_rotation = value,
                "shadowDistance" => constants.shadow_distance = value,
                "shadowIntervalSize" => constants.shadow_interval = value,
                "wetnessHalflife" => constants.wetness_half_life = value,
                "drynessHalflife" => constants.dryness_half_life = value,
                "eyeBrightnessHalflife" => constants.eye_brightness_half_life = value,
                "shadowNearPlane" => constants.shadow_near_plane = value,
                "shadowFarPlane" => constants.shadow_far_plane = value,
                "voxelDistance" => constants.voxel_distance = value,
                "shadowDistanceRenderMul" => constants.shadow_distance_render_mul = value,
                _ => {}
            }
        }
    }
    constants
}

pub(crate) fn draw_buffers(fragment: &str) -> Result<Vec<u8>, String> {
    let draw = find_comment_directive(fragment, "DRAWBUFFERS");
    let render = find_comment_directive(fragment, "RENDERTARGETS");
    let applied = match (draw, render) {
        (Some(d), Some(r)) => Some(if d.0 > r.0 { (d.1, false) } else { (r.1, true) }),
        (Some(d), None) => Some((d.1, false)),
        (None, Some(r)) => Some((r.1, true)),
        (None, None) => None,
    };
    let Some((text, listed)) = applied else {
        return Ok(vec![0]);
    };
    let entries: Vec<&str> = if listed {
        text.split(',').map(str::trim).collect()
    } else {
        text.char_indices()
            .map(|(at, c)| &text[at..at + c.len_utf8()])
            .collect()
    };
    entries
        .into_iter()
        .map(|entry| {
            entry
                .parse::<u8>()
                .ok()
                .filter(|n| (*n as usize) < MAX_COLOR_TARGETS)
                .ok_or_else(|| format!("draw buffer {entry:?} is not a target this client has"))
        })
        .collect()
}

fn find_comment_directive<'a>(haystack: &'a str, name: &str) -> Option<(usize, &'a str)> {
    let prefix = format!("{name}:");
    let at = haystack.rfind(&prefix)?;
    if !haystack[..at].trim_end().ends_with("/*") {
        return None;
    }
    let rest = &haystack[at + prefix.len()..];
    let end = rest.find("*/")?;
    Some((at, rest[..end].trim()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draw_buffers_follow_the_later_directive_and_default_to_zero() {
        assert_eq!(draw_buffers("void main() {}"), Ok(vec![0]));
        assert_eq!(draw_buffers("/* DRAWBUFFERS:0367 */"), Ok(vec![0, 3, 6, 7]));
        assert_eq!(draw_buffers("/*DRAWBUFFERS:12*/"), Ok(vec![1, 2]));
        assert_eq!(draw_buffers("/* RENDERTARGETS: 0, 12 */"), Ok(vec![0, 12]));
        assert_eq!(
            draw_buffers("/* RENDERTARGETS: 5 */\n/* DRAWBUFFERS:12 */"),
            Ok(vec![1, 2])
        );
        assert_eq!(
            draw_buffers("/* DRAWBUFFERS:0 */ x /* DRAWBUFFERS:3 */"),
            Ok(vec![3])
        );
        assert_eq!(draw_buffers("DRAWBUFFERS:3 */"), Ok(vec![0]));
        assert!(draw_buffers("/* RENDERTARGETS: 40 */").is_err());
    }

    #[test]
    fn target_settings_are_read_under_both_names() {
        let (targets, unknown) = read_targets(
            [
                "const int colortex3Format = RGBA16F;",
                "const bool gaux1Clear = false;",
                "const vec4 colortex2ClearColor = vec4(1.0, 0.5, 0.0, 1.0);",
                "const int colortex9Format = RGB8;",
                "const int colortex5Format = NOT_A_FORMAT;",
                "const float colortex6Format = 1.0;",
            ]
            .into_iter(),
        );
        assert_eq!(targets.settings[3].format, TargetFormat::Rgba16Float);
        assert!(!targets.settings[4].clear);
        assert_eq!(
            targets.settings[2].clear_color(2, [0.0; 4]),
            [1.0, 0.5, 0.0, 1.0]
        );
        assert_eq!(targets.settings[9].format, TargetFormat::Rgba8Unorm);
        assert_eq!(targets.settings[5].format, TargetFormat::Rgba8Unorm);
        assert_eq!(unknown, ["colortex5Format = NOT_A_FORMAT"]);
        assert_eq!(targets.settings[6], TargetSettings::default());
    }

    #[test]
    fn constants_are_read_with_their_defaults_otherwise() {
        let constants = read_constants(
            [
                "const float sunPathRotation = -40.0;",
                "const float wetnessHalflife = 300.0f;",
                "const int shadowDistance = 5;",
            ]
            .into_iter(),
        );
        assert_eq!(constants.sun_path_rotation, -40.0);
        assert_eq!(constants.wetness_half_life, 300.0);
        assert_eq!(constants.shadow_distance, 160.0);
    }

    #[test]
    fn default_clear_colours_are_iris_s() {
        let settings = TargetSettings::default();
        let fog = [0.2, 0.3, 0.4, 1.0];
        assert_eq!(settings.clear_color(0, fog), fog);
        assert_eq!(settings.clear_color(1, fog), [1.0; 4]);
        assert_eq!(settings.clear_color(7, fog), [0.0; 4]);
    }

    #[test]
    fn target_names_resolve_and_out_of_range_ones_do_not() {
        assert_eq!(target_index("colortex0"), Some(0));
        assert_eq!(target_index("colortex15"), Some(15));
        assert_eq!(target_index("gaux4"), Some(7));
        assert_eq!(target_index("colortex16"), None);
        assert_eq!(target_index("colortex03"), None);
        assert_eq!(target_index("depthtex0"), None);
    }

    #[test]
    fn mipmap_flags_are_read_per_target() {
        let mask = mipmapped([
            "const bool colortex0MipmapEnabled = true;\nconst bool gaux1MipmapEnabled = true;",
            "const bool colortex4MipmapEnabled = false;\nconst float colortex1MipmapEnabled = 1.0;",
        ]);
        assert_eq!(mask, 0b1);
        assert_eq!(
            mipmapped(["const bool colortex3MipmapEnabled = true;"]),
            0b1000
        );
    }

    #[test]
    fn shadow_targets_are_read_beside_the_colour_ones() {
        let (targets, unknown) = read_targets(
            ["const int shadowcolor0Format = RGBA16F;\nconst bool shadowcolor1Clear = false;\nconst int colortex2Format = RGBA16F;"]
                .into_iter(),
        );
        assert!(unknown.is_empty(), "{unknown:?}");
        assert_eq!(targets.shadow[0].format, TargetFormat::Rgba16Float);
        assert!(!targets.shadow[1].clear);
        assert_eq!(targets.settings[2].format, TargetFormat::Rgba16Float);
        assert_eq!(targets.settings[0].format, TargetFormat::Rgba8Unorm);
        assert_eq!(targets.shadow[0].shadow_clear_color(), [1.0; 4]);
        assert_eq!(shadow_index("shadowcolor"), Some(0));
        assert_eq!(shadow_index("shadowcolor8"), None);
        let constants = read_constants(["const int shadowMapResolution = 2048;"].into_iter());
        assert_eq!(constants.shadow_map_resolution, 2048);
    }

    #[test]
    fn work_groups_are_read() {
        assert_eq!(
            work_groups("const ivec3 workGroups = ivec3(16, 8, 16);"),
            Some([16, 8, 16])
        );
        assert_eq!(
            work_groups("const ivec3 workGroups = ivec3(0, 8, 16);"),
            None
        );
        assert_eq!(work_groups("uniform int x;"), None);
    }

    #[test]
    fn relative_dispatch_and_local_size_are_read() {
        assert_eq!(
            work_groups_render("const vec2 workGroupsRender = vec2(0.5, 0.25);"),
            Some([0.5, 0.25])
        );
        assert_eq!(
            work_groups_render("const vec2 workGroupsRender = vec2(0.0, 1.0);"),
            None
        );
        assert_eq!(
            local_size("layout (local_size_x = 8, local_size_y = 4) in;"),
            [8, 4, 1]
        );
        assert_eq!(local_size("void main() {}"), [1, 1, 1]);
    }
}
