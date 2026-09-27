use crate::gui::painter::Painter;

pub fn background(p: &mut Painter, vw: f32, vh: f32) {
    p.sprite("menu_background", 0.0, 0.0, vw, vh);
}

pub fn list_background(p: &mut Painter, x: f32, y: f32, w: f32, h: f32, scroll: f32) {
    let offset = scroll.rem_euclid(32.0);
    let guard = p.push_clip(x, y, w, h);
    p.sprite("menu_list_background", x, y - offset, w, h + offset);
    p.pop_clip(guard);
    p.sprite("header_separator", x, y - 2.0, w, 2.0);
    p.sprite("footer_separator", x, y + h, w, 2.0);
}
