use crate::gui::atlas::{GuiAtlas, Region, Scaling};
use crate::gui::gui_material::UNIHEX_UV_BIAS;
use crate::text::{Glyph, Span, Style};
use crate::util::mth::srgb_to_linear;

pub const ITEM_SIZE: f32 = 16.0;
#[allow(dead_code, reason = "the slot geometry")]
pub const SLOT_SIZE: f32 = 18.0;

pub const CORNER_TL: u8 = 1;
pub const CORNER_TR: u8 = 2;
#[allow(
    dead_code,
    reason = "the mask is the whole set; only two are named yet"
)]
pub const CORNER_BR: u8 = 4;
#[allow(
    dead_code,
    reason = "the mask is the whole set; only two are named yet"
)]
pub const CORNER_BL: u8 = 8;
pub const CORNERS_TOP: u8 = CORNER_TL | CORNER_TR;
#[allow(
    dead_code,
    reason = "the mask is the whole set; only two are named yet"
)]
pub const CORNERS_BOTTOM: u8 = CORNER_BR | CORNER_BL;
pub const CORNERS_ALL: u8 = 0xF;

const CORNER_SEGMENTS: usize = 5;

const CORNER_FEATHER: f32 = 0.5;

const ARC: [[f32; 2]; CORNER_SEGMENTS + 1] = [
    [-1.0, 0.0],
    [-0.951_056_5, -0.309_017],
    [-0.809_017, -0.587_785_25],
    [-0.587_785_25, -0.809_017],
    [-0.309_017, -0.951_056_5],
    [0.0, -1.0],
];

fn rotate_quadrant(p: [f32; 2], q: usize) -> [f32; 2] {
    let mut p = p;
    for _ in 0..q % 4 {
        p = [-p[1], p[0]];
    }
    p
}

pub struct Digits {
    buf: [u8; 10],
    start: usize,
}

impl Digits {
    pub fn new(mut n: u32) -> Digits {
        let mut d = Digits {
            buf: [b'0'; 10],
            start: 10,
        };
        loop {
            d.start -= 1;
            d.buf[d.start] = b'0' + (n % 10) as u8;
            n /= 10;
            if n == 0 || d.start == 0 {
                break;
            }
        }
        d
    }

    pub fn as_str(&self) -> &str {
        std::str::from_utf8(&self.buf[self.start..]).unwrap_or("0")
    }
}

pub struct Painter<'a> {
    pub atlas: &'a GuiAtlas,
    pub positions: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub colors: Vec<[f32; 4]>,
    pub indices: Vec<u32>,
    pub frame: u32,
    pub scale: f32,
    pub unihex_enabled: bool,
    rng: u32,
    clip: Option<[f32; 4]>,
    xform: (f32, f32, f32),
}

#[derive(Clone, Copy)]
pub struct ClipGuard(Option<[f32; 4]>);

#[derive(Default)]
pub struct PainterBuffers {
    pub positions: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub colors: Vec<[f32; 4]>,
    pub indices: Vec<u32>,
}

impl<'a> Painter<'a> {
    pub fn new(atlas: &'a GuiAtlas, frame: u32) -> Self {
        Self::with_buffers(atlas, frame, PainterBuffers::default())
    }

    pub fn with_buffers(atlas: &'a GuiAtlas, frame: u32, mut bufs: PainterBuffers) -> Self {
        bufs.positions.clear();
        bufs.uvs.clear();
        bufs.colors.clear();
        bufs.indices.clear();
        Painter {
            atlas,
            positions: bufs.positions,
            uvs: bufs.uvs,
            colors: bufs.colors,
            indices: bufs.indices,
            frame,
            scale: 4.0,
            unihex_enabled: true,
            rng: frame.wrapping_mul(0x9E37_79B9) | 1,
            clip: None,
            xform: (1.0, 0.0, 0.0),
        }
    }

    pub fn into_buffers(self) -> PainterBuffers {
        PainterBuffers {
            positions: self.positions,
            uvs: self.uvs,
            colors: self.colors,
            indices: self.indices,
        }
    }

    pub fn push_clip(&mut self, x: f32, y: f32, w: f32, h: f32) -> ClipGuard {
        let saved = ClipGuard(self.clip);
        let mut rect = [x, y, x + w, y + h];
        if let Some(outer) = self.clip {
            rect = [
                rect[0].max(outer[0]),
                rect[1].max(outer[1]),
                rect[2].min(outer[2]),
                rect[3].min(outer[3]),
            ];
        }
        self.clip = Some(rect);
        saved
    }

    pub fn pop_clip(&mut self, saved: ClipGuard) {
        self.clip = saved.0;
    }

    fn place(&self, x: f32, y: f32) -> [f32; 3] {
        let (s, ox, oy) = self.xform;
        [x * s + ox, y * s + oy, 0.0]
    }

    pub fn scaled<R>(&mut self, scale: f32, x: f32, y: f32, f: impl FnOnce(&mut Self) -> R) -> R {
        let saved_xform = self.xform;
        let saved_clip = self.clip;
        let scale = scale.max(0.0);
        if scale > 0.0 {
            self.clip = self.clip.map(|[x0, y0, x1, y1]| {
                [
                    (x0 - x) / scale,
                    (y0 - y) / scale,
                    (x1 - x) / scale,
                    (y1 - y) / scale,
                ]
            });
        }
        self.xform = (scale, x, y);
        let out = f(self);
        self.xform = saved_xform;
        self.clip = saved_clip;
        out
    }

    fn next_rand(&mut self) -> u32 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        x
    }

    fn quad(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        u0: f32,
        v0: f32,
        u1: f32,
        v1: f32,
        color: [f32; 4],
        shear: f32,
    ) {
        self.quad_from(x, y, w, h, u0, v0, u1, v1, color, shear, false);
    }

    fn quad_unihex(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        u0: f32,
        v0: f32,
        u1: f32,
        v1: f32,
        color: [f32; 4],
        shear: f32,
    ) {
        self.quad_from(x, y, w, h, u0, v0, u1, v1, color, shear, true);
    }

    #[allow(clippy::too_many_arguments, reason = "one quad's full geometry")]
    fn quad_from(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        u0: f32,
        v0: f32,
        u1: f32,
        v1: f32,
        color: [f32; 4],
        shear: f32,
        unihex: bool,
    ) {
        if w <= 0.0 || h <= 0.0 || color[3] <= 0.0 {
            return;
        }
        let (mut x, mut y, mut w, mut h) = (x, y, w, h);
        let (mut u0, mut v0, mut u1, mut v1) = (u0, v0, u1, v1);
        if let Some([cx0, cy0, cx1, cy1]) = self.clip {
            let (x0, y0, x1, y1) = (x.max(cx0), y.max(cy0), (x + w).min(cx1), (y + h).min(cy1));
            if x1 <= x0 || y1 <= y0 {
                return;
            }
            let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;
            let (nu0, nu1) = (lerp(u0, u1, (x0 - x) / w), lerp(u0, u1, (x1 - x) / w));
            let (nv0, nv1) = (lerp(v0, v1, (y0 - y) / h), lerp(v0, v1, (y1 - y) / h));
            (u0, u1, v0, v1) = (nu0, nu1, nv0, nv1);
            (x, y, w, h) = (x0, y0, x1 - x0, y1 - y0);
        }
        let base = self.positions.len() as u32;
        let bias = if unihex { UNIHEX_UV_BIAS } else { 0.0 };
        let ([mut au0, av0], [mut au1, av1]) = if unihex {
            (self.atlas.unihex_uv(u0, v0), self.atlas.unihex_uv(u1, v1))
        } else {
            (self.atlas.uv(u0, v0), self.atlas.uv(u1, v1))
        };
        au0 += bias;
        au1 += bias;
        let corners = [
            self.place(x + shear, y),
            self.place(x + w + shear, y),
            self.place(x + w, y + h),
            self.place(x, y + h),
        ];
        self.positions.extend_from_slice(&corners);
        self.uvs.push([au0, av0]);
        self.uvs.push([au1, av0]);
        self.uvs.push([au1, av1]);
        self.uvs.push([au0, av1]);
        for _ in 0..4 {
            self.colors.push(color);
        }
        self.indices
            .extend_from_slice(&[base, base + 2, base + 1, base, base + 3, base + 2]);
    }

    pub fn fill(&mut self, x: f32, y: f32, w: f32, h: f32, argb: u32) {
        let r = self.atlas.white;
        let c = unpack(argb);
        let (u, v) = (r.x as f32 + 0.5, r.y as f32 + 0.5);
        self.quad(x, y, w, h, u, v, u, v, c, 0.0);
    }

    pub fn gradient_v(&mut self, x: f32, y: f32, w: f32, h: f32, top: u32, bottom: u32) {
        let r = self.atlas.white;
        let (u, v) = (r.x as f32 + 0.5, r.y as f32 + 0.5);
        let base = self.positions.len() as u32;
        let uv = self.atlas.uv(u, v);
        let corners = [
            self.place(x, y),
            self.place(x + w, y),
            self.place(x + w, y + h),
            self.place(x, y + h),
        ];
        self.positions.extend_from_slice(&corners);
        for _ in 0..4 {
            self.uvs.push(uv);
        }
        let (t, b) = (unpack(top), unpack(bottom));
        self.colors.push(t);
        self.colors.push(t);
        self.colors.push(b);
        self.colors.push(b);
        self.indices
            .extend_from_slice(&[base, base + 2, base + 1, base, base + 3, base + 2]);
    }

    fn region_part(
        &mut self,
        r: Region,
        sx: f32,
        sy: f32,
        sw: f32,
        sh: f32,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        color: [f32; 4],
    ) {
        let u0 = r.x as f32 + sx;
        let v0 = r.y as f32 + sy;
        self.quad(x, y, w, h, u0, v0, u0 + sw, v0 + sh, color, 0.0);
    }

    pub fn blit_sheet(&mut self, sheet: &str, sx: f32, sy: f32, w: f32, h: f32, x: f32, y: f32) {
        let Some(r) = self.atlas.sheet(sheet) else {
            return;
        };
        self.region_part(r, sx, sy, w, h, x, y, w, h, [1.0; 4]);
    }

    pub fn sprite_part(&mut self, name: &str, sx: f32, sy: f32, w: f32, h: f32, x: f32, y: f32) {
        let Some(sp) = self.atlas.sprite(name).copied() else {
            self.fill(x, y, w, h, 0xFFFF_00FF);
            return;
        };
        self.region_part(sp.region, sx, sy, w, h, x, y, w, h, [1.0; 4]);
    }

    pub fn sprite(&mut self, name: &str, x: f32, y: f32, w: f32, h: f32) {
        self.sprite_tinted(name, x, y, w, h, 0xFFFF_FFFF);
    }

    pub fn sprite_tinted(&mut self, name: &str, x: f32, y: f32, w: f32, h: f32, argb: u32) {
        let Some(sp) = self.atlas.sprite(name).copied() else {
            self.fill(x, y, w, h, 0xFFFF_00FF);
            return;
        };
        let color = unpack(argb);
        match sp.scaling {
            Scaling::Stretch => {
                self.region_part(
                    sp.region,
                    0.0,
                    0.0,
                    sp.region.w as f32,
                    sp.region.h as f32,
                    x,
                    y,
                    w,
                    h,
                    color,
                );
            }
            Scaling::Tile { w: tw, h: th } => {
                self.tile(sp.region, 0.0, 0.0, tw as f32, th as f32, x, y, w, h, color);
            }
            Scaling::NineSlice {
                w: sw,
                h: sh,
                border,
                stretch_inner,
            } => {
                self.nine_slice(
                    sp.region,
                    sw as f32,
                    sh as f32,
                    border,
                    stretch_inner,
                    x,
                    y,
                    w,
                    h,
                    color,
                );
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn tile(
        &mut self,
        r: Region,
        sx: f32,
        sy: f32,
        tw: f32,
        th: f32,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        color: [f32; 4],
    ) {
        if tw <= 0.0 || th <= 0.0 {
            return;
        }
        let mut dy = 0.0;
        while dy < h {
            let ph = th.min(h - dy);
            let mut dx = 0.0;
            while dx < w {
                let pw = tw.min(w - dx);
                self.region_part(r, sx, sy, pw, ph, x + dx, y + dy, pw, ph, color);
                dx += tw;
            }
            dy += th;
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn nine_slice(
        &mut self,
        r: Region,
        sw: f32,
        sh: f32,
        border: [u32; 4],
        stretch_inner: bool,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        color: [f32; 4],
    ) {
        let (l, t, rt, b) = (
            border[0] as f32,
            border[1] as f32,
            border[2] as f32,
            border[3] as f32,
        );
        let (l, rt) = if l + rt > w {
            (w * l / (l + rt), w * rt / (l + rt))
        } else {
            (l, rt)
        };
        let (t, b) = if t + b > h {
            (h * t / (t + b), h * b / (t + b))
        } else {
            (t, b)
        };
        let (mid_sw, mid_sh) = (sw - l - rt, sh - t - b);
        let (mid_w, mid_h) = (w - l - rt, h - t - b);

        self.region_part(r, 0.0, 0.0, l, t, x, y, l, t, color);
        self.region_part(r, sw - rt, 0.0, rt, t, x + w - rt, y, rt, t, color);
        self.region_part(r, 0.0, sh - b, l, b, x, y + h - b, l, b, color);
        self.region_part(
            r,
            sw - rt,
            sh - b,
            rt,
            b,
            x + w - rt,
            y + h - b,
            rt,
            b,
            color,
        );

        if stretch_inner {
            self.region_part(r, l, 0.0, mid_sw, t, x + l, y, mid_w, t, color);
            self.region_part(r, l, sh - b, mid_sw, b, x + l, y + h - b, mid_w, b, color);
            self.region_part(r, 0.0, t, l, mid_sh, x, y + t, l, mid_h, color);
            self.region_part(
                r,
                sw - rt,
                t,
                rt,
                mid_sh,
                x + w - rt,
                y + t,
                rt,
                mid_h,
                color,
            );
            self.region_part(r, l, t, mid_sw, mid_sh, x + l, y + t, mid_w, mid_h, color);
        } else {
            self.tile(r, l, 0.0, mid_sw, t, x + l, y, mid_w, t, color);
            self.tile(r, l, sh - b, mid_sw, b, x + l, y + h - b, mid_w, b, color);
            self.tile(r, 0.0, t, l, mid_sh, x, y + t, l, mid_h, color);
            self.tile(
                r,
                sw - rt,
                t,
                rt,
                mid_sh,
                x + w - rt,
                y + t,
                rt,
                mid_h,
                color,
            );
            self.tile(r, l, t, mid_sw, mid_sh, x + l, y + t, mid_w, mid_h, color);
        }
    }

    pub fn text(&mut self, spans: &[Span], x: f32, y: f32, shadow: bool) -> f32 {
        self.text_faded(spans, x, y, shadow, 1.0)
    }

    pub fn text_faded(&mut self, spans: &[Span], x: f32, y: f32, shadow: bool, alpha: f32) -> f32 {
        if alpha <= 0.0 {
            return x;
        }
        if shadow {
            self.draw_spans(spans, x + 1.0, y + 1.0, true, alpha);
        }
        self.draw_spans(spans, x, y, false, alpha)
    }

    pub fn text_sga(&mut self, s: &str, x: f32, y: f32, argb_rgb: u32) -> f32 {
        use crate::gui::atlas::{SGA_CELL, SGA_SPRITE, sga_advance};

        let Some(region) = self.atlas.sprite(SGA_SPRITE).map(|s| s.region) else {
            return x;
        };
        let color = unpack(0xFF00_0000 | argb_rgb);
        let mut pen = x;
        for c in s.chars() {
            let advance = sga_advance(c);
            if advance == 0.0 {
                continue;
            }
            let code = c as u32;
            if code < 128 && c != ' ' {
                let (cx, cy) = ((code % 16) as f32 * SGA_CELL, (code / 16) as f32 * SGA_CELL);
                self.region_part(
                    region, cx, cy, SGA_CELL, SGA_CELL, pen, y, SGA_CELL, SGA_CELL, color,
                );
            }
            pen += advance;
        }
        pen
    }

    pub fn sga_width(&self, s: &str) -> f32 {
        use crate::gui::atlas::sga_advance;
        s.chars().map(sga_advance).sum()
    }

    pub fn text_str(&mut self, s: &str, x: f32, y: f32, argb_rgb: u32, shadow: bool) -> f32 {
        let spans = [Span {
            text: s.to_string(),
            style: Style::colored(argb_rgb),
        }];
        self.text(&spans, x, y, shadow)
    }

    fn draw_spans(&mut self, spans: &[Span], x: f32, y: f32, as_shadow: bool, alpha: f32) -> f32 {
        let a = (alpha.clamp(0.0, 1.0) * 255.0).round() as u32;
        let mut pen = x;
        for span in spans {
            let st = span.style;
            let rgb = if as_shadow {
                Style::shadow_color(st.color)
            } else {
                st.color
            };
            let color = unpack((a << 24) | rgb);
            let start = pen;
            for ch in span.text.chars() {
                let ch = if st.obfuscated { self.shuffle(ch) } else { ch };
                let Some(g) = self.atlas.font.glyph(ch).copied() else {
                    pen += self.atlas.font.advance(ch, st.bold);
                    continue;
                };
                self.glyph(ch, &g, pen, y, color, st.italic);
                if st.bold {
                    self.glyph(ch, &g, pen + 1.0, y, color, st.italic);
                }
                pen += g.advance + if st.bold { 1.0 } else { 0.0 };
            }
            let x0 = if start == x { start - 1.0 } else { start };
            if st.strikethrough {
                self.fill(x0, y + 3.5, pen - x0, 1.0, (a << 24) | rgb);
            }
            if st.underline {
                self.fill(x0, y + 8.0, pen - x0, 1.0, (a << 24) | rgb);
            }
        }
        pen
    }

    fn glyph(&mut self, ch: char, g: &Glyph, x: f32, y: f32, color: [f32; 4], italic: bool) {
        if g.rect[2] == 0 || g.rect[3] == 0 {
            return;
        }
        let shear = if italic { 1.0 } else { 0.0 };
        if self.atlas.font.is_unihex(ch) && self.unihex_enabled {
            let [x0, y0, w, h] = g.rect;
            self.quad_unihex(
                x,
                y + g.y_off,
                g.draw_w,
                g.draw_h,
                x0 as f32,
                y0 as f32,
                (x0 + w) as f32,
                (y0 + h) as f32,
                color,
                shear,
            );
            return;
        }
        let g = if self.atlas.font.is_unihex(ch) {
            self.atlas.font.missing_glyph()
        } else {
            *g
        };
        let r = self.atlas.glyph_region(g.rect);
        let (u0, v0) = (r.x as f32, r.y as f32);
        self.quad(
            x,
            y + g.y_off,
            g.draw_w,
            g.draw_h,
            u0,
            v0,
            u0 + r.w as f32,
            v0 + r.h as f32,
            color,
            shear,
        );
    }

    fn shuffle(&mut self, ch: char) -> char {
        let pool = self.atlas.font.same_width_chars(ch);
        if pool.is_empty() {
            return ch;
        }
        let i = (self.next_rand() as usize) % pool.len();
        pool[i]
    }

    pub fn item_icon(&mut self, id: &str, x: f32, y: f32) {
        let target_px = ITEM_SIZE * self.scale;
        let Some(r) = self.atlas.item_for_size(id, target_px) else {
            if let Some((item, color)) = crate::items::potions::parse_tint_key(id) {
                self.potion_layers(item, color, x, y);
                return;
            }
            if crate::blockentities::banner::is_model_key(id) {
                self.banner_icon(id, x, y);
                return;
            }
            self.fill(x, y, ITEM_SIZE, ITEM_SIZE, 0xFFFF_00FF);
            return;
        };
        self.region_part(
            r, 0.0, 0.0, r.w as f32, r.h as f32, x, y, ITEM_SIZE, ITEM_SIZE, [1.0; 4],
        );
    }

    fn potion_layers(&mut self, item: &str, color: u32, x: f32, y: f32) {
        use crate::items::potions::layer_key;

        let target_px = ITEM_SIZE * self.scale;
        let mut drew = false;
        for layer in 0..2 {
            let Some(r) = self.atlas.item_for_size(&layer_key(item, layer), target_px) else {
                continue;
            };
            let tint = if layer == 0 {
                unpack(0xFF00_0000 | color)
            } else {
                [1.0; 4]
            };
            self.region_part(
                r, 0.0, 0.0, r.w as f32, r.h as f32, x, y, ITEM_SIZE, ITEM_SIZE, tint,
            );
            drew = true;
        }
        if !drew {
            self.fill(x, y, ITEM_SIZE, ITEM_SIZE, 0xFFFF_00FF);
        }
    }

    fn banner_icon(&mut self, key: &str, x: f32, y: f32) {
        use crate::gui::banner_icons;

        let target_px = ITEM_SIZE * self.scale;
        let edge = crate::items::icons::model_tier(target_px);
        banner_icons::request(key, edge);

        if let Some(slot) = banner_icons::slot(key)
            && let Some(r) = self.atlas.banner_icon(slot)
        {
            let r = Region {
                w: edge,
                h: edge,
                ..r
            };
            self.region_part(
                r,
                0.0,
                0.0,
                edge as f32,
                edge as f32,
                x,
                y,
                ITEM_SIZE,
                ITEM_SIZE,
                [1.0; 4],
            );
            return;
        }

        let plain = crate::blockentities::banner::key_item(key);
        let Some(r) = self.atlas.item_for_size(plain, target_px) else {
            self.fill(x, y, ITEM_SIZE, ITEM_SIZE, 0xFFFF_00FF);
            return;
        };
        self.region_part(
            r, 0.0, 0.0, r.w as f32, r.h as f32, x, y, ITEM_SIZE, ITEM_SIZE, [1.0; 4],
        );
    }

    pub fn item_decorations(&mut self, x: f32, y: f32, count: u8, damage: u16, max_damage: u16) {
        self.item_decorations_colored(x, y, count, damage, max_damage, 0xFFFFFF);
    }

    pub fn item_decorations_colored(
        &mut self,
        x: f32,
        y: f32,
        count: u8,
        damage: u16,
        max_damage: u16,
        count_color: u32,
    ) {
        if max_damage > 0 && damage > 0 {
            let frac = 1.0 - (damage as f32 / max_damage as f32).clamp(0.0, 1.0);
            let bar = (frac * 13.0).round().max(0.0);
            let hue = frac / 3.0;
            let rgb = hsv_to_rgb(hue, 1.0, 1.0);
            self.fill(x + 2.0, y + 13.0, 13.0, 2.0, 0xFF00_0000);
            self.fill(x + 2.0, y + 13.0, bar, 1.0, 0xFF00_0000 | rgb);
        }
        if count > 1 {
            let digits = Digits::new(count as u32);
            let s = digits.as_str();
            let w = self.atlas.font.width_str(s);
            self.text_str(s, x + 17.0 - w, y + 9.0, count_color, true);
        }
    }

    pub fn item_cooldown(&mut self, x: f32, y: f32, frac: u8) {
        if frac == 0 {
            return;
        }
        let p = frac as f32 / 255.0;
        let top = (16.0 * (1.0 - p)).floor();
        let height = (16.0 * p).ceil();
        self.fill(x, y + top, 16.0, height, 0x7FFF_FFFF);
    }

    pub fn skin_face(
        &mut self,
        face: &str,
        hat: Option<&str>,
        x: f32,
        y: f32,
        size: f32,
        flip: bool,
    ) {
        self.sprite_flipped(face, x, y, size, size, flip);
        if let Some(hat) = hat {
            self.sprite_flipped(hat, x, y, size, size, flip);
        }
    }

    fn sprite_flipped(&mut self, name: &str, x: f32, y: f32, w: f32, h: f32, flip_v: bool) {
        let Some(sp) = self.atlas.sprite(name).copied() else {
            self.fill(x, y, w, h, 0xFFFF_00FF);
            return;
        };
        let r = sp.region;
        let (u0, u1) = (r.x as f32, (r.x + r.w) as f32);
        let (v0, v1) = (r.y as f32, (r.y + r.h) as f32);
        let (v0, v1) = if flip_v { (v1, v0) } else { (v0, v1) };
        self.quad(x, y, w, h, u0, v0, u1, v1, [1.0; 4], 0.0);
    }

    #[cfg(feature = "skins")]
    pub fn atlas_region_flipped(
        &mut self,
        r: Region,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        flip_v: bool,
    ) {
        let (u0, u1) = (r.x as f32, (r.x + r.w) as f32);
        let (v0, v1) = (r.y as f32, (r.y + r.h) as f32);
        let (v0, v1) = if flip_v { (v1, v0) } else { (v0, v1) };
        self.quad(x, y, w, h, u0, v0, u1, v1, [1.0; 4], 0.0);
    }

    pub fn region(&mut self, r: Region, x: f32, y: f32, w: f32, h: f32) {
        self.region_part(r, 0.0, 0.0, r.w as f32, r.h as f32, x, y, w, h, [1.0; 4]);
    }

    #[cfg(test)]
    pub fn blit_region_debug(&mut self, r: Region, x: f32, y: f32, w: f32, h: f32) {
        self.region(r, x, y, w, h);
    }

    pub fn slot_highlight_back(&mut self, x: f32, y: f32) {
        self.sprite(
            "container/slot_highlight_back",
            x - 4.0,
            y - 4.0,
            24.0,
            24.0,
        );
    }

    pub fn slot_highlight_front(&mut self, x: f32, y: f32) {
        self.sprite(
            "container/slot_highlight_front",
            x - 4.0,
            y - 4.0,
            24.0,
            24.0,
        );
    }

    pub fn slot_highlight(&mut self, x: f32, y: f32) {
        self.slot_highlight_back(x, y);
        self.slot_highlight_front(x, y);
    }

    pub fn rounded_rect(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, argb: u32) {
        self.rounded_rect_corners(x, y, w, h, r, CORNERS_ALL, argb);
    }

    pub fn rounded_rect_corners(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        r: f32,
        corners: u8,
        argb: u32,
    ) {
        if w <= 0.0 || h <= 0.0 || argb >> 24 == 0 {
            return;
        }
        let r = r.clamp(0.0, w.min(h) * 0.5);
        if r <= 0.0 || corners == 0 {
            self.fill(x, y, w, h, argb);
            return;
        }
        self.fill(x + r, y, w - 2.0 * r, h, argb);
        self.fill(x, y + r, r, h - 2.0 * r, argb);
        self.fill(x + w - r, y + r, r, h - 2.0 * r, argb);

        let color = unpack(argb);
        let geom = [
            (x + r, y + r, x, y),
            (x + w - r, y + r, x + w - r, y),
            (x + w - r, y + h - r, x + w - r, y + h - r),
            (x + r, y + h - r, x, y + h - r),
        ];
        for (q, &(cx, cy, bx, by)) in geom.iter().enumerate() {
            if corners & (1 << q) != 0 {
                self.corner_fan(cx, cy, r, q, color, argb, bx, by);
            } else {
                self.fill(bx, by, r, r, argb);
            }
        }
    }

    #[allow(clippy::too_many_arguments, reason = "one private geometry helper")]
    fn corner_fan(
        &mut self,
        cx: f32,
        cy: f32,
        r: f32,
        q: usize,
        color: [f32; 4],
        argb: u32,
        bx: f32,
        by: f32,
    ) {
        if let Some([x0, y0, x1, y1]) = self.clip {
            let outside = cx + r <= x0 || cy + r <= y0 || cx - r >= x1 || cy - r >= y1;
            if outside {
                return;
            }
            if cx - r < x0 || cy - r < y0 || cx + r > x1 || cy + r > y1 {
                self.fill(bx, by, r, r, argb);
                return;
            }
        }

        let white = self.atlas.white;
        let uv = self.atlas.uv(white.x as f32 + 0.5, white.y as f32 + 0.5);
        let solid = (r - CORNER_FEATHER).max(0.0);
        let mut faded = color;
        faded[3] = 0.0;

        let base = self.positions.len() as u32;
        let centre = self.place(cx, cy);
        self.positions.push(centre);
        self.uvs.push(uv);
        self.colors.push(color);
        for (radius, ramp) in [(solid, false), (r, true)] {
            for i in 0..=CORNER_SEGMENTS {
                let [ux, uy] = rotate_quadrant(ARC[i], q);
                let v = self.place(cx + ux * radius, cy + uy * radius);
                self.positions.push(v);
                self.uvs.push(uv);
                let end = i == 0 || i == CORNER_SEGMENTS;
                self.colors.push(if ramp && !end { faded } else { color });
            }
        }

        let inner = base + 1;
        let outer = inner + CORNER_SEGMENTS as u32 + 1;
        for i in 0..CORNER_SEGMENTS as u32 {
            self.indices
                .extend_from_slice(&[base, inner + i, inner + i + 1]);
            self.indices.extend_from_slice(&[
                inner + i,
                outer + i,
                inner + i + 1,
                inner + i + 1,
                outer + i,
                outer + i + 1,
            ]);
        }
    }

    pub fn text_plain(&mut self, s: &str, x: f32, y: f32, argb_rgb: u32, shadow: bool) -> f32 {
        if shadow {
            self.plain_run(s, x + 1.0, y + 1.0, Style::shadow_color(argb_rgb));
        }
        self.plain_run(s, x, y, argb_rgb)
    }

    fn plain_run(&mut self, s: &str, x: f32, y: f32, rgb: u32) -> f32 {
        let color = unpack(0xFF00_0000 | rgb);
        let mut pen = x;
        for ch in s.chars() {
            let Some(g) = self.atlas.font.glyph(ch).copied() else {
                continue;
            };
            self.glyph(ch, &g, pen, y, color, false);
            pen += g.advance;
        }
        pen
    }

    pub fn outline(&mut self, x: f32, y: f32, w: f32, h: f32, argb: u32) {
        self.fill(x, y, w, 1.0, argb);
        self.fill(x, y + h - 1.0, w, 1.0, argb);
        self.fill(x, y + 1.0, 1.0, h - 2.0, argb);
        self.fill(x + w - 1.0, y + 1.0, 1.0, h - 2.0, argb);
    }
}

pub fn unpack(argb: u32) -> [f32; 4] {
    [
        srgb_to_linear(((argb >> 16) & 0xFF) as f32 / 255.0),
        srgb_to_linear(((argb >> 8) & 0xFF) as f32 / 255.0),
        srgb_to_linear((argb & 0xFF) as f32 / 255.0),
        ((argb >> 24) & 0xFF) as f32 / 255.0,
    ]
}

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> u32 {
    let i = (h * 6.0).floor();
    let f = h * 6.0 - i;
    let p = v * (1.0 - s);
    let q = v * (1.0 - f * s);
    let t = v * (1.0 - (1.0 - f) * s);
    let (r, g, b) = match (i as i32) % 6 {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    ((r * 255.0) as u32) << 16 | ((g * 255.0) as u32) << 8 | (b * 255.0) as u32
}
