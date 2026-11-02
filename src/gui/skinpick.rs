use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use base64::Engine as _;
use image::RgbaImage;

use crate::client::skins::SkinRef;
use crate::gui::ScreenCtx;
use crate::gui::painter::Painter;
use crate::gui::widgets::{Button, WIDGET_HEIGHT};
use crate::renderer::anim::AnimSample;

const PRESETS: [&str; 24] = [
    "Default Steve",
    "Default Alex",
    "Tennis Steve",
    "Tennis Alex",
    "Tuxedo Steve",
    "Tuxedo Alex",
    "Athlete Steve",
    "Athlete Alex",
    "Cyclist Steve",
    "Cyclist Alex",
    "Boxer Steve",
    "Boxer Alex",
    "Prisoner Steve",
    "Prisoner Alex",
    "Scottish Steve",
    "Scottish Alex",
    "Developer Steve",
    "Developer Alex",
    "Herobrine",
    "Notch",
    "Creeper",
    "Zombie",
    "Pig",
    "Mooshroom",
];
const PRESET_PNGS: [&[u8]; 24] = [
    include_bytes!("../eagler/skins/preset_0.png"),
    include_bytes!("../eagler/skins/preset_1.png"),
    include_bytes!("../eagler/skins/preset_2.png"),
    include_bytes!("../eagler/skins/preset_3.png"),
    include_bytes!("../eagler/skins/preset_4.png"),
    include_bytes!("../eagler/skins/preset_5.png"),
    include_bytes!("../eagler/skins/preset_6.png"),
    include_bytes!("../eagler/skins/preset_7.png"),
    include_bytes!("../eagler/skins/preset_8.png"),
    include_bytes!("../eagler/skins/preset_9.png"),
    include_bytes!("../eagler/skins/preset_10.png"),
    include_bytes!("../eagler/skins/preset_11.png"),
    include_bytes!("../eagler/skins/preset_12.png"),
    include_bytes!("../eagler/skins/preset_13.png"),
    include_bytes!("../eagler/skins/preset_14.png"),
    include_bytes!("../eagler/skins/preset_15.png"),
    include_bytes!("../eagler/skins/preset_16.png"),
    include_bytes!("../eagler/skins/preset_17.png"),
    include_bytes!("../eagler/skins/preset_18.png"),
    include_bytes!("../eagler/skins/preset_19.png"),
    include_bytes!("../eagler/skins/preset_20.png"),
    include_bytes!("../eagler/skins/preset_21.png"),
    include_bytes!("../eagler/skins/preset_22.png"),
    include_bytes!("../eagler/skins/preset_23.png"),
];
const WORN_URL: &str = "eagler:self";
const GAP: f32 = 4.0;
const ARROW_W: f32 = 20.0;
pub const HEIGHT: f32 = 12.0 + WIDGET_HEIGHT + 2.0 * (WIDGET_HEIGHT + GAP);

#[cfg(not(target_arch = "wasm32"))]
static DROPPED: Mutex<Vec<std::path::PathBuf>> = Mutex::new(Vec::new());

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn collect_drops(
    mut drops: bevy::prelude::MessageReader<bevy::window::FileDragAndDrop>,
    state: bevy::prelude::Res<crate::gui::GuiState>,
) {
    let on_form = state.screen == crate::gui::Screen::EditProfile;
    for drop in drops.read() {
        if let bevy::window::FileDragAndDrop::DroppedFile { path_buf, .. } = drop
            && on_form
        {
            DROPPED.lock().unwrap().push(path_buf.clone());
        }
    }
}

struct Skin {
    name: String,
    slim: bool,
    rgba: Arc<Vec<u8>>,
}

#[derive(Default)]
pub struct SkinPick {
    loaded: bool,
    skins: Vec<Skin>,
    picks: HashMap<String, String>,
    index: usize,
    rejected: bool,
}

impl SkinPick {
    pub fn open(&mut self, account: Option<&str>) {
        if !self.loaded {
            self.loaded = true;
            let (picks, uploads) = read();
            self.picks = picks;
            self.skins = (0..PRESETS.len())
                .map(|id| Skin {
                    name: PRESETS[id].into(),
                    slim: preset_slim(id),
                    rgba: Arc::new(preset(id)),
                })
                .chain(uploads)
                .collect();
        }
        self.index = account
            .and_then(|a| self.picks.get(a))
            .and_then(|n| self.skins.iter().position(|s| &s.name == n))
            .unwrap_or(0);
    }

    pub fn commit(&mut self, old: Option<&str>, name: &str) {
        let Some(skin) = self.skins.get(self.index) else {
            return;
        };
        if let Some(old) = old {
            self.picks.remove(old);
        }
        self.picks.insert(name.to_string(), skin.name.clone());
        self.save();
    }

    pub fn draw(
        &mut self,
        p: &mut Painter,
        ctx: &ScreenCtx,
        x: f32,
        y: f32,
        w: f32,
        preview: [f32; 4],
    ) {
        if !self.loaded {
            self.open(None);
        }
        let n = self.skins.len();
        p.text_str("Skin", x + 1.0, y, 0xA0A0A0, true);
        let row = y + 12.0;
        if Button::new(x, row, ARROW_W, WIDGET_HEIGHT, "<").draw(p, ctx) {
            self.index = (self.index + n - 1) % n;
        }
        if Button::new(x + w - ARROW_W, row, ARROW_W, WIDGET_HEIGHT, ">").draw(p, ctx) {
            self.index = (self.index + 1) % n;
        }
        let skin = &self.skins[self.index];
        let model = if skin.slim { "Slim" } else { "Classic" };
        let label = format!("{} ({model})", skin.name);
        let mid_w = w - 2.0 * (ARROW_W + GAP);
        Button::new(x + ARROW_W + GAP, row, mid_w, WIDGET_HEIGHT, &label).draw(p, ctx);

        let uploaded = self.index >= PRESETS.len();
        let half = (w - GAP) / 2.0;
        let row = row + WIDGET_HEIGHT + GAP;
        #[cfg(target_arch = "wasm32")]
        if Button::new(x, row, half, WIDGET_HEIGHT, "Upload Skin...").draw(p, ctx) {
            crate::platform::upload::choose_png();
        }
        #[cfg(not(target_arch = "wasm32"))]
        Button {
            x,
            y: row,
            w: half,
            h: WIDGET_HEIGHT,
            label: "Drop Skin File",
            active: false,
        }
        .draw(p, ctx);
        let label = format!("Model: {model}");
        let toggle = Button {
            x: x + half + GAP,
            y: row,
            w: half,
            h: WIDGET_HEIGHT,
            label: &label,
            active: uploaded,
        };
        if toggle.draw(p, ctx) {
            self.skins[self.index].slim ^= true;
            self.save();
        }
        let row = row + WIDGET_HEIGHT + GAP;
        let delete = Button {
            x,
            y: row,
            w: half,
            h: WIDGET_HEIGHT,
            label: "Delete Skin",
            active: uploaded,
        };
        if delete.draw(p, ctx) {
            self.skins.remove(self.index);
            self.index = 0;
            self.save();
        }
        if self.rejected {
            p.text_str(
                "Not a skin PNG",
                x + half + GAP + 4.0,
                row + 6.0,
                0xFF5555,
                true,
            );
        }
        #[cfg(target_arch = "wasm32")]
        if let Some((file, bytes)) = crate::platform::upload::take_png() {
            self.add(&file, &bytes);
        }
        #[cfg(not(target_arch = "wasm32"))]
        for path in std::mem::take(&mut *DROPPED.lock().unwrap()) {
            match std::fs::read(&path) {
                Ok(bytes) => self.add(&path.to_string_lossy(), &bytes),
                Err(e) => {
                    crate::log_warn!("skins", "could not read {}: {e}", path.display());
                    self.rejected = true;
                }
            }
        }

        self.preview(p, ctx, preview);
    }

    fn add(&mut self, file: &str, bytes: &[u8]) {
        let Some(rgba) = decode(bytes) else {
            crate::log_warn!("skins", "{file}: not a 64x64 or 64x32 PNG");
            self.rejected = true;
            return;
        };
        self.rejected = false;
        let stem = std::path::Path::new(file)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Skin");
        let mut name = stem.to_string();
        for n in 2.. {
            if !self.skins.iter().any(|s| s.name == name) {
                break;
            }
            name = format!("{stem} ({n})");
        }
        self.skins.push(Skin {
            name,
            slim: false,
            rgba: Arc::new(rgba),
        });
        self.index = self.skins.len() - 1;
        self.save();
    }

    fn save(&self) {
        let skins: Vec<serde_json::Value> = self.skins[PRESETS.len()..]
            .iter()
            .filter_map(|s| {
                let mut png = std::io::Cursor::new(Vec::new());
                RgbaImage::from_raw(64, 64, s.rgba.to_vec())?
                    .write_to(&mut png, image::ImageFormat::Png)
                    .ok()?;
                let png = base64::engine::general_purpose::STANDARD.encode(png.into_inner());
                Some(serde_json::json!({ "name": s.name, "slim": s.slim, "png": png }))
            })
            .collect();
        let doc = serde_json::json!({ "picks": self.picks, "skins": skins });
        crate::platform::storage::SKINS.store(&doc.to_string());
    }

    fn preview(&mut self, p: &mut Painter, ctx: &ScreenCtx, [x, y, w, h]: [f32; 4]) {
        let skin = &self.skins[self.index];
        crate::gui::puppet::show_skin(&skin.rgba);
        let figure = crate::gui::puppet::Figure {
            sample: &AnimSample::default(),
            slim: skin.slim,
            parts: u8::MAX,
            armor: [false; 4],
        };
        let size = h / 34.0 * 16.0 / crate::renderer::player_model::AVATAR_SCALE;
        crate::gui::puppet::follows_mouse(
            p,
            &figure,
            [x, y, x + w, y + h],
            size,
            0.0625,
            ctx.mouse(),
        );
    }
}

fn preset(id: usize) -> Vec<u8> {
    decode(PRESET_PNGS[id]).unwrap_or_else(|| vec![0; 64 * 64 * 4])
}

fn preset_slim(id: usize) -> bool {
    id < 18 && id % 2 == 1
}

pub(crate) fn preset_image(id: i32) -> Option<RgbaImage> {
    let id = usize::try_from(id).ok().filter(|&id| id < PRESETS.len())?;
    RgbaImage::from_raw(64, 64, preset(id))
}

pub(crate) fn preset_is_slim(id: i32) -> bool {
    usize::try_from(id).is_ok_and(preset_slim)
}

fn decode(bytes: &[u8]) -> Option<Vec<u8>> {
    let image = image::load_from_memory(bytes).ok()?.to_rgba8();
    Some(crate::client::skins::process_legacy_skin(image, "skin upload")?.into_raw())
}

fn read() -> (HashMap<String, String>, Vec<Skin>) {
    let doc: serde_json::Value = crate::platform::storage::SKINS
        .load()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default();
    let picks = doc
        .get("picks")
        .and_then(|p| p.as_object())
        .map(|o| {
            o.iter()
                .filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_string())))
                .collect()
        })
        .unwrap_or_default();
    let skins = doc
        .get("skins")
        .and_then(|s| s.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|s| {
                    let png = base64::engine::general_purpose::STANDARD
                        .decode(s.get("png")?.as_str()?)
                        .ok()?;
                    Some(Skin {
                        name: s.get("name")?.as_str()?.to_string(),
                        slim: s.get("slim").and_then(|v| v.as_bool()).unwrap_or(false),
                        rgba: Arc::new(decode(&png)?),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    (picks, skins)
}

pub(crate) struct Picked {
    pub(crate) wire: Vec<u8>,
    slim: bool,
    image: RgbaImage,
}

pub(crate) fn for_account(name: &str) -> Option<Picked> {
    let (picks, uploads) = read();
    let pick = picks.get(name)?;
    if let Some(id) = PRESETS.iter().position(|p| p == pick) {
        let mut wire = vec![1];
        wire.extend((id as u32).to_be_bytes());
        let image = RgbaImage::from_raw(64, 64, preset(id))?;
        return Some(Picked {
            wire,
            slim: preset_slim(id),
            image,
        });
    }
    let skin = uploads.into_iter().find(|s| &s.name == pick)?;
    let mut wire = vec![2, skin.slim as u8];
    wire.extend(
        skin.rgba
            .chunks_exact(4)
            .flat_map(|p| [p[2], p[1], (p[0] >> 1) | (p[3] & 0x80)]),
    );
    let image = RgbaImage::from_raw(64, 64, Arc::unwrap_or_clone(skin.rgba))?;
    Some(Picked {
        wire,
        slim: skin.slim,
        image,
    })
}

static WORN: Mutex<Option<(u128, SkinRef)>> = Mutex::new(None);

pub(crate) fn wear(picked: Option<Picked>, uuid: u128) {
    let worn = picked.map(|p| {
        crate::client::skins::provide(WORN_URL.into(), p.image);
        (
            uuid,
            SkinRef {
                body: Some(WORN_URL.into()),
                slim: p.slim,
                ..SkinRef::default()
            },
        )
    });
    *WORN.lock().unwrap() = worn;
}

pub(crate) fn worn() -> Option<(SkinRef, u8)> {
    WORN.lock()
        .unwrap()
        .as_ref()
        .map(|(_, refs)| (refs.clone(), 0))
}

pub(crate) fn worn_by(uuid: u128) -> Option<SkinRef> {
    WORN.lock()
        .unwrap()
        .as_ref()
        .filter(|(u, _)| *u == uuid)
        .map(|(_, refs)| refs.clone())
}
