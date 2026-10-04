use std::path::PathBuf;

use image::{Rgba, RgbaImage};

use crate::gui::painter::Painter;
use crate::util::mth::{linear_to_srgb, srgb_to_linear};

pub fn rasterize(p: &Painter, width: u32, height: u32, background: [u8; 4]) -> RgbaImage {
    rasterize_scaled(p, width, height, 1, background)
}

pub fn rasterize_scaled(
    p: &Painter,
    width: u32,
    height: u32,
    scale: u32,
    background: [u8; 4],
) -> RgbaImage {
    let s = scale.max(1) as f32;
    let (width, height) = (width * scale.max(1), height * scale.max(1));
    let mut img = RgbaImage::from_pixel(width, height, Rgba(background));
    let guard = p.atlas.pixels();
    let Some(pixels) = &*guard else {
        return img;
    };
    let atlas = &pixels.rgba;
    let (aw, ah) = (atlas.width() as f32, atlas.height() as f32);

    for tri in p.indices.chunks_exact(3) {
        let idx = [tri[0] as usize, tri[1] as usize, tri[2] as usize];
        let pos: Vec<[f32; 2]> = idx
            .iter()
            .map(|&i| [p.positions[i][0] * s, p.positions[i][1] * s])
            .collect();
        let uv: Vec<[f32; 2]> = idx.iter().map(|&i| p.uvs[i]).collect();
        let col: Vec<[f32; 4]> = idx.iter().map(|&i| p.colors[i]).collect();

        let min_x = pos
            .iter()
            .map(|v| v[0])
            .fold(f32::MAX, f32::min)
            .floor()
            .max(0.0) as u32;
        let max_x = (pos.iter().map(|v| v[0]).fold(f32::MIN, f32::max).ceil() as i64)
            .clamp(0, width as i64) as u32;
        let min_y = pos
            .iter()
            .map(|v| v[1])
            .fold(f32::MAX, f32::min)
            .floor()
            .max(0.0) as u32;
        let max_y = (pos.iter().map(|v| v[1]).fold(f32::MIN, f32::max).ceil() as i64)
            .clamp(0, height as i64) as u32;

        let area = edge(&pos[0], &pos[1], &pos[2]);
        if area.abs() < 1e-6 {
            continue;
        }

        for y in min_y..max_y {
            for x in min_x..max_x {
                let pt = [x as f32 + 0.5, y as f32 + 0.5];
                let w0 = edge(&pos[1], &pos[2], &pt) / area;
                let w1 = edge(&pos[2], &pos[0], &pt) / area;
                let w2 = edge(&pos[0], &pos[1], &pt) / area;
                if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                    continue;
                }

                let u = w0 * uv[0][0] + w1 * uv[1][0] + w2 * uv[2][0];
                let v = w0 * uv[0][1] + w1 * uv[1][1] + w2 * uv[2][1];
                let tx = ((u * aw) as i64).clamp(0, atlas.width() as i64 - 1) as u32;
                let ty = ((v * ah) as i64).clamp(0, atlas.height() as i64 - 1) as u32;
                let texel = atlas.get_pixel(tx, ty).0;

                let mut src = [0.0f32; 4];
                for c in 0..3 {
                    let tex_linear = srgb_to_linear(texel[c] as f32 / 255.0);
                    let vc = w0 * col[0][c] + w1 * col[1][c] + w2 * col[2][c];
                    src[c] = tex_linear * vc;
                }
                src[3] =
                    (texel[3] as f32 / 255.0) * (w0 * col[0][3] + w1 * col[1][3] + w2 * col[2][3]);
                if src[3] <= 0.0 {
                    continue;
                }

                let dst = img.get_pixel(x, y).0;
                let mut out = [0u8; 4];
                for c in 0..3 {
                    let d = srgb_to_linear(dst[c] as f32 / 255.0);
                    let blended = src[c] * src[3] + d * (1.0 - src[3]);
                    out[c] = (linear_to_srgb(blended) * 255.0).round().clamp(0.0, 255.0) as u8;
                }
                let da = dst[3] as f32 / 255.0;
                out[3] = ((src[3] + da * (1.0 - src[3])) * 255.0)
                    .round()
                    .clamp(0.0, 255.0) as u8;
                img.put_pixel(x, y, Rgba(out));
            }
        }
    }
    img
}

pub fn upscale(img: &RgbaImage, factor: u32) -> RgbaImage {
    let mut out = RgbaImage::new(img.width() * factor, img.height() * factor);
    for (x, y, px) in out.enumerate_pixels_mut() {
        *px = *img.get_pixel(x / factor, y / factor);
    }
    out
}

pub fn output_dir() -> PathBuf {
    let dir = match std::env::var_os("MC_GUI_PREVIEW_DIR") {
        Some(d) => PathBuf::from(d),
        None => PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/scratch"),
    };
    let _ = std::fs::create_dir_all(&dir);
    dir
}

fn edge(a: &[f32; 2], b: &[f32; 2], c: &[f32; 2]) -> f32 {
    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::SharedMutex;
    use crate::gui::atlas::build_gui_atlas;
    use crate::gui::painter::Painter;
    use crate::gui::render::GuiInput;
    use crate::gui::{GuiState, Screen, screens};
    use crate::session::{Gamemode, SessionState, SharedState, SlotStack};
    use crate::text::{Span, Style, parse_formatted};
    use bevy::prelude::Vec2;

    fn stack(id: &'static str, count: u8) -> SlotStack {
        SlotStack {
            item: id,
            count,
            ..Default::default()
        }
    }

    fn demo_menu() -> Vec<SlotStack> {
        let mut slots = vec![SlotStack::default(); 46];
        slots[1] = stack("oak_planks", 4);
        slots[5] = stack("diamond_helmet", 1);
        slots[6] = stack("iron_chestplate", 1);
        for (i, id) in [
            "stone",
            "cobblestone",
            "oak_log",
            "diamond",
            "redstone",
            "torch",
            "glass",
            "sand",
            "gravel",
            "bricks",
            "oak_stairs",
            "chest",
            "crafting_table",
            "furnace",
            "apple",
            "bread",
            "golden_apple",
            "arrow",
        ]
        .iter()
        .enumerate()
        {
            slots[9 + i] = stack(id, ((i * 7) % 64 + 1) as u8);
        }
        for (i, id) in [
            "diamond_sword",
            "diamond_pickaxe",
            "bow",
            "cooked_beef",
            "ender_pearl",
            "water_bucket",
            "tnt",
            "oak_door",
            "torch",
        ]
        .iter()
        .enumerate()
        {
            slots[36 + i] = stack(id, if i > 2 { 12 } else { 1 });
        }
        slots[36].damage = 900;
        slots[36].max_damage = 1561;
        slots
    }

    fn shared_with(menu: Vec<SlotStack>, gamemode: Gamemode) -> Arc<SharedMutex> {
        Arc::new(SharedMutex::new(SharedState {
            in_world: true,
            session: SessionState {
                hotbar: Arc::from(&menu[36..46]),
                menu_slots: menu,
                hotbar_selected: 2,
                health: 15.0,
                food: 7,
                gamemode,
                ..Default::default()
            },
            ..Default::default()
        }))
    }

    fn input(vw: f32, vh: f32, mouse: Option<Vec2>) -> GuiInput {
        GuiInput {
            size: Vec2::new(vw, vh),
            scale: 3.0,
            mouse,
            ..Default::default()
        }
    }

    const SCALE: u32 = 3;

    fn save(img: &RgbaImage, name: &str) {
        let path = output_dir().join(name);
        img.save(&path).unwrap();
        println!("[preview] wrote {}", path.display());
    }

    #[test]
    fn preview_item_scales() {
        let atlas = build_gui_atlas(&crate::assets_root());
        let ids = [
            "diamond_sword",
            "apple",
            "stick",
            "iron_ingot",
            "bread",
            "arrow",
            "stone",
            "grass_block",
            "oak_stairs",
            "chest",
            "crafting_table",
            "torch",
        ];
        for scale in [2u32, 3, 4, 6] {
            let mut p = Painter::new(&atlas, 0);
            let (w, h) = (ids.len() as f32 * 18.0 + 2.0, 22.0);
            p.fill(0.0, 0.0, w, h, 0xFF30_3030);
            for (i, id) in ids.iter().enumerate() {
                p.item_icon(id, i as f32 * 18.0 + 3.0, 3.0);
            }
            let img = rasterize_scaled(&p, w as u32, h as u32, scale, [0, 0, 0, 255]);
            save(&upscale(&img, 2), &format!("icons_scale{}.png", scale));
        }
    }

    #[cfg(feature = "click_gui")]
    #[test]
    fn preview_click_gui() {
        use crate::gui::clickgui;

        let atlas = build_gui_atlas(&crate::assets_root());
        let (vw, vh) = (560.0_f32, 300.0_f32);
        fn ctx(input: &GuiInput, vw: f32, vh: f32) -> crate::gui::ScreenCtx<'_> {
            crate::gui::ScreenCtx { input, vw, vh }
        }

        let mut st = clickgui::ClickGuiState::default();
        macro_rules! click_gui {
            ($p:expr, $inp:expr) => {
                clickgui::draw($p, &mut st, &ctx($inp, vw, vh))
            };
        }
        let mut inp = input(vw, vh, Some(Vec2::new(60.0, 60.0)));
        inp.time = 100.0;
        let mut p = Painter::new(&atlas, 0);
        click_gui!(&mut p, &inp);
        inp.time = 100.5;
        let mut p = Painter::new(&atlas, 0);
        click_gui!(&mut p, &inp);
        save(
            &rasterize_scaled(&p, vw as u32, vh as u32, SCALE, [70, 110, 170, 255]),
            "gui_clickgui.png",
        );

        let rows_top = 4.0 + 16.0 + 4.0 + 15.0;
        let row_y = rows_top + 13.0 + 6.0;
        let settings_top = rows_top + 2.0 * 13.0 + 3.0;

        let mut inp = input(vw, vh, Some(Vec2::new(50.0, row_y)));
        inp.right_click = true;
        inp.time = 101.0;
        let mut p = Painter::new(&atlas, 0);
        click_gui!(&mut p, &inp);
        let mut inp = input(vw, vh, Some(Vec2::new(60.0, settings_top + 6.0)));
        inp.left_click = true;
        inp.time = 101.05;
        let mut p = Painter::new(&atlas, 0);
        click_gui!(&mut p, &inp);
        let mut inp = input(vw, vh, Some(Vec2::new(50.0, row_y)));
        inp.time = 101.1;
        let mut p = Painter::new(&atlas, 0);
        click_gui!(&mut p, &inp);
        save(
            &rasterize_scaled(&p, vw as u32, vh as u32, SCALE, [70, 110, 170, 255]),
            "gui_clickgui_expanded.png",
        );

        let drop_y = settings_top + 4.0 * 12.0 + 19.0 + 12.0 + 19.0 + 5.0;
        let mut inp = input(vw, vh, Some(Vec2::new(50.0, drop_y)));
        inp.left_click = true;
        inp.time = 101.2;
        let mut p = Painter::new(&atlas, 0);
        click_gui!(&mut p, &inp);
        let mut inp = input(vw, vh, Some(Vec2::new(50.0, drop_y)));
        inp.time = 101.3;
        let mut p = Painter::new(&atlas, 0);
        click_gui!(&mut p, &inp);
        save(
            &rasterize_scaled(&p, vw as u32, vh as u32, SCALE, [70, 110, 170, 255]),
            "gui_clickgui_dropdown.png",
        );

        let mut st = clickgui::ClickGuiState::default();
        let mut inp = input(vw, vh, None);
        inp.time = 200.0;
        let mut p = Painter::new(&atlas, 0);
        click_gui!(&mut p, &inp);
        let mut inp = input(vw, vh, None);
        inp.time = 200.5;
        inp.typed = "es".chars().collect();
        let mut p = Painter::new(&atlas, 0);
        click_gui!(&mut p, &inp);
        let mut inp = input(vw, vh, None);
        inp.time = 200.6;
        inp.down_arrow = true;
        let mut p = Painter::new(&atlas, 0);
        click_gui!(&mut p, &inp);
        save(
            &rasterize_scaled(&p, vw as u32, vh as u32, SCALE, [70, 110, 170, 255]),
            "gui_clickgui_search.png",
        );

        let tab_x = vw / 2.0 + 20.0;
        let mut inp = input(vw, vh, Some(Vec2::new(tab_x, 10.0)));
        inp.left_click = true;
        inp.time = 201.0;
        let mut p = Painter::new(&atlas, 0);
        click_gui!(&mut p, &inp);
        let mut inp = input(vw, vh, Some(Vec2::new(tab_x, 10.0)));
        inp.time = 201.1;
        let mut p = Painter::new(&atlas, 0);
        click_gui!(&mut p, &inp);
        save(
            &rasterize_scaled(&p, vw as u32, vh as u32, SCALE, [70, 110, 170, 255]),
            "gui_clickgui_config.png",
        );
    }

    #[test]
    fn preview_screens() {
        let atlas = build_gui_atlas(&crate::assets_root());
        let (vw, vh) = (427.0_f32, 240.0_f32);

        let shared = shared_with(demo_menu(), Gamemode::Survival);
        let mut state = GuiState::default();
        let mut p = Painter::new(&atlas, 0);
        screens::draw(
            &mut p,
            &mut state,
            &input(vw, vh, None),
            &shared,
            #[cfg(feature = "skins")]
            &mut Default::default(),
        );
        save(
            &rasterize_scaled(&p, vw as u32, vh as u32, SCALE, [70, 110, 170, 255]),
            "gui_hud.png",
        );

        let mut state = GuiState {
            screen: Screen::Inventory,
            ..Default::default()
        };
        let mut p = Painter::new(&atlas, 0);
        let mouse = Some(Vec2::new(vw / 2.0 - 80.0 + 8.0, vh / 2.0 - 83.0 + 92.0));
        screens::draw(
            &mut p,
            &mut state,
            &input(vw, vh, mouse),
            &shared,
            #[cfg(feature = "skins")]
            &mut Default::default(),
        );
        save(
            &rasterize_scaled(&p, vw as u32, vh as u32, SCALE, [70, 110, 170, 255]),
            "gui_inventory.png",
        );

        let shared = shared_with(demo_menu(), Gamemode::Creative);
        let mut state = GuiState {
            screen: Screen::Creative,
            ..Default::default()
        };
        let mut p = Painter::new(&atlas, 0);
        let mouse = Some(Vec2::new(vw / 2.0 - 50.0, vh / 2.0 - 30.0));
        screens::draw(
            &mut p,
            &mut state,
            &input(vw, vh, mouse),
            &shared,
            #[cfg(feature = "skins")]
            &mut Default::default(),
        );
        save(
            &rasterize_scaled(&p, vw as u32, vh as u32, SCALE, [70, 110, 170, 255]),
            "gui_creative.png",
        );

        let mut menu = demo_menu();
        let mut rich = stack("diamond_sword", 1);
        rich.damage = 900;
        rich.max_damage = 1561;
        rich.custom_name = Some(parse_formatted("\u{a7}bStormbringer"));
        rich.lore = vec![
            parse_formatted("Forged in the deep dark"),
            parse_formatted("\u{a7}cCursed"),
        ];
        rich.enchantments = vec![
            ("sharpness".into(), 5),
            ("unbreaking".into(), 3),
            ("binding_curse".into(), 1),
        ];
        rich.unbreakable = false;
        rich.component_count = 4;
        rich.nbt_bytes = 1462;
        menu[9] = rich;
        let shared = shared_with(menu, Gamemode::Survival);
        let mut state = GuiState {
            screen: Screen::Inventory,
            advanced_tooltips: true,
            ..Default::default()
        };
        let mut p = Painter::new(&atlas, 0);
        let left = ((vw - 176.0) / 2.0).floor();
        let top = ((vh - 166.0) / 2.0).floor();
        let mouse = Some(Vec2::new(left + 8.0 + 4.0, top + 84.0 + 4.0));
        screens::draw(
            &mut p,
            &mut state,
            &input(vw, vh, mouse),
            &shared,
            #[cfg(feature = "skins")]
            &mut Default::default(),
        );
        save(
            &rasterize_scaled(&p, vw as u32, vh as u32, SCALE, [70, 110, 170, 255]),
            "gui_tooltip.png",
        );

        let ids = [
            "stone",
            "torch",
            "cobblestone",
            "grass_block",
            "oak_stairs",
            "chest",
            "diamond_sword",
            "apple",
            "glass",
            "oak_door",
            "cake",
            "end_rod",
        ];
        let mut p = Painter::new(&atlas, 0);
        p.fill(0.0, 0.0, 12.0 * 34.0, 60.0, 0xFF404040);
        for (i, id) in ids.iter().enumerate() {
            let x = i as f32 * 34.0 + 1.0;
            let r = atlas.item(id).unwrap();
            p.blit_region_debug(r, x, 2.0, 32.0, 32.0);
            p.item_icon(id, x + 8.0, 38.0);
        }
        save(
            &rasterize_scaled(&p, 12 * 34, 60, SCALE, [0, 0, 0, 255]),
            "gui_icons.png",
        );

        let mut p = Painter::new(&atlas, 0);
        p.fill(0.0, 0.0, 320.0, 120.0, 0xFF202020);
        let mut y = 4.0;
        for row in [
            "\u{a7}0black \u{a7}1blue \u{a7}2green \u{a7}3aqua \u{a7}4red \u{a7}5purple",
            "\u{a7}6gold \u{a7}7gray \u{a7}8dgray \u{a7}9blue \u{a7}aglue \u{a7}baqua",
            "\u{a7}cred \u{a7}dpink \u{a7}eyellow \u{a7}fwhite",
            "\u{a7}lbold \u{a7}r\u{a7}oitalic \u{a7}r\u{a7}nunderline \u{a7}r\u{a7}mstrike",
            "\u{a7}kobfuscated\u{a7}r normal",
            "\u{a7}c\u{a7}lbold red \u{a7}r\u{a7}9\u{a7}oitalic blue",
            "Accented: aeiou AEIOU 0123456789 !?@#$%",
        ] {
            p.text(&parse_formatted(row), 4.0, y, true);
            y += 12.0;
        }
        p.text(
            &[Span {
                text: "no shadow, plain white".into(),
                style: Style::default(),
            }],
            4.0,
            y,
            false,
        );
        save(
            &rasterize_scaled(&p, 320, 120, SCALE, [0, 0, 0, 255]),
            "gui_text.png",
        );
    }
}
