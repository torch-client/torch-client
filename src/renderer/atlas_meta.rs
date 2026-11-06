#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MetaFrame {
    index: u32,
    time: Option<u32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AnimMeta {
    frames: Option<Vec<MetaFrame>>,
    frametime: u32,
    width: Option<u32>,
    height: Option<u32>,
    interpolate: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SpriteMeta {
    Animated(AnimMeta),
    Broken,
}

pub fn parse_meta(bytes: &[u8]) -> Option<SpriteMeta> {
    let json =
        serde_json::from_slice::<serde_json::Value>(crate::platform::assets::skip_bom(bytes));
    let Ok(serde_json::Value::Object(json)) = json else {
        return Some(SpriteMeta::Broken);
    };
    let animation = json.get("animation")?;
    Some(parse_animation(animation).map_or(SpriteMeta::Broken, SpriteMeta::Animated))
}

fn int(value: &serde_json::Value) -> Option<i64> {
    value.as_i64().or_else(|| value.as_f64().map(|v| v as i64))
}

fn positive(value: &serde_json::Value) -> Option<u32> {
    int(value)
        .filter(|v| (1..=i64::from(i32::MAX)).contains(v))
        .map(|v| v as u32)
}

fn non_negative(value: &serde_json::Value) -> Option<u32> {
    int(value)
        .filter(|v| (0..=i64::from(i32::MAX)).contains(v))
        .map(|v| v as u32)
}

fn optional<T>(
    value: Option<&serde_json::Value>,
    read: impl Fn(&serde_json::Value) -> Option<T>,
) -> Option<Option<T>> {
    match value {
        None => Some(None),
        Some(value) => read(value).map(Some),
    }
}

fn parse_frame(value: &serde_json::Value) -> Option<MetaFrame> {
    if let Some(index) = non_negative(value) {
        return Some(MetaFrame { index, time: None });
    }
    let index = non_negative(value.as_object()?.get("index")?)?;
    Some(MetaFrame {
        index,
        time: optional(value.get("time"), positive)?,
    })
}

fn parse_animation(value: &serde_json::Value) -> Option<AnimMeta> {
    let section = value.as_object()?;
    let frames = optional(section.get("frames"), |frames| {
        frames
            .as_array()?
            .iter()
            .map(parse_frame)
            .collect::<Option<Vec<_>>>()
    })?;
    Some(AnimMeta {
        frames,
        frametime: optional(section.get("frametime"), positive)?.unwrap_or(1),
        width: optional(section.get("width"), positive)?,
        height: optional(section.get("height"), positive)?,
        interpolate: optional(section.get("interpolate"), serde_json::Value::as_bool)?
            .unwrap_or(false),
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameRect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AnimLayout {
    frame_w: u32,
    frame_h: u32,
    columns: u32,
    pub(super) steps: Vec<(u32, u32)>,
    #[cfg_attr(not(feature = "animated_textures"), allow(dead_code))]
    pub(super) interpolate: bool,
}

impl AnimLayout {
    pub(super) fn rect(&self, index: u32) -> FrameRect {
        FrameRect {
            x: index % self.columns * self.frame_w,
            y: index / self.columns * self.frame_h,
            w: self.frame_w,
            h: self.frame_h,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum SpriteLayout {
    Static(FrameRect),
    Animated(AnimLayout),
    Broken,
}

impl SpriteLayout {
    pub fn of(meta: Option<&SpriteMeta>, width: u32, height: u32) -> SpriteLayout {
        let side = width.min(height);
        let first = |w, h| FrameRect { x: 0, y: 0, w, h };
        let meta = match meta {
            None => return SpriteLayout::Static(first(side, side)),
            Some(SpriteMeta::Broken) => return SpriteLayout::Broken,
            Some(SpriteMeta::Animated(meta)) => meta,
        };
        let (frame_w, frame_h) = match (meta.width, meta.height) {
            (None, None) => (side, side),
            (w, h) => (w.unwrap_or(width), h.unwrap_or(height)),
        };
        if frame_w == 0 || frame_h == 0 || width % frame_w != 0 || height % frame_h != 0 {
            return SpriteLayout::Broken;
        }
        let columns = width / frame_w;
        let count = columns * (height / frame_h);
        let steps: Vec<(u32, u32)> = match &meta.frames {
            Some(frames) => frames
                .iter()
                .filter(|frame| frame.index < count)
                .map(|frame| (frame.index, frame.time.unwrap_or(meta.frametime)))
                .collect(),
            None => (0..count).map(|index| (index, meta.frametime)).collect(),
        };
        if steps.len() <= 1 {
            return SpriteLayout::Static(first(frame_w, frame_h));
        }
        SpriteLayout::Animated(AnimLayout {
            frame_w,
            frame_h,
            columns,
            steps,
            interpolate: meta.interpolate,
        })
    }

    pub fn first_rect(&self) -> Option<FrameRect> {
        match self {
            SpriteLayout::Static(rect) => Some(*rect),
            SpriteLayout::Animated(layout) => Some(layout.rect(layout.steps[0].0)),
            SpriteLayout::Broken => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meta(json: &str) -> SpriteMeta {
        parse_meta(json.as_bytes()).expect("has an animation section")
    }

    fn animated(json: &str, width: u32, height: u32) -> AnimLayout {
        match SpriteLayout::of(Some(&meta(json)), width, height) {
            SpriteLayout::Animated(layout) => layout,
            other => panic!("not animated: {other:?}"),
        }
    }

    const FIRE_0: &str = r#"{"animation":{"frames":[16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31,0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15]}}"#;

    #[test]
    fn meta_defaults_and_fields() {
        assert_eq!(
            meta(r#"{"animation":{}}"#),
            SpriteMeta::Animated(AnimMeta {
                frames: None,
                frametime: 1,
                width: None,
                height: None,
                interpolate: false
            })
        );
        assert_eq!(
            meta(
                r#"{"animation":{"frametime":8,"interpolate":true,"width":4,"height":2,"frames":[1,{"index":0,"time":5},{"index":2}]}}"#
            ),
            SpriteMeta::Animated(AnimMeta {
                frames: Some(vec![
                    MetaFrame {
                        index: 1,
                        time: None
                    },
                    MetaFrame {
                        index: 0,
                        time: Some(5)
                    },
                    MetaFrame {
                        index: 2,
                        time: None
                    },
                ]),
                frametime: 8,
                width: Some(4),
                height: Some(2),
                interpolate: true,
            })
        );
    }

    #[test]
    fn meta_numbers_truncate_as_codec_int_does() {
        let SpriteMeta::Animated(section) = meta(r#"{"animation":{"frametime":2.5}}"#) else {
            panic!()
        };
        assert_eq!(section.frametime, 2);
        assert_eq!(
            meta(r#"{"animation":{"frametime":0.5}}"#),
            SpriteMeta::Broken
        );
    }

    #[test]
    fn meta_without_animation_is_nothing() {
        assert_eq!(
            parse_meta(br#"{"texture":{"alpha_cutoff_bias":0.1}}"#),
            None
        );
        assert_eq!(parse_meta(b"{}"), None);
    }

    #[test]
    fn meta_past_a_byte_order_mark() {
        assert!(matches!(
            parse_meta(b"\xEF\xBB\xBF{\"animation\":{}}"),
            Some(SpriteMeta::Animated(_))
        ));
    }

    #[test]
    fn malformed_meta_is_broken() {
        for bad in [
            "not json",
            "[1]",
            r#"{"animation":3}"#,
            r#"{"animation":{"frametime":0}}"#,
            r#"{"animation":{"frametime":"2"}}"#,
            r#"{"animation":{"width":0}}"#,
            r#"{"animation":{"interpolate":1}}"#,
            r#"{"animation":{"frames":5}}"#,
            r#"{"animation":{"frames":[-1]}}"#,
            r#"{"animation":{"frames":[{"time":2}]}}"#,
            r#"{"animation":{"frames":[{"index":0,"time":0}]}}"#,
        ] {
            assert_eq!(
                parse_meta(bad.as_bytes()),
                Some(SpriteMeta::Broken),
                "{bad}"
            );
        }
    }

    #[test]
    fn layout_without_meta_is_the_top_square() {
        let top = FrameRect {
            x: 0,
            y: 0,
            w: 16,
            h: 16,
        };
        assert_eq!(SpriteLayout::of(None, 16, 16), SpriteLayout::Static(top));
        assert_eq!(SpriteLayout::of(None, 16, 48), SpriteLayout::Static(top));
    }

    #[test]
    fn absent_frames_run_in_order() {
        let layout = animated(r#"{"animation":{"frametime":2}}"#, 16, 48);
        assert_eq!(layout.steps, [(0, 2), (1, 2), (2, 2)]);
    }

    #[test]
    fn frames_cross_a_grid_row_by_row() {
        let layout = animated(r#"{"animation":{"width":8,"height":8}}"#, 16, 16);
        assert_eq!(layout.columns, 2);
        assert_eq!(layout.steps.len(), 4);
        assert_eq!(
            layout.rect(3),
            FrameRect {
                x: 8,
                y: 8,
                w: 8,
                h: 8
            }
        );
    }

    #[test]
    fn invalid_indices_are_dropped() {
        let layout = animated(
            r#"{"animation":{"frames":[0,9,1,{"index":3,"time":4}]}}"#,
            16,
            32,
        );
        assert_eq!(layout.steps, [(0, 1), (1, 1)]);
    }

    #[test]
    fn one_frame_is_static_from_the_corner() {
        let corner = SpriteLayout::Static(FrameRect {
            x: 0,
            y: 0,
            w: 16,
            h: 16,
        });
        assert_eq!(
            SpriteLayout::of(Some(&meta(r#"{"animation":{}}"#)), 16, 16),
            corner
        );
        assert_eq!(
            SpriteLayout::of(Some(&meta(r#"{"animation":{"frames":[2]}}"#)), 16, 48),
            corner
        );
        assert_eq!(
            SpriteLayout::of(Some(&meta(r#"{"animation":{"frames":[]}}"#)), 16, 48),
            corner
        );
    }

    #[test]
    fn non_dividing_images_are_broken() {
        let meta = meta(r#"{"animation":{"width":16,"height":10}}"#);
        assert_eq!(SpriteLayout::of(Some(&meta), 16, 48), SpriteLayout::Broken);
        assert_eq!(
            SpriteLayout::of(Some(&SpriteMeta::Broken), 16, 48),
            SpriteLayout::Broken
        );
        assert_eq!(SpriteLayout::Broken.first_rect(), None);
    }

    #[test]
    fn fire_seeds_from_its_first_listed_frame() {
        let layout = animated(FIRE_0, 16, 512);
        assert_eq!(layout.steps.len(), 32);
        let seed = SpriteLayout::Animated(layout).first_rect();
        assert_eq!(
            seed,
            Some(FrameRect {
                x: 0,
                y: 256,
                w: 16,
                h: 16
            })
        );
    }
}
