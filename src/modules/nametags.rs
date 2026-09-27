use super::registry::{Id, nametags as setting};
use super::store;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sizing {
    Perspective,
    Constant,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Health {
    Off,
    Number,
    Bar,
    Both,
}

impl Health {
    pub fn number(self) -> bool {
        matches!(self, Health::Number | Health::Both)
    }

    pub fn bar(self) -> bool {
        matches!(self, Health::Bar | Health::Both)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Config {
    pub scale: f32,
    pub sizing: Sizing,
    pub health: Health,
    pub gamemode: bool,
    pub distance: bool,
}

pub fn config() -> Option<Config> {
    let s = store();
    if !s.enabled(Id::Nametags) {
        return None;
    }
    Some(Config {
        scale: s.num(Id::Nametags, setting::SCALE),
        sizing: match s.choice(Id::Nametags, setting::SIZING) {
            1 => Sizing::Constant,
            _ => Sizing::Perspective,
        },
        health: match s.choice(Id::Nametags, setting::HEALTH) {
            0 => Health::Off,
            2 => Health::Bar,
            3 => Health::Both,
            _ => Health::Number,
        },
        gamemode: s.flag(Id::Nametags, setting::GAMEMODE),
        distance: s.flag(Id::Nametags, setting::DISTANCE),
    })
}

#[cfg(all(test, feature = "click_gui"))]
mod tests {
    use super::*;

    #[test]
    fn config_follows_the_registry() {
        let s = store();
        assert!(!s.enabled(Id::Nametags));
        assert!(config().is_none());

        s.set_enabled(Id::Nametags, true);
        let cfg = config().expect("on");
        assert_eq!(cfg.sizing, Sizing::Perspective);
        assert!((cfg.scale - 1.6).abs() < 1e-6);
        assert_eq!(cfg.health, Health::Number);
        assert!(cfg.health.number() && !cfg.health.bar());
        assert!(cfg.gamemode && cfg.distance);
        s.set_enabled(Id::Nametags, false);
    }
}
