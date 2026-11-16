use super::registry::{Id, player_esp as setting};
use super::store;

#[derive(Clone, Copy, Debug)]
pub struct Config {
    pub alpha: u8,
    pub lines: bool,
    pub tracers: bool,
    pub friend_color: bool,
}

pub fn config() -> Option<Config> {
    let s = store();
    if !s.enabled(Id::PlayerEsp) {
        return None;
    }
    let (alpha, lines, _) = super::esp::table::look(s.choice(setting::SHAPE), s.num(setting::FILL));
    Some(Config {
        alpha,
        lines,
        tracers: s.flag(setting::TRACERS),
        friend_color: s.flag(setting::FRIEND_COLOR),
    })
}
