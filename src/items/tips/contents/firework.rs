use azalea_inventory::components::{FireworkExplosion, FireworkExplosionShape, Fireworks};

use super::{GRAY, Line, gray};
use crate::gui::tooltip::translate;
use crate::text::Span;

const FIREWORK_COLORS: [i32; 16] = [
    15790320, 15435844, 12801229, 6719955, 14602026, 4312372, 14188952, 4408131, 11250603, 2651799,
    8073150, 2437522, 5320730, 3887386, 11743532, 1973019,
];

fn shape_name(shape: FireworkExplosionShape) -> String {
    let name = match shape {
        FireworkExplosionShape::SmallBall => "small_ball",
        FireworkExplosionShape::LargeBall => "large_ball",
        FireworkExplosionShape::Star => "star",
        FireworkExplosionShape::Creeper => "creeper",
        FireworkExplosionShape::Burst => "burst",
    };
    translate(&format!("item.minecraft.firework_star.shape.{name}"), &[])
}

pub(super) fn color_name(color: i32) -> String {
    match FIREWORK_COLORS.iter().position(|c| *c == color) {
        Some(id) => translate(
            &format!(
                "item.minecraft.firework_star.{}",
                crate::util::dye::by_id(id).name
            ),
            &[],
        ),
        None => translate("item.minecraft.firework_star.custom_color", &[]),
    }
}

fn color_list(colors: &[i32], text: &mut String) {
    for (i, color) in colors.iter().enumerate() {
        if i > 0 {
            text.push_str(", ");
        }
        text.push_str(&color_name(*color));
    }
}

pub(super) fn explosion_rows(explosion: &FireworkExplosion, out: &mut Vec<Line>) {
    out.push(gray(shape_name(explosion.shape)));
    additional_rows(explosion, &mut |line: Line| out.push(line));
}

fn additional_rows(explosion: &FireworkExplosion, out: &mut dyn FnMut(Line)) {
    if !explosion.colors.is_empty() {
        let mut text = String::new();
        color_list(&explosion.colors, &mut text);
        out(gray(text));
    }
    if !explosion.fade_colors.is_empty() {
        let mut text = translate("item.minecraft.firework_star.fade_to", &[]);
        text.push(' ');
        color_list(&explosion.fade_colors, &mut text);
        out(gray(text));
    }
    if explosion.has_trail {
        out(gray(translate("item.minecraft.firework_star.trail", &[])));
    }
    if explosion.has_twinkle {
        out(gray(translate("item.minecraft.firework_star.flicker", &[])));
    }
}

pub(super) fn fireworks_rows(fireworks: &Fireworks, out: &mut Vec<Line>) {
    if fireworks.flight_duration > 0 {
        let text = format!(
            "{} {}",
            translate("item.minecraft.firework_rocket.flight", &[]),
            fireworks.flight_duration
        );
        out.push(gray(text));
    }
    let mut runs: Vec<(&FireworkExplosion, usize)> = Vec::new();
    for explosion in &fireworks.explosions {
        if let Some(last) = runs.last_mut()
            && last.0 == explosion
        {
            last.1 += 1;
        } else {
            runs.push((explosion, 1));
        }
    }
    for (explosion, count) in runs {
        let shape = shape_name(explosion.shape);
        let text = if count == 1 {
            translate("item.minecraft.firework_rocket.single_star", &[shape])
        } else {
            translate(
                "item.minecraft.firework_rocket.multiple_stars",
                &[count.to_string(), shape],
            )
        };
        out.push(vec![Span::colored(text, GRAY)]);
        additional_rows(explosion, &mut |line: Line| {
            let mut indented = vec![Span::new("  ")];
            indented.extend(line);
            out.push(indented);
        });
    }
}
