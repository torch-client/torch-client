use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::Mesh;

use crate::gui::atlas::GuiAtlas;
use crate::gui::painter::unpack;
use crate::text::{Span, Style};

use super::feed::{INHERIT_COLOR, SignFace};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SignKind {
    Standing,
    Hanging,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct SignMetrics {
    pub max_width: f32,
    pub line_height: f32,
}

impl SignKind {
    pub fn of(kind: azalea_registry::builtin::BlockEntityKind) -> Option<SignKind> {
        use azalea_registry::builtin::BlockEntityKind;
        match kind {
            BlockEntityKind::Sign => Some(SignKind::Standing),
            BlockEntityKind::HangingSign => Some(SignKind::Hanging),
            _ => None,
        }
    }

    pub fn metrics(self) -> SignMetrics {
        match self {
            SignKind::Standing => SignMetrics {
                max_width: 90.0,
                line_height: 10.0,
            },
            SignKind::Hanging => SignMetrics {
                max_width: 60.0,
                line_height: 9.0,
            },
        }
    }
}

pub const LINES: usize = 4;

pub const BLACK_TEXT_OUTLINE_COLOR: u32 = 0xF0_EB_CC;

const SHADOW_OFFSET: f32 = 1.0;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct TextStyle {
    pub color: u32,
    pub outline: Option<u32>,
    pub full_bright: bool,
}

pub fn dark_color(face: &SignFace) -> u32 {
    if face.color == 0 && face.glowing {
        BLACK_TEXT_OUTLINE_COLOR
    } else {
        scale_rgb(face.color, 0.4)
    }
}

fn scale_rgb(color: u32, scale: f32) -> u32 {
    let channel = |shift: u32| {
        let value = ((color >> shift) & 0xFF) as f32 * scale;
        (value as u32).min(255) << shift
    };
    channel(16) | channel(8) | channel(0)
}

pub fn text_style(face: &SignFace, draw_outline: bool) -> TextStyle {
    let dark = dark_color(face);
    if face.glowing {
        TextStyle {
            color: face.color,
            outline: (face.color == 0 || draw_outline).then_some(dark),
            full_bright: true,
        }
    } else {
        TextStyle {
            color: dark,
            outline: None,
            full_bright: false,
        }
    }
}

pub fn sign_text_mesh(
    atlas: &GuiAtlas,
    face: &SignFace,
    style: TextStyle,
    metrics: SignMetrics,
) -> Option<Mesh> {
    let mut builder = TextMesh::new(atlas);
    let midpoint = LINES as f32 * metrics.line_height / 2.0;
    for (i, line) in face.lines.iter().enumerate() {
        let spans = first_split_line(atlas, line, style.color, metrics.max_width);
        if spans.is_empty() {
            continue;
        }
        let width = atlas.font.width(&spans).ceil() as i32;
        let x = -(width / 2) as f32;
        let y = i as f32 * metrics.line_height - midpoint;
        if let Some(outline) = style.outline {
            builder.outline_line(&spans, x, y, outline);
        }
        builder.line(&spans, x, y);
    }
    builder.finish()
}

fn first_split_line(atlas: &GuiAtlas, spans: &[Span], color: u32, max_width: f32) -> Vec<Span> {
    if spans.is_empty() {
        return Vec::new();
    }
    let wrapped = atlas.font.wrap(spans, max_width);
    let mut line = wrapped.into_iter().next().unwrap_or_default();
    for span in &mut line {
        if span.style.color == INHERIT_COLOR {
            span.style.color = color;
        }
    }
    line.retain(|span| !span.text.is_empty());
    line
}

struct TextMesh<'a> {
    atlas: &'a GuiAtlas,
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    colors: Vec<[f32; 4]>,
    indices: Vec<u32>,
}

impl<'a> TextMesh<'a> {
    fn new(atlas: &'a GuiAtlas) -> TextMesh<'a> {
        TextMesh {
            atlas,
            positions: Vec::new(),
            normals: Vec::new(),
            uvs: Vec::new(),
            colors: Vec::new(),
            indices: Vec::new(),
        }
    }

    fn outline_line(&mut self, spans: &[Span], x: f32, y: f32, color: u32) {
        let flat: Vec<Span> = spans
            .iter()
            .map(|span| Span {
                text: span.text.clone(),
                style: Style {
                    color,
                    ..span.style
                },
            })
            .collect();
        for xo in -1..=1 {
            for yo in -1..=1 {
                if xo == 0 && yo == 0 {
                    continue;
                }
                self.line(
                    &flat,
                    x + xo as f32 * SHADOW_OFFSET,
                    y + yo as f32 * SHADOW_OFFSET,
                );
            }
        }
    }

    fn line(&mut self, spans: &[Span], x: f32, y: f32) {
        let mut pen = x;
        for span in spans {
            let st = span.style;
            let color = unpack(0xFF00_0000 | st.color);
            let start = pen;
            for ch in span.text.chars() {
                let Some(glyph) = self.atlas.font.glyph(ch).copied() else {
                    pen += self.atlas.font.advance(ch, st.bold);
                    continue;
                };
                self.glyph(&glyph, pen, y, color, st.italic);
                if st.bold {
                    self.glyph(&glyph, pen + 1.0, y, color, st.italic);
                }
                pen += glyph.advance + if st.bold { 1.0 } else { 0.0 };
            }
            let x0 = if start == x { start - 1.0 } else { start };
            if st.strikethrough {
                self.bar(x0, y + 3.5, pen - x0, 1.0, color);
            }
            if st.underline {
                self.bar(x0, y + 8.0, pen - x0, 1.0, color);
            }
        }
    }

    fn glyph(&mut self, glyph: &crate::text::Glyph, x: f32, y: f32, color: [f32; 4], italic: bool) {
        if glyph.rect[2] == 0 || glyph.rect[3] == 0 {
            return;
        }
        let region = self.atlas.glyph_region(glyph.rect);
        let (u0, v0) = (region.x as f32, region.y as f32);
        let shear = if italic { 1.0 } else { 0.0 };
        self.quad(
            x,
            y + glyph.y_off,
            glyph.draw_w,
            glyph.draw_h,
            [u0, v0, u0 + region.w as f32, v0 + region.h as f32],
            color,
            shear,
        );
    }

    fn bar(&mut self, x: f32, y: f32, w: f32, h: f32, color: [f32; 4]) {
        let white = self.atlas.white;
        let (u0, v0) = (white.x as f32, white.y as f32);
        self.quad(
            x,
            y,
            w,
            h,
            [u0, v0, u0 + white.w as f32, v0 + white.h as f32],
            color,
            0.0,
        );
    }

    fn quad(&mut self, x: f32, y: f32, w: f32, h: f32, uv: [f32; 4], color: [f32; 4], shear: f32) {
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        let [au0, av0] = self.atlas.uv(uv[0], uv[1]);
        let [au1, av1] = self.atlas.uv(uv[2], uv[3]);
        let base = self.positions.len() as u32;
        self.positions.push([x + shear, y, 0.0]);
        self.positions.push([x + w + shear, y, 0.0]);
        self.positions.push([x + w, y + h, 0.0]);
        self.positions.push([x, y + h, 0.0]);
        self.uvs.push([au0, av0]);
        self.uvs.push([au1, av0]);
        self.uvs.push([au1, av1]);
        self.uvs.push([au0, av1]);
        for _ in 0..4 {
            self.normals.push([0.0, 0.0, 1.0]);
            self.colors.push(color);
        }
        self.indices
            .extend_from_slice(&[base, base + 2, base + 1, base, base + 3, base + 2]);
    }

    fn finish(self) -> Option<Mesh> {
        if self.positions.is_empty() {
            return None;
        }
        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        );
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, self.positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, self.uvs);
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, self.colors);
        mesh.insert_indices(Indices::U32(self.indices));
        Some(mesh)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn face(color: u32, glowing: bool) -> SignFace {
        SignFace {
            color,
            glowing,
            ..SignFace::default()
        }
    }

    #[test]
    fn plain_text_is_darkened_to_two_fifths() {
        assert_eq!(dark_color(&face(0xFF_FFFF, false)), 0x66_6666);
        assert_eq!(dark_color(&face(0, false)), 0);
    }

    #[test]
    fn glowing_black_text_takes_the_outline_colour() {
        assert_eq!(dark_color(&face(0, true)), BLACK_TEXT_OUTLINE_COLOR);
    }

    #[test]
    fn only_glowing_text_is_full_bright() {
        let glowing = text_style(&face(0xFF_0000, true), false);
        assert!(glowing.full_bright);
        assert_eq!(glowing.color, 0xFF_0000);
        assert_eq!(glowing.outline, None);

        let close = text_style(&face(0xFF_0000, true), true);
        assert_eq!(close.outline, Some(0x66_0000));

        let plain = text_style(&face(0xFF_0000, false), true);
        assert!(!plain.full_bright);
        assert_eq!(plain.color, 0x66_0000);
        assert_eq!(plain.outline, None);
    }
}
