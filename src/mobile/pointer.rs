use bevy::input::ButtonInput;
use bevy::input::mouse::MouseButton;
use bevy::input::touch::Touches;
use bevy::math::Vec2;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PointerId {
    Mouse,
    Touch(u64),
}

#[derive(Clone, Copy, Debug)]
pub struct Pointer {
    pub id: PointerId,
    pub pos: Vec2,
    pub pressed: bool,
    pub released: bool,
}

pub fn gather(
    out: &mut Vec<Pointer>,
    touches: &Touches,
    buttons: &ButtonInput<MouseButton>,
    cursor: Option<Vec2>,
    scale: f32,
    dpr: f32,
) {
    out.clear();
    for touch in touches.iter() {
        out.push(Pointer {
            id: PointerId::Touch(touch.id()),
            pos: touch.position() * dpr / scale,
            pressed: touches.just_pressed(touch.id()),
            released: false,
        });
    }
    for touch in touches.iter_just_released() {
        out.push(Pointer {
            id: PointerId::Touch(touch.id()),
            pos: touch.position() * dpr / scale,
            pressed: false,
            released: true,
        });
    }
    if let Some(pos) = cursor {
        let down = buttons.pressed(MouseButton::Left);
        let released = buttons.just_released(MouseButton::Left);
        if down || released {
            out.push(Pointer {
                id: PointerId::Mouse,
                pos: pos / scale,
                pressed: buttons.just_pressed(MouseButton::Left),
                released,
            });
        }
    }
}
