#![allow(dead_code)]

use std::collections::HashMap;
use std::ops::Range;
use std::path::{Path, PathBuf};

use image::RgbaImage;

use crate::util::zip;

pub mod events;
pub mod texts;

pub const LINE_HEIGHT: f32 = 9.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Glyph {
    pub rect: [u32; 4],
    pub draw_w: f32,
    pub draw_h: f32,
    pub y_off: f32,
    pub advance: f32,
}

pub struct FontPixels {
    pub pages: RgbaImage,
    pub unihex: Vec<u8>,
}

pub struct Font {
    glyphs: HashMap<char, Glyph>,
    ascii: [Option<Glyph>; 128],
    unihex_glyphs: HashMap<char, Glyph>,
    pub unihex_dims: (u32, u32),
    by_advance: HashMap<i32, Vec<char>>,
    missing: Glyph,
    empty_chars: Vec<char>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct Style {
    pub color: u32,
    pub event: u16,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikethrough: bool,
    pub obfuscated: bool,
}

impl Default for Style {
    fn default() -> Style {
        Style {
            color: 0xFFFFFF,
            event: 0,
            bold: false,
            italic: false,
            underline: false,
            strikethrough: false,
            obfuscated: false,
        }
    }
}

impl Style {
    pub fn colored(rgb: u32) -> Style {
        Style {
            color: rgb,
            ..Style::default()
        }
    }

    pub fn is_default(&self) -> bool {
        *self == Style::default()
    }

    pub fn shadow_color(rgb: u32) -> u32 {
        (rgb & 0xFC_FC_FC) >> 2
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Span {
    pub text: String,
    pub style: Style,
}

pub const FORMAT_COLORS: [u32; 16] = [
    0x000000, 0x0000AA, 0x00AA00, 0x00AAAA, 0xAA0000, 0xAA00AA, 0xFFAA00, 0xAAAAAA, 0x555555,
    0x5555FF, 0x55FF55, 0x55FFFF, 0xFF5555, 0xFF55FF, 0xFFFF55, 0xFFFFFF,
];

const COLOR_NAMES: [&str; 16] = [
    "black",
    "dark_blue",
    "dark_green",
    "dark_aqua",
    "dark_red",
    "dark_purple",
    "gold",
    "gray",
    "dark_gray",
    "blue",
    "green",
    "aqua",
    "red",
    "light_purple",
    "yellow",
    "white",
];

pub fn named_color(name: &str) -> Option<u32> {
    let clean = |s: &str| -> String {
        s.chars()
            .filter(|c| c.is_ascii_alphabetic())
            .map(|c| c.to_ascii_lowercase())
            .collect()
    };
    let want = clean(name);
    COLOR_NAMES
        .iter()
        .position(|n| clean(n) == want)
        .map(|i| FORMAT_COLORS[i])
}

pub fn parse_formatted(s: &str) -> Vec<Span> {
    parse_formatted_from(s, Style::default())
}

pub fn parse_formatted_from(s: &str, style: Style) -> Vec<Span> {
    let mut out: Vec<Span> = Vec::new();
    let mut style = style;
    let mut buf = String::new();
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        if c != '\u{a7}' {
            buf.push(c);
            continue;
        }
        let Some(code) = it.next() else { break };
        let code = code.to_ascii_lowercase();
        let mut next = style;
        match code {
            '0'..='9' | 'a'..='f' => {
                let idx = code.to_digit(16).unwrap() as usize;
                next = Style {
                    event: style.event,
                    ..Style::colored(FORMAT_COLORS[idx])
                };
            }
            'k' => next.obfuscated = true,
            'l' => next.bold = true,
            'm' => next.strikethrough = true,
            'n' => next.underline = true,
            'o' => next.italic = true,
            'r' => {
                next = Style {
                    event: style.event,
                    ..Style::default()
                }
            }
            _ => continue,
        }
        if next != style {
            if !buf.is_empty() {
                out.push(Span {
                    text: std::mem::take(&mut buf),
                    style,
                });
            }
            style = next;
        }
    }
    if !buf.is_empty() {
        out.push(Span { text: buf, style });
    }
    out
}

pub fn styled(s: &str, style: Style) -> Vec<Span> {
    if s.is_empty() {
        Vec::new()
    } else {
        vec![Span {
            text: s.to_string(),
            style,
        }]
    }
}

enum Provider {
    Space(Vec<(char, f32)>),
    Bitmap {
        file: PathBuf,
        height: i32,
        ascent: i32,
        chars: Vec<Vec<char>>,
    },
    Unihex(PathBuf),
}

fn font_json_path(assets: &Path, id: &str) -> PathBuf {
    let name = id.split_once(':').map(|(_, p)| p).unwrap_or(id);
    assets.join("font").join(format!("{name}.json"))
}

fn texture_path(assets: &Path, id: &str) -> PathBuf {
    let name = id.split_once(':').map(|(_, p)| p).unwrap_or(id);
    assets.join("textures").join(name)
}

fn hex_zip_path(assets: &Path, id: &str) -> PathBuf {
    let name = id.split_once(':').map(|(_, p)| p).unwrap_or(id);
    assets.join(name)
}

fn hex_decode(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok())
        .collect()
}

fn resolve(assets: &Path, id: &str, out: &mut Vec<Provider>) {
    let path = font_json_path(assets, id);
    let mut parsed = false;
    for text in crate::platform::assets::read_stack(&path) {
        match serde_json::from_str::<serde_json::Value>(&text) {
            Ok(json) => {
                parsed = true;
                resolve_file(assets, &json, out);
            }
            Err(e) => crate::log_warn!("font", "font definition {}: {e}", path.display()),
        }
    }
    if !parsed {
        panic!("font definition {} is missing", path.display());
    }
}

fn resolve_file(assets: &Path, json: &serde_json::Value, out: &mut Vec<Provider>) {
    let providers = json["providers"].as_array().cloned().unwrap_or_default();
    for p in providers.iter().rev() {
        match p["type"].as_str().unwrap_or("") {
            "reference" => {
                if let Some(id) = p["id"].as_str() {
                    resolve(assets, id, out);
                }
            }
            "space" => {
                let mut advances = Vec::new();
                if let Some(map) = p["advances"].as_object() {
                    for (k, v) in map {
                        if let Some(c) = k.chars().next() {
                            advances.push((c, v.as_f64().unwrap_or(0.0) as f32));
                        }
                    }
                }
                out.push(Provider::Space(advances));
            }
            "bitmap" => {
                let (Some(rows), Some(file)) = (p["chars"].as_array(), p["file"].as_str()) else {
                    continue;
                };
                let chars = rows
                    .iter()
                    .map(|row| row.as_str().unwrap_or("").chars().collect::<Vec<char>>())
                    .collect();
                out.push(Provider::Bitmap {
                    file: texture_path(assets, file),
                    height: p["height"].as_i64().unwrap_or(8) as i32,
                    ascent: p["ascent"].as_i64().unwrap_or(7) as i32,
                    chars,
                });
            }
            "unihex" if cfg!(feature = "full_font") => {
                if let Some(id) = p["hex_file"].as_str() {
                    out.push(Provider::Unihex(hex_zip_path(assets, id)));
                }
            }
            _ => {}
        }
    }
}

impl Font {
    pub fn load(assets: &Path) -> (Font, FontPixels) {
        let mut providers = Vec::new();
        resolve(assets, "minecraft:default", &mut providers);
        providers.reverse();

        let mut pages: Vec<(PathBuf, RgbaImage)> = Vec::new();
        let mut page_y: HashMap<PathBuf, u32> = HashMap::new();
        let mut unreadable: Vec<PathBuf> = Vec::new();
        for p in &providers {
            if let Provider::Bitmap { file, .. } = p {
                if page_y.contains_key(file) || unreadable.contains(file) {
                    continue;
                }
                let img = match crate::platform::assets::open_image(file) {
                    Ok(img) => img.to_rgba8(),
                    Err(e) => {
                        crate::log_warn!("font", "font page {}: {e}", file.display());
                        unreadable.push(file.clone());
                        continue;
                    }
                };
                page_y.insert(file.clone(), pages.iter().map(|(_, i)| i.height()).sum());
                pages.push((file.clone(), img));
            }
        }
        providers
            .retain(|p| !matches!(p, Provider::Bitmap { file, .. } if unreadable.contains(file)));
        let mut unihex_paths: Vec<PathBuf> = Vec::new();
        for p in &providers {
            if let Provider::Unihex(path) = p {
                if !unihex_paths.contains(path) {
                    unihex_paths.push(path.clone());
                }
            }
        }
        let mut unihex_glyphs: Vec<(char, u32, Vec<u8>)> = Vec::new();
        let mut unihex_seen: std::collections::HashSet<char> = std::collections::HashSet::new();
        for path in &unihex_paths {
            let Some(zip_bytes) = crate::platform::assets::read(path) else {
                continue;
            };
            let Ok(entries) = zip::entries(&zip_bytes) else {
                continue;
            };
            for entry in entries.iter().filter(|e| e.name.ends_with(".hex")) {
                let Ok(bytes) = zip::read(&zip_bytes, entry) else {
                    continue;
                };
                for line in String::from_utf8_lossy(&bytes).lines() {
                    let Some((cp_hex, data_hex)) = line.split_once(':') else {
                        continue;
                    };
                    let Ok(cp) = u32::from_str_radix(cp_hex.trim(), 16) else {
                        continue;
                    };
                    let Some(c) = char::from_u32(cp) else {
                        continue;
                    };
                    if !unihex_seen.insert(c) {
                        continue;
                    }
                    let Some(raw) = hex_decode(data_hex.trim()) else {
                        continue;
                    };
                    if raw.len() != 16 && raw.len() != 32 {
                        continue;
                    }
                    unihex_glyphs.push((c, if raw.len() == 16 { 8 } else { 16 }, raw));
                }
            }
        }

        let total_w = pages
            .iter()
            .map(|(_, i)| i.width())
            .max()
            .unwrap_or(8)
            .max(5);
        let pages_h: u32 = pages.iter().map(|(_, i)| i.height()).sum();
        let mut image = RgbaImage::new(total_w, pages_h + 8);
        for (file, page) in &pages {
            let oy = page_y[file];
            for y in 0..page.height() {
                for x in 0..page.width() {
                    image.put_pixel(x, oy + y, *page.get_pixel(x, y));
                }
            }
        }
        for y in 0..8u32 {
            for x in 0..5u32 {
                if x == 0 || x == 4 || y == 0 || y == 7 {
                    image.put_pixel(x, pages_h + y, image::Rgba([255, 255, 255, 255]));
                }
            }
        }
        let missing = Glyph {
            rect: [0, pages_h, 5, 8],
            draw_w: 5.0,
            draw_h: 8.0,
            y_off: 0.0,
            advance: 6.0,
        };

        const UNIHEX_COLS: u32 = 256;
        let (unihex_w, unihex_h) = if unihex_glyphs.is_empty() {
            (0, 0)
        } else {
            let rows = (unihex_glyphs.len() as u32).div_ceil(UNIHEX_COLS);
            (UNIHEX_COLS * 16, rows * 16)
        };
        let mut unihex_image = vec![0u8; unihex_w as usize * unihex_h as usize];
        let mut unihex_rects: HashMap<char, [u32; 4]> = HashMap::new();
        for (i, (c, width, raw)) in unihex_glyphs.iter().enumerate() {
            let cx = (i as u32 % UNIHEX_COLS) * 16;
            let cy = (i as u32 / UNIHEX_COLS) * 16;
            for ry in 0..16u32 {
                let row_bits: u32 = if *width == 8 {
                    (raw[ry as usize] as u32) << 8
                } else {
                    ((raw[ry as usize * 2] as u32) << 8) | raw[ry as usize * 2 + 1] as u32
                };
                for rx in 0..*width {
                    if row_bits & (0x8000 >> rx) != 0 {
                        unihex_image[((cy + ry) * unihex_w + (cx + rx)) as usize] = 255;
                    }
                }
            }
            unihex_rects.insert(*c, [cx, cy, *width, 16]);
        }

        let mut glyphs: HashMap<char, Glyph> = HashMap::new();
        for p in &providers {
            match p {
                Provider::Space(advances) => {
                    for (c, adv) in advances {
                        glyphs.entry(*c).or_insert(Glyph {
                            rect: [0, 0, 0, 0],
                            draw_w: 0.0,
                            draw_h: 0.0,
                            y_off: 0.0,
                            advance: *adv,
                        });
                    }
                }
                Provider::Bitmap {
                    file,
                    height,
                    ascent,
                    chars,
                } => {
                    let page = &pages.iter().find(|(f, _)| f == file).unwrap().1;
                    let oy = page_y[file];
                    let cols = chars[0].len() as u32;
                    let rows = chars.len() as u32;
                    let cell_w = page.width() / cols;
                    let cell_h = page.height() / rows;
                    let scale = *height as f32 / cell_h as f32;
                    for (ry, row) in chars.iter().enumerate() {
                        for (cx, c) in row.iter().enumerate() {
                            if *c == '\0' {
                                continue;
                            }
                            if glyphs.contains_key(c) {
                                continue;
                            }
                            let x0 = cx as u32 * cell_w;
                            let y0 = ry as u32 * cell_h;
                            let trimmed = trimmed_width(page, x0, y0, cell_w, cell_h);
                            let advance = (0.5 + trimmed as f64 * scale as f64) as i32 + 1;
                            glyphs.insert(
                                *c,
                                Glyph {
                                    rect: [x0, oy + y0, cell_w, cell_h],
                                    draw_w: cell_w as f32 * scale,
                                    draw_h: cell_h as f32 * scale,
                                    y_off: 7.0 - *ascent as f32,
                                    advance: advance as f32,
                                },
                            );
                        }
                    }
                }
                Provider::Unihex(_) => {}
            }
        }

        let mut unihex_glyphs_out: HashMap<char, Glyph> = HashMap::new();
        for (c, rect) in &unihex_rects {
            let [x0, y0, w, h] = *rect;
            let scale = 0.5;
            let trimmed = trimmed_width_packed(&unihex_image, unihex_w, x0, y0, w, h);
            let advance = (0.5 + trimmed as f64 * scale) as i32 + 1;
            unihex_glyphs_out.insert(
                *c,
                Glyph {
                    rect: [x0, y0, w, h],
                    draw_w: w as f32 * scale as f32,
                    draw_h: h as f32 * scale as f32,
                    y_off: 0.0,
                    advance: advance as f32,
                },
            );
        }

        let mut by_advance: HashMap<i32, Vec<char>> = HashMap::new();
        for (c, g) in &glyphs {
            if g.rect[2] == 0 || g.rect[3] == 0 {
                continue;
            }
            by_advance
                .entry(g.advance.ceil() as i32)
                .or_default()
                .push(*c);
        }
        for v in by_advance.values_mut() {
            v.sort_unstable();
        }

        let font = Font {
            ascii: ascii_table(&glyphs),
            glyphs,
            unihex_glyphs: unihex_glyphs_out,
            unihex_dims: (unihex_w, unihex_h),
            by_advance,
            missing,
            empty_chars: Vec::new(),
        };
        let pixels = FontPixels {
            pages: image,
            unihex: unihex_image,
        };
        (font, pixels)
    }

    pub fn glyph(&self, c: char) -> Option<&Glyph> {
        if let Some(Some(g)) = self.ascii.get(c as usize) {
            return Some(g);
        }
        match self.glyphs.get(&c).or_else(|| self.unihex_glyphs.get(&c)) {
            Some(g) => Some(g),
            None if (c as u32) < 0x20 || (0x7f..0xa0).contains(&(c as u32)) => None,
            None => Some(&self.missing),
        }
    }

    pub fn is_unihex(&self, c: char) -> bool {
        if let Some(Some(_)) = self.ascii.get(c as usize) {
            return false;
        }
        !self.glyphs.contains_key(&c) && self.unihex_glyphs.contains_key(&c)
    }

    pub fn missing_glyph(&self) -> Glyph {
        self.missing
    }

    pub fn advance(&self, c: char, bold: bool) -> f32 {
        let base = self.glyph(c).map(|g| g.advance).unwrap_or(0.0);
        if bold && base > 0.0 { base + 1.0 } else { base }
    }

    pub fn width(&self, spans: &[Span]) -> f32 {
        spans
            .iter()
            .map(|s| {
                s.text
                    .chars()
                    .map(|c| self.advance(c, s.style.bold))
                    .sum::<f32>()
            })
            .sum()
    }

    pub fn width_str(&self, s: &str) -> f32 {
        s.chars().map(|c| self.advance(c, false)).sum()
    }

    pub fn same_width_chars(&self, c: char) -> &[char] {
        match self.glyphs.get(&c) {
            Some(g) => self
                .by_advance
                .get(&(g.advance.ceil() as i32))
                .map(|v| v.as_slice())
                .unwrap_or(&self.empty_chars),
            None => &self.empty_chars,
        }
    }

    pub fn wrap(&self, spans: &[Span], max_width: f32) -> Vec<Vec<Span>> {
        let flat: Vec<(char, Style)> = spans
            .iter()
            .flat_map(|s| s.text.chars().map(move |c| (c, s.style)))
            .collect();

        let mut lines: Vec<Vec<(char, Style)>> = Vec::new();
        let mut line: Vec<(char, Style)> = Vec::new();
        let mut width = 0.0f32;
        let mut last_break: Option<usize> = None;
        let mut i = 0usize;
        while i < flat.len() {
            let (c, style) = flat[i];
            if c == '\n' {
                lines.push(std::mem::take(&mut line));
                width = 0.0;
                last_break = None;
                i += 1;
                continue;
            }
            if c == ' ' && line.is_empty() && !lines.is_empty() {
                i += 1;
                continue;
            }
            let adv = self.advance(c, style.bold);
            if !line.is_empty() && width + adv > max_width {
                if c == ' ' {
                    lines.push(std::mem::take(&mut line));
                    width = 0.0;
                    last_break = None;
                    i += 1;
                    continue;
                }
                match last_break {
                    Some(b) => {
                        let rest: Vec<(char, Style)> = line.split_off(b);
                        while line.last().map(|(c, _)| *c == ' ').unwrap_or(false) {
                            line.pop();
                        }
                        lines.push(std::mem::take(&mut line));
                        line = rest;
                        width = line.iter().map(|(c, s)| self.advance(*c, s.bold)).sum();
                    }
                    None => {
                        lines.push(std::mem::take(&mut line));
                        width = 0.0;
                    }
                }
                last_break = None;
                continue;
            }
            line.push((c, style));
            width += adv;
            if c == ' ' {
                last_break = Some(line.len());
            }
            i += 1;
        }
        lines.push(line);

        lines.into_iter().map(|l| merge_spans(&l)).collect()
    }

    pub fn wrap_ranges(&self, s: &str, max_width: f32, out: &mut Vec<Range<usize>>) {
        out.clear();
        let mut start = 0usize;
        let mut width = 0.0f32;
        let mut last_break: Option<usize> = None;
        let mut break_width = 0.0f32;
        let mut i = 0usize;
        let mut chars = s.chars().peekable();
        while let Some(c) = chars.peek().copied() {
            if c == '\n' {
                out.push(start..i);
                chars.next();
                i += 1;
                start = i;
                width = 0.0;
                last_break = None;
                continue;
            }
            let adv = self.advance(c, false);
            if i > start && width + adv > max_width {
                match last_break {
                    Some(b) => {
                        out.push(start..b);
                        start = b;
                        width -= break_width;
                    }
                    None => {
                        out.push(start..i);
                        start = i;
                        width = 0.0;
                    }
                }
                last_break = None;
                continue;
            }
            chars.next();
            width += adv;
            i += 1;
            if c == ' ' {
                last_break = Some(i);
                break_width = width;
            }
        }
        out.push(start..i);
    }

    pub fn trim_to_width(&self, spans: &[Span], max_width: f32) -> Vec<Span> {
        let mut out: Vec<(char, Style)> = Vec::new();
        let mut width = 0.0f32;
        'outer: for s in spans {
            for c in s.text.chars() {
                let adv = self.advance(c, s.style.bold);
                if width + adv > max_width {
                    break 'outer;
                }
                width += adv;
                out.push((c, s.style));
            }
        }
        merge_spans(&out)
    }
}

fn ascii_table(glyphs: &HashMap<char, Glyph>) -> [Option<Glyph>; 128] {
    std::array::from_fn(|i| glyphs.get(&(i as u8 as char)).copied())
}

fn merge_spans(chars: &[(char, Style)]) -> Vec<Span> {
    let mut out: Vec<Span> = Vec::new();
    for (c, style) in chars {
        match out.last_mut() {
            Some(last) if last.style == *style => last.text.push(*c),
            _ => out.push(Span {
                text: c.to_string(),
                style: *style,
            }),
        }
    }
    out
}

fn trimmed_width(page: &RgbaImage, x0: u32, y0: u32, cell_w: u32, cell_h: u32) -> u32 {
    for col in (0..cell_w).rev() {
        for y in 0..cell_h {
            if page.get_pixel(x0 + col, y0 + y)[3] != 0 {
                return col + 1;
            }
        }
    }
    0
}

fn trimmed_width_packed(
    buf: &[u8],
    stride: u32,
    x0: u32,
    y0: u32,
    cell_w: u32,
    cell_h: u32,
) -> u32 {
    for col in (0..cell_w).rev() {
        for y in 0..cell_h {
            if buf[((y0 + y) * stride + x0 + col) as usize] != 0 {
                return col + 1;
            }
        }
    }
    0
}

const BUILTIN: [(char, [u8; 8]); 66] = [
    (' ', [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]),
    ('%', [0x00, 0xC6, 0xCC, 0x18, 0x30, 0x66, 0xC6, 0x00]),
    ('.', [0x00, 0x00, 0x00, 0x00, 0x00, 0xC0, 0xC0, 0x00]),
    ('?', [0x78, 0xCC, 0x0C, 0x18, 0x30, 0x00, 0x30, 0x00]),
    ('0', [0x78, 0xCC, 0xDC, 0xEC, 0xCC, 0xCC, 0x78, 0x00]),
    ('1', [0x30, 0x70, 0x30, 0x30, 0x30, 0x30, 0xFC, 0x00]),
    ('2', [0x78, 0xCC, 0x18, 0x30, 0x60, 0xC0, 0xFC, 0x00]),
    ('3', [0x78, 0xCC, 0x0C, 0x38, 0x0C, 0xCC, 0x78, 0x00]),
    ('4', [0x1C, 0x3C, 0x6C, 0xCC, 0xFE, 0x0C, 0x0C, 0x00]),
    ('5', [0xFC, 0xC0, 0xF8, 0x0C, 0x0C, 0xCC, 0x78, 0x00]),
    ('6', [0x38, 0x60, 0xC0, 0xF8, 0xCC, 0xCC, 0x78, 0x00]),
    ('7', [0xFC, 0x0C, 0x0C, 0x18, 0x30, 0x30, 0x30, 0x00]),
    ('8', [0x78, 0xCC, 0xCC, 0x78, 0xCC, 0xCC, 0x78, 0x00]),
    ('9', [0x78, 0xCC, 0xCC, 0x7C, 0x0C, 0x18, 0x70, 0x00]),
    ('A', [0x30, 0x78, 0xCC, 0xCC, 0xFC, 0xCC, 0xCC, 0x00]),
    ('B', [0xF8, 0xCC, 0xCC, 0xF8, 0xCC, 0xCC, 0xF8, 0x00]),
    ('C', [0x78, 0xCC, 0xC0, 0xC0, 0xC0, 0xCC, 0x78, 0x00]),
    ('D', [0xF0, 0xD8, 0xCC, 0xCC, 0xCC, 0xD8, 0xF0, 0x00]),
    ('E', [0xFC, 0xC0, 0xC0, 0xF8, 0xC0, 0xC0, 0xFC, 0x00]),
    ('F', [0xFC, 0xC0, 0xC0, 0xF8, 0xC0, 0xC0, 0xC0, 0x00]),
    ('G', [0x78, 0xCC, 0xC0, 0xDC, 0xCC, 0xCC, 0x7C, 0x00]),
    ('H', [0xCC, 0xCC, 0xCC, 0xFC, 0xCC, 0xCC, 0xCC, 0x00]),
    ('I', [0xFC, 0x30, 0x30, 0x30, 0x30, 0x30, 0xFC, 0x00]),
    ('J', [0x0C, 0x0C, 0x0C, 0x0C, 0x0C, 0xCC, 0x78, 0x00]),
    ('K', [0xC6, 0xCC, 0xD8, 0xF0, 0xD8, 0xCC, 0xC6, 0x00]),
    ('L', [0xC0, 0xC0, 0xC0, 0xC0, 0xC0, 0xC0, 0xFC, 0x00]),
    ('M', [0xC6, 0xEE, 0xFE, 0xD6, 0xC6, 0xC6, 0xC6, 0x00]),
    ('N', [0xC6, 0xE6, 0xF6, 0xDE, 0xCE, 0xC6, 0xC6, 0x00]),
    ('O', [0x78, 0xCC, 0xCC, 0xCC, 0xCC, 0xCC, 0x78, 0x00]),
    ('P', [0xF8, 0xCC, 0xCC, 0xF8, 0xC0, 0xC0, 0xC0, 0x00]),
    ('Q', [0x78, 0xCC, 0xCC, 0xCC, 0xCC, 0xD8, 0x6C, 0x00]),
    ('R', [0xF8, 0xCC, 0xCC, 0xF8, 0xD8, 0xCC, 0xCC, 0x00]),
    ('S', [0x78, 0xCC, 0xC0, 0x78, 0x0C, 0xCC, 0x78, 0x00]),
    ('T', [0xFC, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x00]),
    ('U', [0xCC, 0xCC, 0xCC, 0xCC, 0xCC, 0xCC, 0x78, 0x00]),
    ('V', [0xCC, 0xCC, 0xCC, 0xCC, 0xCC, 0x78, 0x30, 0x00]),
    ('W', [0xC6, 0xC6, 0xC6, 0xD6, 0xFE, 0xEE, 0xC6, 0x00]),
    ('X', [0xC3, 0x66, 0x3C, 0x18, 0x3C, 0x66, 0xC3, 0x00]),
    ('Y', [0xC3, 0x66, 0x3C, 0x18, 0x18, 0x18, 0x18, 0x00]),
    ('Z', [0xFC, 0x0C, 0x18, 0x30, 0x60, 0xC0, 0xFC, 0x00]),
    ('a', [0x00, 0x00, 0x78, 0x0C, 0x7C, 0xCC, 0x7C, 0x00]),
    ('b', [0xC0, 0xC0, 0xF8, 0xCC, 0xCC, 0xCC, 0xF8, 0x00]),
    ('c', [0x00, 0x00, 0x78, 0xC0, 0xC0, 0xC0, 0x78, 0x00]),
    ('d', [0x0C, 0x0C, 0x7C, 0xCC, 0xCC, 0xCC, 0x7C, 0x00]),
    ('e', [0x00, 0x00, 0x78, 0xCC, 0xFC, 0xC0, 0x78, 0x00]),
    ('f', [0x38, 0x60, 0xF8, 0x60, 0x60, 0x60, 0x60, 0x00]),
    ('g', [0x00, 0x00, 0x7C, 0xCC, 0xCC, 0x7C, 0x0C, 0xF8]),
    ('h', [0xC0, 0xC0, 0xF8, 0xCC, 0xCC, 0xCC, 0xCC, 0x00]),
    ('i', [0x60, 0x00, 0xE0, 0x60, 0x60, 0x60, 0x78, 0x00]),
    ('j', [0x18, 0x00, 0x18, 0x18, 0x18, 0x18, 0x18, 0xF0]),
    ('k', [0xC0, 0xC0, 0xCC, 0xD8, 0xF0, 0xD8, 0xCC, 0x00]),
    ('l', [0xE0, 0x60, 0x60, 0x60, 0x60, 0x60, 0x78, 0x00]),
    ('m', [0x00, 0x00, 0xCC, 0xFE, 0xD6, 0xD6, 0xC6, 0x00]),
    ('n', [0x00, 0x00, 0xF8, 0xCC, 0xCC, 0xCC, 0xCC, 0x00]),
    ('o', [0x00, 0x00, 0x78, 0xCC, 0xCC, 0xCC, 0x78, 0x00]),
    ('p', [0x00, 0x00, 0xF8, 0xCC, 0xCC, 0xF8, 0xC0, 0xC0]),
    ('q', [0x00, 0x00, 0x7C, 0xCC, 0xCC, 0x7C, 0x0C, 0x0C]),
    ('r', [0x00, 0x00, 0xF8, 0xCC, 0xC0, 0xC0, 0xC0, 0x00]),
    ('s', [0x00, 0x00, 0x7C, 0xC0, 0x78, 0x0C, 0xF8, 0x00]),
    ('t', [0x60, 0x60, 0xFC, 0x60, 0x60, 0x60, 0x3C, 0x00]),
    ('u', [0x00, 0x00, 0xCC, 0xCC, 0xCC, 0xCC, 0x7C, 0x00]),
    ('v', [0x00, 0x00, 0xCC, 0xCC, 0xCC, 0x78, 0x30, 0x00]),
    ('w', [0x00, 0x00, 0xC6, 0xC6, 0xD6, 0x7C, 0x6C, 0x00]),
    ('x', [0x00, 0x00, 0xC6, 0x6C, 0x38, 0x6C, 0xC6, 0x00]),
    ('y', [0x00, 0x00, 0xCC, 0xCC, 0xCC, 0x7C, 0x0C, 0x78]),
    ('z', [0x00, 0x00, 0xFC, 0x18, 0x30, 0x60, 0xFC, 0x00]),
];

impl Font {
    pub fn builtin() -> (Font, FontPixels) {
        let cells = BUILTIN.len() as u32;
        let mut image = RgbaImage::new(cells * 8, 16);
        let white = image::Rgba([255, 255, 255, 255]);
        for (i, (_, rows)) in BUILTIN.iter().enumerate() {
            let x0 = i as u32 * 8;
            for (y, row) in rows.iter().enumerate() {
                for x in 0..8u32 {
                    if row & (0x80u8 >> x) != 0 {
                        image.put_pixel(x0 + x, y as u32, white);
                    }
                }
            }
        }
        for y in 0..8u32 {
            for x in 0..5u32 {
                if x == 0 || x == 4 || y == 0 || y == 7 {
                    image.put_pixel(x, 8 + y, white);
                }
            }
        }
        let missing = Glyph {
            rect: [0, 8, 5, 8],
            draw_w: 5.0,
            draw_h: 8.0,
            y_off: 0.0,
            advance: 6.0,
        };

        let mut glyphs: HashMap<char, Glyph> = HashMap::new();
        for (i, (c, _)) in BUILTIN.iter().enumerate() {
            let x0 = i as u32 * 8;
            let trimmed = trimmed_width(&image, x0, 0, 8, 8);
            glyphs.insert(
                *c,
                if trimmed == 0 {
                    Glyph {
                        rect: [0, 0, 0, 0],
                        draw_w: 0.0,
                        draw_h: 0.0,
                        y_off: 0.0,
                        advance: 4.0,
                    }
                } else {
                    Glyph {
                        rect: [x0, 0, 8, 8],
                        draw_w: 8.0,
                        draw_h: 8.0,
                        y_off: 0.0,
                        advance: trimmed as f32 + 1.0,
                    }
                },
            );
        }

        let mut by_advance: HashMap<i32, Vec<char>> = HashMap::new();
        for (c, g) in &glyphs {
            if g.rect[2] == 0 || g.rect[3] == 0 {
                continue;
            }
            by_advance
                .entry(g.advance.ceil() as i32)
                .or_default()
                .push(*c);
        }
        for v in by_advance.values_mut() {
            v.sort_unstable();
        }

        let font = Font {
            ascii: ascii_table(&glyphs),
            glyphs,
            unihex_glyphs: HashMap::new(),
            unihex_dims: (0, 0),
            by_advance,
            missing,
            empty_chars: Vec::new(),
        };
        let pixels = FontPixels {
            pages: image,
            unihex: Vec::new(),
        };
        (font, pixels)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_style_still_fits_in_twelve_bytes() {
        assert_eq!(std::mem::size_of::<Style>(), 12);
    }

    #[test]
    fn a_legacy_code_keeps_the_components_event() {
        let style = Style {
            event: 3,
            ..Style::default()
        };
        let spans = parse_formatted_from("a\u{a7}cb\u{a7}rc", style);
        assert_eq!(spans.len(), 3);
        assert!(spans.iter().all(|s| s.style.event == 3));
        assert_eq!(spans[1].style.color, 0xFF5555);
        assert_eq!(spans[2].style.color, Style::default().color);
    }

    fn assets() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("reference/minecraft-26.1.1/assets/minecraft")
    }

    fn font() -> Font {
        Font::load(&assets()).0
    }

    #[test]
    fn ascii_glyph_metrics() {
        let f = font();
        for c in 0x20u8..=0x7e {
            let c = c as char;
            assert!(f.glyphs.contains_key(&c), "missing glyph for {c:?}");
        }
        for c in "AZaz09".chars() {
            let g = f.glyph(c).unwrap();
            assert_eq!((g.draw_w, g.draw_h, g.y_off), (8.0, 8.0, 0.0), "{c:?}");
            assert_eq!(
                g.rect[3], 8,
                "{c:?} should come from ascii.png, not accented.png"
            );
        }
        let a = f.glyph('\u{c0}').unwrap();
        assert_eq!((a.draw_w, a.draw_h, a.y_off), (9.0, 12.0, -3.0));
        let alpha = f.glyph('\u{391}').unwrap();
        assert_eq!((alpha.draw_w, alpha.draw_h, alpha.y_off), (8.0, 8.0, 0.0));
    }

    #[test]
    fn advances_match_hand_counted_pixels() {
        let f = font();
        for (c, adv) in [
            ('H', 6.0),
            ('e', 6.0),
            ('l', 3.0),
            ('o', 6.0),
            ('i', 2.0),
            ('!', 2.0),
            ('W', 6.0),
        ] {
            assert_eq!(f.advance(c, false), adv, "{c:?}");
        }
        assert_eq!(f.advance(' ', false), 4.0);
        assert_eq!(f.advance('\u{200c}', false), 0.0);

        assert_eq!(f.width_str("Hello"), 24.0);
        assert_eq!(f.width_str("Hello, World!"), 59.0);
        assert_eq!(f.width_str("iiii"), 8.0);
        assert_eq!(f.width_str("WWW"), 18.0);
        assert_eq!(f.width_str("0123456789"), 60.0);
        assert_eq!(f.width_str("abcdefghijklmnopqrstuvwxyz"), 145.0);
    }

    #[test]
    fn bold_adds_one_per_char() {
        let f = font();
        let plain = styled("Hello, World!", Style::default());
        let bold = styled(
            "Hello, World!",
            Style {
                bold: true,
                ..Style::default()
            },
        );
        assert_eq!(f.width(&bold) - f.width(&plain), 13.0);
    }

    #[test]
    fn parse_legacy_codes() {
        let spans = parse_formatted("\u{a7}cRed\u{a7}lBold");
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].text, "Red");
        assert_eq!(spans[0].style.color, 0xFF5555);
        assert!(!spans[0].style.bold);
        assert_eq!(spans[1].text, "Bold");
        assert_eq!(spans[1].style.color, 0xFF5555);
        assert!(spans[1].style.bold);

        let spans = parse_formatted("\u{a7}la\u{a7}9b");
        assert!(spans[0].style.bold);
        assert!(!spans[1].style.bold);
        assert_eq!(spans[1].style.color, 0x5555FF);

        let spans = parse_formatted("\u{a7}c\u{a7}lx\u{a7}ry\u{a7}zz");
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[1].text, "yz");
        assert_eq!(spans[1].style, Style::default());

        assert_eq!(named_color("red"), Some(0xFF5555));
        assert_eq!(named_color("dark_blue"), Some(0x0000AA));
        assert_eq!(named_color("nope"), None);
        assert_eq!(Style::shadow_color(0xFF5555), 0x3F1515);
    }

    #[test]
    fn wrapping() {
        let f = font();
        let text = styled("aaa bbb ccc", Style::default());
        let w = f.width_str("aaa bbb");
        let lines = f.wrap(&text, w);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0][0].text, "aaa bbb");
        assert_eq!(lines[1][0].text, "ccc");

        let long = styled("aaaaaaaaaa", Style::default());
        let lines = f.wrap(&long, 12.0);
        assert!(lines.len() > 1);
        for l in &lines {
            assert!(f.width(l) <= 12.0);
        }

        let spans = parse_formatted("\u{a7}caaa \u{a7}9bbb");
        let lines = f.wrap(&spans, f.width_str("aaa"));
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0][0].style.color, 0xFF5555);
        assert_eq!(lines[1][0].style.color, 0x5555FF);
        assert_eq!(lines[1][0].text, "bbb");

        let trimmed = f.trim_to_width(&styled("Hello", Style::default()), 12.0);
        assert_eq!(trimmed[0].text, "He");
    }

    #[test]
    fn obfuscation_groups() {
        let f = font();
        let group = f.same_width_chars('a');
        assert!(group.contains(&'a'));
        assert!(
            group
                .iter()
                .all(|c| f.advance(*c, false) == f.advance('a', false))
        );
        assert!(!f.same_width_chars('i').contains(&'a'));
        assert!(f.same_width_chars('\u{fffff}').is_empty());
    }

    #[test]
    fn wrap_ranges_tile_the_string() {
        let f = font();
        let mut out = Vec::new();
        for text in [
            "",
            "short",
            "the quick brown fox jumps over the lazy dog again and again",
            "line one\nline two",
            "supercalifragilisticexpialidocious_and_then_some_more_letters",
            "trailing space   ",
        ] {
            f.wrap_ranges(text, 60.0, &mut out);
            assert!(!out.is_empty(), "{text:?} produced no lines");
            assert_eq!(out[0].start, 0);
            assert_eq!(out.last().unwrap().end, text.chars().count(), "{text:?}");
            for pair in out.windows(2) {
                assert!(pair[0].end <= pair[1].start, "{text:?} went backwards");
                let gap = pair[1].start - pair[0].end;
                assert!(gap <= 1, "{text:?} left a gap of {gap}");
            }
        }
    }

    #[test]
    fn wrap_ranges_breaks_on_newlines() {
        let f = font();
        let mut out = Vec::new();
        f.wrap_ranges("a\nb\n\nc", 200.0, &mut out);
        assert_eq!(out.len(), 4);
        assert_eq!(out[2], 4..4, "the empty line between the two breaks");
    }

    #[test]
    fn dump_atlas_stats() {
        let (f, pixels) = Font::load(&assets());
        println!(
            "font image {}x{}, {} glyphs, {} advance groups",
            pixels.pages.width(),
            pixels.pages.height(),
            f.glyphs.len(),
            f.by_advance.len()
        );
        assert!(pixels.pages.width() > 0 && pixels.pages.height() > 0);
    }
}
