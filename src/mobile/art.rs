use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use bevy::prelude::*;

use crate::client::bedrock_art;
use crate::gui::atlas::{GuiAtlas, Region, TOUCH_ART_H, TOUCH_ART_W};
use crate::gui::atlas_writes::AtlasWrites;
use crate::{log_info, log_warn};

const TAG: &str = "bedrock_art";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tile {
    Jump,
    Sneak,
    Sprint,
    Attack,
    Use,
    JumpDpad,
    SneakDpad,
    FlyUp,
    FlyDown,
    Up,
    Down,
    Left,
    Right,
    UpLeft,
    UpRight,
    DownLeft,
    DownRight,
    Inventory,
    Chat,
    Command,
    Pause,
    Camera,
    Debug,
    Drop,
    Offhand,
    Mods,
    Frame,
    Knob,
}

const TWINNED: [Tile; 26] = [
    Tile::Jump,
    Tile::Sneak,
    Tile::Sprint,
    Tile::Attack,
    Tile::Use,
    Tile::JumpDpad,
    Tile::SneakDpad,
    Tile::FlyUp,
    Tile::FlyDown,
    Tile::Up,
    Tile::Down,
    Tile::Left,
    Tile::Right,
    Tile::UpLeft,
    Tile::UpRight,
    Tile::DownLeft,
    Tile::DownRight,
    Tile::Inventory,
    Tile::Chat,
    Tile::Command,
    Tile::Pause,
    Tile::Camera,
    Tile::Debug,
    Tile::Drop,
    Tile::Offhand,
    Tile::Mods,
];

enum Art {
    Mojang(&'static str),
    Glyph(&'static str),
}

impl Tile {
    fn art(self) -> Art {
        match self {
            Tile::Jump => Art::Mojang("ui/jump"),
            Tile::Sneak => Art::Mojang("ui/sneak"),
            Tile::Sprint => Art::Mojang("ui/sprint"),
            Tile::Attack => Art::Mojang("ui/attack"),
            Tile::Use => Art::Mojang("ui/interact"),
            Tile::JumpDpad => Art::Mojang("gui/controls/jump_dpad"),
            Tile::SneakDpad => Art::Mojang("gui/controls/sneak_dpad"),
            Tile::FlyUp => Art::Mojang("gui/controls/flyingascend"),
            Tile::FlyDown => Art::Mojang("gui/controls/flyingdescend"),
            Tile::Up => Art::Mojang("gui/controls/up"),
            Tile::Down => Art::Mojang("gui/controls/down"),
            Tile::Left => Art::Mojang("gui/controls/left"),
            Tile::Right => Art::Mojang("gui/controls/right"),
            Tile::UpLeft => Art::Mojang("gui/controls/up_left"),
            Tile::UpRight => Art::Mojang("gui/controls/up_right"),
            Tile::DownLeft => Art::Mojang("gui/controls/down_left"),
            Tile::DownRight => Art::Mojang("gui/controls/down_right"),
            Tile::Inventory => Art::Glyph(GLYPH_INVENTORY),
            Tile::Chat => Art::Glyph(GLYPH_CHAT),
            Tile::Command => Art::Glyph(GLYPH_COMMAND),
            Tile::Pause => Art::Glyph(GLYPH_PAUSE),
            Tile::Camera => Art::Glyph(GLYPH_CAMERA),
            Tile::Debug => Art::Glyph(GLYPH_DEBUG),
            Tile::Drop => Art::Glyph(GLYPH_DROP),
            Tile::Offhand => Art::Glyph(GLYPH_OFFHAND),
            Tile::Mods => Art::Glyph(GLYPH_MODS),
            Tile::Frame => Art::Mojang("ui/joystick_frame"),
            Tile::Knob => Art::Mojang("ui/joystick_knob"),
        }
    }
}

const BUTTON: u32 = 22;
const CELL: u32 = 24;
const COLUMNS: u32 = 16;
const STICK_Y: u32 = 96;
const FRAME_CELL: u32 = 72;
const FRAME: u32 = 64;
const KNOB_CELL: u32 = 48;
const KNOB: u32 = 32;

const _: () = {
    let cells = TWINNED.len() as u32 * 2;
    assert!(cells.div_ceil(COLUMNS) * CELL <= STICK_Y);
    assert!(COLUMNS * CELL <= TOUCH_ART_W);
    assert!(FRAME_CELL + KNOB_CELL <= TOUCH_ART_W);
    assert!(STICK_Y + FRAME_CELL <= TOUCH_ART_H);
    assert!(KNOB_CELL <= FRAME_CELL);
};

fn local(tile: Tile, pressed: bool) -> Region {
    match tile {
        Tile::Frame => {
            let inset = (FRAME_CELL - FRAME) / 2;
            Region {
                x: inset,
                y: STICK_Y + inset,
                w: FRAME,
                h: FRAME,
            }
        }
        Tile::Knob => {
            let inset = (KNOB_CELL - KNOB) / 2;
            Region {
                x: FRAME_CELL + inset,
                y: STICK_Y + inset,
                w: KNOB,
                h: KNOB,
            }
        }
        _ => {
            let i = TWINNED.iter().position(|t| *t == tile).unwrap_or(0) as u32;
            let cell = i * 2 + u32::from(pressed);
            let gutter = (CELL - BUTTON) / 2;
            Region {
                x: (cell % COLUMNS) * CELL + gutter,
                y: (cell / COLUMNS) * CELL + gutter,
                w: BUTTON,
                h: BUTTON,
            }
        }
    }
}

pub fn region(atlas: &GuiAtlas, tile: Tile, pressed: bool) -> Option<Region> {
    if !ready() {
        return None;
    }
    let block = atlas.touch_art()?;
    let r = local(tile, pressed);
    Some(Region {
        x: block.x + r.x,
        y: block.y + r.y,
        w: r.w,
        h: r.h,
    })
}

static READY: AtomicBool = AtomicBool::new(false);

pub fn ready() -> bool {
    READY.load(Ordering::Relaxed)
}

const FACE: [u8; 4] = [128, 128, 128, 255];
const INK: [[u8; 4]; 2] = [[0, 0, 0, 255], [230, 230, 230, 255]];

const GLYPH_PAUSE: &str =
    "###..###\n###..###\n###..###\n###..###\n###..###\n###..###\n###..###\n###..###";
const GLYPH_INVENTORY: &str = "##..##..##\n##..##..##";
const GLYPH_CHAT: &str = ".##########.\n#..........#\n#.########.#\n#..........#\n#.######...#\n#..........#\n\
     .###.######.\n...#.#......\n...##.......";
const GLYPH_CAMERA: &str = "...######...\n.##......##.\n#....##....#\n#...####...#\n#....##....#\n.##......##.\n\
     ...######...";
const GLYPH_OFFHAND: &str = "...#........\n..##........\n.##########.\n..##........\n...#........\n........#...\n\
     ........##..\n.##########.\n........##..\n........#...";
const GLYPH_COMMAND: &str =
    ".......#\n......##\n.....##.\n....##..\n...##...\n..##....\n.##.....\n##......\n#.......";
const GLYPH_DEBUG: &str = "######.#####.\n######.######\n##.........##\n##.........##\n#####...####.\n\
     #####...####.\n##.........##\n##.........##\n##.....######\n##.....#####.";

const GLYPH_DROP: &str = ".....#######\n\
     .....#######\n\
     ........####\n\
     .......##.##\n\
     ......##..##\n\
     .....##...##\n\
     ....##......\n\
     ...##.......\n\
     ............\n\
     ######......";

const GLYPH_MODS: &str = "####..####\n\
     ####..####\n\
     ####..####\n\
     ####..####\n\
     ..........\n\
     ..........\n\
     ####..####\n\
     ####..####\n\
     ####..####\n\
     ####..####";

fn blank(plate: &mut [u8], w: u32, pressed: bool) {
    let ink = INK[usize::from(pressed)];
    for y in 2..20u32 {
        for x in 3..19u32 {
            let i = ((y * w + x) * 4) as usize;
            if plate.get(i..i + 4) == Some(&ink[..]) {
                plate[i..i + 4].copy_from_slice(&FACE);
            }
        }
    }
}

fn glyph_origin(glyph: &str, pressed: bool) -> (i32, i32, i32, i32) {
    let rows: Vec<&str> = glyph.lines().map(str::trim).collect();
    let gw = rows.iter().map(|r| r.len()).max().unwrap_or(0) as i32;
    let gh = rows.len() as i32;
    (
        11 - (gw + 1) / 2,
        10 - (gh + 1) / 2 + i32::from(pressed),
        gw,
        gh,
    )
}

fn stamp(plate: &[u8], w: u32, glyph: &str, pressed: bool) -> Vec<u8> {
    let mut out = plate.to_vec();
    let h = (plate.len() / (w as usize * 4)) as i32;
    let (ox, oy, _, _) = glyph_origin(glyph, pressed);
    let ink = INK[usize::from(pressed)];
    for (dy, row) in glyph.lines().map(str::trim).enumerate() {
        for (dx, c) in row.bytes().enumerate() {
            let (x, y) = (ox + dx as i32, oy + dy as i32);
            if c == b'#' && (0..w as i32).contains(&x) && (0..h).contains(&y) {
                let i = ((y as u32 * w + x as u32) * 4) as usize;
                out[i..i + 4].copy_from_slice(&ink);
            }
        }
    }
    out
}

fn decode(files: &[(&'static str, Vec<u8>)], name: &str, size: u32) -> Result<Vec<u8>, String> {
    let (_, bytes) = files
        .iter()
        .find(|(n, _)| *n == name)
        .ok_or_else(|| format!("{name} is missing"))?;
    let image = image::load_from_memory(bytes)
        .map_err(|e| format!("{name}: {e}"))?
        .to_rgba8();
    if image.dimensions() != (size, size) {
        let (w, h) = image.dimensions();
        return Err(format!("{name} is {w}x{h}, expected {size}x{size}"));
    }
    Ok(image.into_raw())
}

fn put(block: &mut [u8], r: Region, pixels: &[u8]) {
    let row = (r.w * 4) as usize;
    for y in 0..r.h {
        let to = (((r.y + y) * TOUCH_ART_W + r.x) * 4) as usize;
        let from = y as usize * row;
        block[to..to + row].copy_from_slice(&pixels[from..from + row]);
    }
}

fn compose(files: &[(&'static str, Vec<u8>)]) -> Result<Vec<u8>, String> {
    let mut block = vec![0u8; (TOUCH_ART_W * TOUCH_ART_H * 4) as usize];
    let mut plates = [
        decode(files, "ui/sprint", BUTTON)?,
        decode(files, "ui/sprint_pressed", BUTTON)?,
    ];
    for (pressed, plate) in plates.iter_mut().enumerate() {
        blank(plate, BUTTON, pressed == 1);
    }
    for tile in TWINNED {
        for pressed in [false, true] {
            let pixels = match tile.art() {
                Art::Mojang(name) if pressed => decode(files, &format!("{name}_pressed"), BUTTON)?,
                Art::Mojang(name) => decode(files, name, BUTTON)?,
                Art::Glyph(glyph) => stamp(&plates[usize::from(pressed)], BUTTON, glyph, pressed),
            };
            put(&mut block, local(tile, pressed), &pixels);
        }
    }
    for (tile, size) in [(Tile::Frame, FRAME), (Tile::Knob, KNOB)] {
        let Art::Mojang(name) = tile.art() else {
            continue;
        };
        put(&mut block, local(tile, false), &decode(files, name, size)?);
    }
    Ok(block)
}

#[derive(Resource, Default)]
struct Uploaded {
    composed: Option<(Arc<Vec<(&'static str, Vec<u8>)>>, Option<Arc<Vec<u8>>>)>,
    layout: Option<u32>,
}

fn upload(
    assets: Option<Res<crate::gui::render::GuiAssets>>,
    mut uploaded: ResMut<Uploaded>,
    mut writes: ResMut<AtlasWrites>,
) {
    if let Some(layout) = uploaded.layout {
        if layout == writes.layout {
            return;
        }
        READY.store(false, Ordering::Relaxed);
        uploaded.layout = None;
    }
    let Some(files) = bedrock_art::files() else {
        return;
    };
    let Some(assets) = assets else { return };
    let Some(block) = assets.atlas.touch_art() else {
        return;
    };

    if !uploaded
        .composed
        .as_ref()
        .is_some_and(|(f, _)| Arc::ptr_eq(f, &files))
    {
        let pixels = match compose(&files) {
            Ok(pixels) => {
                log_info!(TAG, "button textures composed");
                Some(Arc::new(pixels))
            }
            Err(e) => {
                log_warn!(TAG, "button textures not used: {e}");
                None
            }
        };
        uploaded.composed = Some((files, pixels));
    }
    let Some(Some(pixels)) = uploaded.composed.as_ref().map(|(_, p)| p.clone()) else {
        return;
    };
    writes.push(block, pixels);
    uploaded.layout = Some(writes.layout);
    READY.store(true, Ordering::Relaxed);
}

pub fn register(app: &mut App) {
    app.init_resource::<Uploaded>()
        .add_systems(Update, upload.before(crate::gui::render::draw_gui));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all() -> Vec<(Tile, bool)> {
        let mut out: Vec<(Tile, bool)> = TWINNED
            .iter()
            .flat_map(|&t| [(t, false), (t, true)])
            .collect();
        out.push((Tile::Frame, false));
        out.push((Tile::Knob, false));
        out
    }

    #[test]
    fn every_tile_has_its_own_rectangle_with_a_gutter() {
        let tiles = all();
        for &(tile, pressed) in &tiles {
            let r = local(tile, pressed);
            assert!(r.x >= 1 && r.y >= 1, "{tile:?} {pressed}");
            assert!(
                r.x + r.w < TOUCH_ART_W && r.y + r.h < TOUCH_ART_H,
                "{tile:?} {pressed}"
            );
        }
        for (i, &(a, ap)) in tiles.iter().enumerate() {
            for &(b, bp) in &tiles[i + 1..] {
                let (ra, rb) = (local(a, ap), local(b, bp));
                let apart = ra.x + ra.w + 1 <= rb.x
                    || rb.x + rb.w + 1 <= ra.x
                    || ra.y + ra.h + 1 <= rb.y
                    || rb.y + rb.h + 1 <= ra.y;
                assert!(apart, "{a:?} {ap} touches {b:?} {bp}");
            }
        }
        assert_eq!(local(Tile::Frame, false).w, 64);
        assert_eq!(local(Tile::Knob, true), local(Tile::Knob, false));
    }

    #[test]
    fn every_mojang_tile_names_a_listed_file() {
        let listed = |name: &str| bedrock_art::FILES.iter().any(|(n, _)| *n == name);
        for &(tile, pressed) in &all() {
            if let Art::Mojang(name) = tile.art() {
                assert!(listed(name), "{name}");
                if pressed {
                    assert!(listed(&format!("{name}_pressed")), "{name}_pressed");
                }
            }
        }
        assert!(listed("ui/sprint") && listed("ui/sprint_pressed"));
    }

    #[test]
    fn a_glyph_goes_where_the_icon_was() {
        let w = 22;
        let mut plate = vec![128; (w * w * 4) as usize];
        let centre = ((10 * w + 11) * 4) as usize;
        plate[centre..centre + 4].copy_from_slice(&[0, 0, 0, 255]);
        blank(&mut plate, w, false);
        assert_eq!(&plate[centre..centre + 4], &FACE);
        let out = stamp(&plate, w, "##\n##", false);
        let black = out.chunks(4).filter(|p| *p == INK[0]).count();
        assert_eq!(black, 4);
        let pressed = stamp(&plate, w, "##\n##", true);
        let light = pressed.chunks(4).filter(|p| *p == INK[1]).count();
        assert_eq!(light, 4);
    }

    #[test]
    fn every_glyph_stays_on_the_face() {
        for tile in TWINNED {
            let Art::Glyph(glyph) = tile.art() else {
                continue;
            };
            for pressed in [false, true] {
                let (ox, oy, gw, gh) = glyph_origin(glyph, pressed);
                assert!(
                    ox >= 3 && ox + gw <= 19,
                    "{tile:?} {pressed}: columns {ox}..{}",
                    ox + gw
                );
                assert!(
                    oy >= 2 && oy + gh <= 20,
                    "{tile:?} {pressed}: rows {oy}..{}",
                    oy + gh
                );
            }
            let rows: Vec<&str> = glyph.lines().map(str::trim).collect();
            assert!(
                rows.iter()
                    .all(|r| r.bytes().all(|c| c == b'#' || c == b'.')),
                "{tile:?}"
            );
        }
        for glyph in [GLYPH_DROP, GLYPH_MODS] {
            let (_, _, gw, gh) = glyph_origin(glyph, false);
            assert!(gw <= 12 && gh <= 10);
        }
    }

    fn synthetic(frame: u32) -> Vec<(&'static str, Vec<u8>)> {
        let png = |size: u32| {
            let image = image::RgbaImage::from_pixel(size, size, image::Rgba(FACE));
            let mut out = std::io::Cursor::new(Vec::new());
            image::DynamicImage::ImageRgba8(image)
                .write_to(&mut out, image::ImageFormat::Png)
                .unwrap();
            out.into_inner()
        };
        bedrock_art::FILES
            .iter()
            .map(|&(name, _)| {
                let size = match name {
                    "ui/joystick_frame" => frame,
                    "ui/joystick_knob" => KNOB,
                    _ => BUTTON,
                };
                (name, png(size))
            })
            .collect()
    }

    #[test]
    fn the_block_is_composed_or_refused_whole() {
        let block = compose(&synthetic(FRAME)).unwrap();
        assert_eq!(block.len(), (TOUCH_ART_W * TOUCH_ART_H * 4) as usize);
        assert_eq!(&block[..4], &[0, 0, 0, 0]);
        let r = local(Tile::Pause, true);
        let i = (((r.y + 10) * TOUCH_ART_W + r.x + 10) * 4) as usize;
        assert_eq!(block[i + 3], 255);
        assert!(compose(&synthetic(FRAME + 1)).is_err());
    }
}
