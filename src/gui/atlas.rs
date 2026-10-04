use std::collections::HashMap;
use std::path::Path;

use image::RgbaImage;

use crate::items::icons::{ItemIcons, bake_item_icons};
use crate::text::Font;

pub const SERVER_ICON_PX: u32 = 64;

pub const SERVER_ICON_SLOTS: usize = 32;

const SERVER_ICON_COLUMNS: usize = 8;

pub const BANNER_ICON_PX: u32 = crate::items::icons::MODEL_ICON;

pub const BANNER_ICON_SLOTS: usize = 8;

const BANNER_ICON_COLUMNS: usize = 4;

pub const PLAYER_FACE_SLOTS: usize = 80;

const PLAYER_FACE_COLUMNS: usize = 10;

const PLAYER_FACE_W: u32 = SKIN_HEAD_SIZE * 2;
const PLAYER_FACE_H: u32 = SKIN_HEAD_SIZE;

pub(crate) const SKIN_HEAD_U: u32 = 8;
pub(crate) const SKIN_HEAD_V: u32 = 8;
pub(crate) const SKIN_HAT_U: u32 = 40;
pub(crate) const SKIN_HEAD_SIZE: u32 = 8;

#[derive(Clone, Copy, Debug)]
struct ReservedGrid {
    origin: (u32, u32),
    cell_w: u32,
    cell_h: u32,
    columns: usize,
    slots: usize,
}

impl ReservedGrid {
    const fn new(cell_w: u32, cell_h: u32, columns: usize, slots: usize) -> ReservedGrid {
        ReservedGrid {
            origin: (0, 0),
            cell_w,
            cell_h,
            columns,
            slots,
        }
    }

    fn at(self, origin: (u32, u32)) -> ReservedGrid {
        ReservedGrid { origin, ..self }
    }

    fn blank(&self) -> RgbaImage {
        let rows = self.slots.div_ceil(self.columns) as u32;
        RgbaImage::new(self.columns as u32 * self.cell_w, rows * self.cell_h)
    }

    fn cell(&self, slot: usize) -> Option<(u32, u32)> {
        if slot >= self.slots {
            return None;
        }
        Some((
            self.origin.0 + (slot % self.columns) as u32 * self.cell_w,
            self.origin.1 + (slot / self.columns) as u32 * self.cell_h,
        ))
    }
}

const SERVER_ICON_GRID: ReservedGrid = ReservedGrid::new(
    SERVER_ICON_PX,
    SERVER_ICON_PX,
    SERVER_ICON_COLUMNS,
    SERVER_ICON_SLOTS,
);
const BANNER_ICON_GRID: ReservedGrid = ReservedGrid::new(
    BANNER_ICON_PX,
    BANNER_ICON_PX,
    BANNER_ICON_COLUMNS,
    BANNER_ICON_SLOTS,
);
const PLAYER_FACE_GRID: ReservedGrid = ReservedGrid::new(
    PLAYER_FACE_W,
    PLAYER_FACE_H,
    PLAYER_FACE_COLUMNS,
    PLAYER_FACE_SLOTS,
);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Region {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

#[derive(Clone, Copy, Debug)]
pub enum Scaling {
    Stretch,
    Tile {
        w: u32,
        h: u32,
    },
    NineSlice {
        w: u32,
        h: u32,
        border: [u32; 4],
        stretch_inner: bool,
    },
}

#[derive(Clone, Copy, Debug)]
pub struct Sprite {
    pub region: Region,
    pub scaling: Scaling,
}

pub struct AtlasPixels {
    pub rgba: RgbaImage,
    pub unihex: Vec<u8>,
}

pub struct GuiAtlas {
    width: u32,
    height: u32,
    pixels: std::sync::Mutex<Option<AtlasPixels>>,
    sprites: HashMap<String, Sprite>,
    sheets: HashMap<String, Region>,
    items: HashMap<String, Vec<(u32, Region)>>,
    font_origin: (u32, u32),
    server_icons: ReservedGrid,
    banner_icons: ReservedGrid,
    player_faces: ReservedGrid,
    pub white: Region,
    pub font: Font,
}

impl GuiAtlas {
    pub fn sprite(&self, name: &str) -> Option<&Sprite> {
        self.sprites.get(name)
    }

    pub fn sheet(&self, name: &str) -> Option<Region> {
        self.sheets.get(name).copied()
    }

    #[allow(
        dead_code,
        reason = "untiered lookups, used by the preview and the tests"
    )]
    pub fn item(&self, id: &str) -> Option<Region> {
        self.items
            .get(id)
            .and_then(|tiers| tiers.last())
            .map(|(_, r)| *r)
    }

    pub fn item_for_size(&self, id: &str, target_px: f32) -> Option<Region> {
        let tiers = self.items.get(id)?;
        tiers
            .iter()
            .find(|(size, _)| *size as f32 + 0.5 >= target_px)
            .or_else(|| tiers.last())
            .map(|(_, r)| *r)
    }

    #[allow(
        dead_code,
        reason = "untiered lookups, used by the preview and the tests"
    )]
    pub fn has_item(&self, id: &str) -> bool {
        self.items.contains_key(id)
    }

    pub fn banner_icon(&self, slot: usize) -> Option<Region> {
        let (x, y) = self.banner_icons.cell(slot)?;
        Some(Region {
            x,
            y,
            w: BANNER_ICON_PX,
            h: BANNER_ICON_PX,
        })
    }

    pub fn server_icon(&self, slot: usize) -> Option<Region> {
        let (x, y) = self.server_icons.cell(slot)?;
        Some(Region {
            x,
            y,
            w: SERVER_ICON_PX,
            h: SERVER_ICON_PX,
        })
    }

    #[cfg(feature = "skins")]
    pub fn player_face(&self, slot: usize) -> Option<(Region, Region)> {
        let (x, y) = self.player_faces.cell(slot)?;
        let cell = |x| Region {
            x,
            y,
            w: SKIN_HEAD_SIZE,
            h: SKIN_HEAD_SIZE,
        };
        Some((cell(x), cell(x + SKIN_HEAD_SIZE)))
    }

    pub fn glyph_region(&self, rect: [u32; 4]) -> Region {
        Region {
            x: self.font_origin.0 + rect[0],
            y: self.font_origin.1 + rect[1],
            w: rect[2],
            h: rect[3],
        }
    }

    pub fn boot() -> GuiAtlas {
        let (font, font_pixels) = Font::builtin();
        let page = &font_pixels.pages;
        let (w, h) = (page.width(), page.height());
        let mut image = RgbaImage::new(w.max(1), h + 1);
        for y in 0..h {
            for x in 0..w {
                image.put_pixel(x, y, *page.get_pixel(x, y));
            }
        }
        image.put_pixel(0, h, image::Rgba([255, 255, 255, 255]));

        GuiAtlas {
            width: image.width(),
            height: image.height(),
            pixels: std::sync::Mutex::new(Some(AtlasPixels {
                rgba: image,
                unihex: font_pixels.unihex,
            })),
            sprites: HashMap::new(),
            sheets: HashMap::new(),
            items: HashMap::new(),
            font_origin: (0, 0),
            server_icons: SERVER_ICON_GRID,
            banner_icons: BANNER_ICON_GRID,
            player_faces: PLAYER_FACE_GRID,
            white: Region {
                x: 0,
                y: h,
                w: 1,
                h: 1,
            },
            font,
        }
    }

    pub fn uv(&self, px: f32, py: f32) -> [f32; 2] {
        [px / self.width as f32, py / self.height as f32]
    }

    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    pub fn take_pixels(&self) -> Option<AtlasPixels> {
        self.pixels
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
    }

    pub fn pixels(&self) -> std::sync::MutexGuard<'_, Option<AtlasPixels>> {
        self.pixels
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    pub fn unihex_uv(&self, px: f32, py: f32) -> [f32; 2] {
        let (w, h) = self.font.unihex_dims;
        [px / w.max(1) as f32, py / h.max(1) as f32]
    }
}

struct Sheet {
    key: &'static str,
    w: u32,
    h: u32,
}

const SHEETS: &[Sheet] = &[
    Sheet {
        key: "container/generic_54",
        w: 176,
        h: 222,
    },
    Sheet {
        key: "container/inventory",
        w: 176,
        h: 166,
    },
    Sheet {
        key: "container/crafting_table",
        w: 176,
        h: 166,
    },
    Sheet {
        key: "container/furnace",
        w: 176,
        h: 166,
    },
    Sheet {
        key: "container/blast_furnace",
        w: 176,
        h: 166,
    },
    Sheet {
        key: "container/smoker",
        w: 176,
        h: 166,
    },
    Sheet {
        key: "container/creative_inventory/tab_items",
        w: 195,
        h: 136,
    },
    Sheet {
        key: "container/creative_inventory/tab_item_search",
        w: 195,
        h: 136,
    },
    Sheet {
        key: "container/creative_inventory/tab_inventory",
        w: 195,
        h: 136,
    },
    Sheet {
        key: "container/enchanting_table",
        w: 176,
        h: 166,
    },
    Sheet {
        key: "container/grindstone",
        w: 176,
        h: 166,
    },
    Sheet {
        key: "container/loom",
        w: 176,
        h: 166,
    },
    Sheet {
        key: "container/stonecutter",
        w: 176,
        h: 166,
    },
    Sheet {
        key: "container/cartography_table",
        w: 176,
        h: 166,
    },
    Sheet {
        key: "container/smithing",
        w: 176,
        h: 166,
    },
    Sheet {
        key: "container/anvil",
        w: 176,
        h: 166,
    },
    Sheet {
        key: "container/brewing_stand",
        w: 176,
        h: 166,
    },
    Sheet {
        key: "container/hopper",
        w: 176,
        h: 133,
    },
    Sheet {
        key: "container/beacon",
        w: 230,
        h: 219,
    },
    Sheet {
        key: "container/villager",
        w: 276,
        h: 166,
    },
    Sheet {
        key: "book",
        w: 192,
        h: 192,
    },
];

struct Extra {
    key: &'static str,
    path: &'static str,
    alt_path: Option<&'static str>,
    size: (u32, u32),
    crop_h: Option<u32>,
    nearest: bool,
    scaling: Scaling,
}

const EXTRAS: &[Extra] = &[
    Extra {
        key: "menu_background",
        path: "gui/menu_background.png",
        alt_path: None,
        size: (32, 32),
        crop_h: None,
        nearest: true,
        scaling: Scaling::Tile { w: 32, h: 32 },
    },
    Extra {
        key: "menu_list_background",
        path: "gui/menu_list_background.png",
        alt_path: None,
        size: (32, 32),
        crop_h: None,
        nearest: true,
        scaling: Scaling::Tile { w: 32, h: 32 },
    },
    Extra {
        key: "header_separator",
        path: "gui/header_separator.png",
        alt_path: None,
        size: (32, 2),
        crop_h: None,
        nearest: true,
        scaling: Scaling::Tile { w: 32, h: 2 },
    },
    Extra {
        key: "footer_separator",
        path: "gui/footer_separator.png",
        alt_path: None,
        size: (32, 2),
        crop_h: None,
        nearest: true,
        scaling: Scaling::Tile { w: 32, h: 2 },
    },
    Extra {
        key: "title/minecraft",
        path: "gui/title/minecraft.png",
        alt_path: None,
        size: (256, 64),
        crop_h: Some(44),
        nearest: true,
        scaling: Scaling::Stretch,
    },
    Extra {
        key: "title/edition",
        path: "gui/title/edition.png",
        alt_path: None,
        size: (128, 16),
        crop_h: Some(14),
        nearest: true,
        scaling: Scaling::Stretch,
    },
    Extra {
        key: "unknown_server",
        path: "misc/unknown_server.png",
        alt_path: None,
        size: (32, 32),
        crop_h: None,
        nearest: false,
        scaling: Scaling::Stretch,
    },
    Extra {
        key: "powder_snow_outline",
        path: "misc/powder_snow_outline.png",
        alt_path: None,
        size: (256, 256),
        crop_h: None,
        nearest: true,
        scaling: Scaling::Stretch,
    },
    Extra {
        key: "pack_icon",
        path: "gui/pack.png",
        alt_path: Some("../../../pack.png"),
        size: (32, 32),
        crop_h: None,
        nearest: false,
        scaling: Scaling::Stretch,
    },
    #[cfg(resource_packs)]
    Extra {
        key: "unknown_pack",
        path: "misc/unknown_pack.png",
        alt_path: None,
        size: (32, 32),
        crop_h: None,
        nearest: false,
        scaling: Scaling::Stretch,
    },
];

fn bake_banner_layers(assets: &Path) -> Vec<Entry> {
    const FACE: (u32, u32, u32, u32) = (1, 1, 20, 40);
    const SWATCH: (u32, u32) = (5, 10);

    let dir = assets.join("textures").join("entity").join("banner");
    let mut files = Vec::new();
    collect_pngs(&dir, &dir, &mut files);
    files.sort();

    files
        .into_iter()
        .filter_map(|(name, path)| {
            let mut img = crate::platform::assets::open_image(&path).ok()?.to_rgba8();
            if img.width() < FACE.0 + FACE.2 || img.height() < FACE.1 + FACE.3 {
                return None;
            }
            let face = image::imageops::crop(&mut img, FACE.0, FACE.1, FACE.2, FACE.3).to_image();
            let swatch = image::imageops::resize(
                &face,
                SWATCH.0,
                SWATCH.1,
                image::imageops::FilterType::Triangle,
            );
            Some(Entry {
                kind: Kind::Sprite,
                key: format!("banner/{name}"),
                image: swatch,
                scaling: Scaling::Stretch,
            })
        })
        .collect()
}

fn load_sga(assets: &Path) -> Option<Entry> {
    let path = assets.join("textures").join("font").join("ascii_sga.png");
    let image = crate::platform::assets::open_image(&path).ok()?.to_rgba8();
    SGA_ADVANCE.set(sga_advances(&image)).ok();
    Some(Entry {
        kind: Kind::Sprite,
        key: SGA_SPRITE.to_string(),
        image,
        scaling: Scaling::Stretch,
    })
}

pub const SGA_SPRITE: &str = "font/ascii_sga";

pub const SGA_CELL: f32 = 8.0;

static SGA_ADVANCE: std::sync::OnceLock<[u8; 128]> = std::sync::OnceLock::new();

pub fn sga_advance(c: char) -> f32 {
    let table = match SGA_ADVANCE.get() {
        Some(t) => t,
        None => return 0.0,
    };
    let i = c as usize;
    if i < table.len() {
        table[i] as f32
    } else {
        0.0
    }
}

fn sga_advances(image: &RgbaImage) -> [u8; 128] {
    let mut out = [0u8; 128];
    let cell = SGA_CELL as u32;
    for (i, slot) in out.iter_mut().enumerate() {
        let (cx, cy) = ((i as u32 % 16) * cell, (i as u32 / 16) * cell);
        if cx + cell > image.width() || cy + cell > image.height() {
            continue;
        }
        let mut used = 0;
        for x in 0..cell {
            for y in 0..cell {
                if image.get_pixel(cx + x, cy + y)[3] != 0 {
                    used = x + 1;
                    break;
                }
            }
        }
        *slot = if used == 0 { 0 } else { (used + 1) as u8 };
    }
    out[b' ' as usize] = 4;
    out
}

fn load_extra(textures: &Path, extra: &Extra) -> Option<RgbaImage> {
    let path = textures.join(extra.path);
    let img = match crate::platform::assets::open_image(&path) {
        Ok(img) => img.to_rgba8(),
        Err(e) => {
            let Some(alt) = extra.alt_path.map(|alt| textures.join(alt)) else {
                eprintln!("[GuiAtlas] missing {}: {e}", path.display());
                return None;
            };
            match crate::platform::assets::open_image(&alt) {
                Ok(img) => img.to_rgba8(),
                Err(e) => {
                    eprintln!(
                        "[GuiAtlas] missing {} and {}: {e}",
                        path.display(),
                        alt.display()
                    );
                    return None;
                }
            }
        }
    };
    let filter = if extra.nearest {
        image::imageops::FilterType::Nearest
    } else {
        image::imageops::FilterType::CatmullRom
    };
    let (w, h) = extra.size;
    let mut img = if img.dimensions() == extra.size {
        img
    } else {
        image::imageops::resize(&img, w, h, filter)
    };
    if let Some(crop) = extra.crop_h
        && crop < h
    {
        img = image::imageops::crop(&mut img, 0, 0, w, crop).to_image();
    }
    Some(img)
}

pub fn build_gui_atlas(assets: &Path) -> GuiAtlas {
    let t0 = crate::platform::time::Instant::now();
    let gui_dir = assets.join("textures/gui");

    let mut entries: Vec<Entry> = Vec::new();

    entries.push(Entry {
        kind: Kind::White,
        key: String::new(),
        image: RgbaImage::from_pixel(1, 1, image::Rgba([255, 255, 255, 255])),
        scaling: Scaling::Stretch,
    });

    let sprites_dir = gui_dir.join("sprites");
    let mut sprite_files = Vec::new();
    collect_pngs(&sprites_dir, &sprites_dir, &mut sprite_files);
    for (key, path) in sprite_files {
        let Ok(img) = crate::platform::assets::open_gui_image(&path) else {
            continue;
        };
        let scaling = read_scaling(&path, img.width(), img.height());
        entries.push(Entry {
            kind: Kind::Sprite,
            key,
            image: img,
            scaling,
        });
    }

    let textures = assets.join("textures");

    let mob_effect_dir = textures.join("mob_effect");
    let mut effect_files = Vec::new();
    collect_pngs(&mob_effect_dir, &mob_effect_dir, &mut effect_files);
    for (name, path) in effect_files {
        let Ok(img) = crate::platform::assets::open_gui_image(&path) else {
            continue;
        };
        entries.push(Entry {
            kind: Kind::Sprite,
            key: format!("mob_effect/{name}"),
            image: img,
            scaling: Scaling::Stretch,
        });
    }

    for extra in EXTRAS {
        let Some(img) = load_extra(&textures, extra) else {
            continue;
        };
        entries.push(Entry {
            kind: Kind::Sprite,
            key: extra.key.to_string(),
            image: img,
            scaling: extra.scaling,
        });
    }

    for skin in &crate::gui::tablist::DEFAULT_SKINS {
        let path = textures.join(format!("entity/player/{}.png", skin.path));
        let Ok(img) = crate::platform::assets::open_image(&path) else {
            eprintln!("[GuiAtlas] missing skin {}", path.display());
            continue;
        };
        let mut img = img.to_rgba8();
        for (key, sx) in [(skin.face, SKIN_HEAD_U), (skin.hat, SKIN_HAT_U)] {
            if img.width() < sx + SKIN_HEAD_SIZE || img.height() < SKIN_HEAD_V + SKIN_HEAD_SIZE {
                continue;
            }
            entries.push(Entry {
                kind: Kind::Sprite,
                key: key.to_string(),
                image: image::imageops::crop(
                    &mut img,
                    sx,
                    SKIN_HEAD_V,
                    SKIN_HEAD_SIZE,
                    SKIN_HEAD_SIZE,
                )
                .to_image(),
                scaling: Scaling::Stretch,
            });
        }
    }

    for wood in crate::blockentities::render::sign::WOODS {
        let hanging = textures.join(format!("gui/hanging_signs/{wood}.png"));
        match crate::platform::assets::open_gui_image(&hanging) {
            Ok(img) => entries.push(Entry {
                kind: Kind::Sprite,
                key: crate::gui::sign_edit::hanging_sprite(wood),
                image: img,
                scaling: Scaling::Stretch,
            }),
            Err(_) => eprintln!("[GuiAtlas] missing {}", hanging.display()),
        }
        let plain = textures.join(format!("entity/signs/{wood}.png"));
        let Ok(img) = crate::platform::assets::open_image(&plain) else {
            eprintln!("[GuiAtlas] missing {}", plain.display());
            continue;
        };
        let mut img = img.to_rgba8();
        let k = (img.width() / crate::gui::sign_edit::SIGN_TEXTURE_WIDTH).max(1);
        for (key, [u, v, w, h]) in [
            (
                crate::gui::sign_edit::board_sprite(wood),
                crate::gui::sign_edit::BOARD_UV,
            ),
            (
                crate::gui::sign_edit::stick_sprite(wood),
                crate::gui::sign_edit::STICK_UV,
            ),
        ] {
            if img.width() < (u + w) * k || img.height() < (v + h) * k {
                continue;
            }
            entries.push(Entry {
                kind: Kind::Sprite,
                key,
                image: image::imageops::crop(&mut img, u * k, v * k, w * k, h * k).to_image(),
                scaling: Scaling::Stretch,
            });
        }
    }

    for sheet in SHEETS {
        let path = gui_dir.join(format!("{}.png", sheet.key));
        let Ok(mut img) = crate::platform::assets::open_gui_image(&path) else {
            eprintln!("[GuiAtlas] missing sheet {}", path.display());
            continue;
        };
        let (w, h) = (sheet.w.min(img.width()), sheet.h.min(img.height()));
        let image = if (w, h) == (img.width(), img.height()) {
            img
        } else {
            image::imageops::crop(&mut img, 0, 0, w, h).to_image()
        };
        entries.push(Entry {
            kind: Kind::Sheet,
            key: sheet.key.to_string(),
            image,
            scaling: Scaling::Stretch,
        });
    }

    entries.extend(bake_banner_layers(assets));

    if let Some(sga) = load_sga(assets) {
        entries.push(sga);
    }

    let (font, font_pixels) = Font::load(assets);
    let unihex = font_pixels.unihex;
    entries.push(Entry {
        kind: Kind::Font,
        key: String::new(),
        image: font_pixels.pages,
        scaling: Scaling::Stretch,
    });

    entries.push(Entry {
        kind: Kind::ServerIcons,
        key: String::new(),
        image: SERVER_ICON_GRID.blank(),
        scaling: Scaling::Stretch,
    });

    entries.push(Entry {
        kind: Kind::BannerIcons,
        key: String::new(),
        image: BANNER_ICON_GRID.blank(),
        scaling: Scaling::Stretch,
    });

    #[cfg(feature = "skins")]
    {
        entries.push(Entry {
            kind: Kind::PlayerFaces,
            key: String::new(),
            image: PLAYER_FACE_GRID.blank(),
            scaling: Scaling::Stretch,
        });
    }

    let icons: ItemIcons = bake_item_icons(assets);
    let icon_map = icons.map;
    entries.push(Entry {
        kind: Kind::Items,
        key: String::new(),
        image: icons.image,
        scaling: Scaling::Stretch,
    });

    let mut order: Vec<usize> = (0..entries.len()).collect();
    order.sort_by_key(|&i| std::cmp::Reverse(entries[i].image.height()));

    const WIDTH: u32 = 2048;
    const PAD: u32 = 1;
    let mut placements: Vec<Region> = vec![
        Region {
            x: 0,
            y: 0,
            w: 0,
            h: 0
        };
        entries.len()
    ];
    let (mut sx, mut sy, mut shelf_h) = (0u32, 0u32, 0u32);
    for &i in &order {
        let (w, h) = (entries[i].image.width(), entries[i].image.height());
        if sx + w > WIDTH {
            sx = 0;
            sy += shelf_h + PAD;
            shelf_h = 0;
        }
        placements[i] = Region { x: sx, y: sy, w, h };
        sx += w + PAD;
        shelf_h = shelf_h.max(h);
    }
    let height = (sy + shelf_h).max(64);

    let mut image = RgbaImage::new(WIDTH, height);
    let mut sprites = HashMap::new();
    let mut sheets = HashMap::new();
    let mut font_origin = (0, 0);
    let mut items_origin = (0, 0);
    let mut server_icons = SERVER_ICON_GRID;
    let mut banner_icons = BANNER_ICON_GRID;
    let mut player_faces = PLAYER_FACE_GRID;
    let mut white = Region {
        x: 0,
        y: 0,
        w: 1,
        h: 1,
    };

    for (i, entry) in entries.into_iter().enumerate() {
        let r = placements[i];
        blit(&mut image, &entry.image, r.x, r.y);
        match entry.kind {
            Kind::White => white = r,
            Kind::Sprite => {
                sprites.insert(
                    entry.key,
                    Sprite {
                        region: r,
                        scaling: entry.scaling,
                    },
                );
            }
            Kind::Sheet => {
                sheets.insert(entry.key, r);
            }
            Kind::Font => font_origin = (r.x, r.y),
            Kind::Items => items_origin = (r.x, r.y),
            Kind::ServerIcons => server_icons = SERVER_ICON_GRID.at((r.x, r.y)),
            Kind::BannerIcons => banner_icons = BANNER_ICON_GRID.at((r.x, r.y)),
            Kind::PlayerFaces => player_faces = PLAYER_FACE_GRID.at((r.x, r.y)),
        }
    }

    let items = icon_map
        .into_iter()
        .map(|(id, tiers)| {
            let regions = tiers
                .into_iter()
                .map(|(size, rect)| {
                    (
                        size,
                        Region {
                            x: items_origin.0 + rect[0],
                            y: items_origin.1 + rect[1],
                            w: rect[2],
                            h: rect[3],
                        },
                    )
                })
                .collect::<Vec<_>>();
            (id, regions)
        })
        .collect::<HashMap<_, _>>();

    println!(
        "[GuiAtlas] {}×{}, {} sprites, {} sheets, {} item icons, {:.0} ms",
        WIDTH,
        height,
        sprites.len(),
        sheets.len(),
        items.len(),
        t0.elapsed().as_secs_f32() * 1000.0
    );

    GuiAtlas {
        width: image.width(),
        height: image.height(),
        pixels: std::sync::Mutex::new(Some(AtlasPixels {
            rgba: image,
            unihex,
        })),
        sprites,
        sheets,
        items,
        font_origin,
        server_icons,
        banner_icons,
        player_faces,
        white,
        font,
    }
}

enum Kind {
    White,
    Sprite,
    Sheet,
    Font,
    Items,
    ServerIcons,
    BannerIcons,
    PlayerFaces,
}

struct Entry {
    kind: Kind,
    key: String,
    image: RgbaImage,
    scaling: Scaling,
}

fn collect_pngs(root: &Path, dir: &Path, out: &mut Vec<(String, std::path::PathBuf)>) {
    for path in crate::platform::assets::walk(dir) {
        if path.extension().and_then(|x| x.to_str()) == Some("png") {
            let rel = path.strip_prefix(root).unwrap().with_extension("");
            out.push((rel.to_string_lossy().replace('\\', "/"), path));
        }
    }
}

fn read_scaling(png_path: &Path, w: u32, h: u32) -> Scaling {
    let meta_path = png_path.with_extension("png.mcmeta");
    let Some(text) = crate::platform::assets::read_to_string(&meta_path) else {
        return Scaling::Stretch;
    };
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) else {
        return Scaling::Stretch;
    };
    let Some(s) = json.get("gui").and_then(|g| g.get("scaling")) else {
        return Scaling::Stretch;
    };
    let ty = s.get("type").and_then(|t| t.as_str()).unwrap_or("stretch");
    let num = |k: &str, d: u32| s.get(k).and_then(|v| v.as_u64()).unwrap_or(d as u64) as u32;
    match ty {
        "tile" => Scaling::Tile {
            w: num("width", w),
            h: num("height", h),
        },
        "nine_slice" => {
            let border = match s.get("border") {
                Some(serde_json::Value::Number(n)) => {
                    let b = n.as_u64().unwrap_or(0) as u32;
                    [b; 4]
                }
                Some(o) => {
                    let g = |k: &str| o.get(k).and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                    [g("left"), g("top"), g("right"), g("bottom")]
                }
                None => [0; 4],
            };
            Scaling::NineSlice {
                w: num("width", w),
                h: num("height", h),
                border,
                stretch_inner: s
                    .get("stretch_inner")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
            }
        }
        _ => Scaling::Stretch,
    }
}

fn blit(dst: &mut RgbaImage, src: &RgbaImage, x: u32, y: u32) {
    for (sx, sy, p) in src.enumerate_pixels() {
        let (dx, dy) = (x + sx, y + sy);
        if dx < dst.width() && dy < dst.height() {
            dst.put_pixel(dx, dy, *p);
        }
    }
}
