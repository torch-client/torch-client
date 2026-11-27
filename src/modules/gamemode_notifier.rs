use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

use azalea_protocol::packets::game::ClientboundGamePacket;

use super::registry::{Id, gamemode_notifier as setting};
use super::store;
use crate::session::{Gamemode, SharedMutex};

static OWN_HI: AtomicU64 = AtomicU64::new(0);
static OWN_LO: AtomicU64 = AtomicU64::new(0);

pub fn tick(bot: &azalea::Client) {
    if !store().enabled(Id::GamemodeNotifier) {
        return;
    }
    let own = bot.profile().map(|p| p.uuid.as_u128()).unwrap_or(0);
    OWN_HI.store((own >> 64) as u64, Relaxed);
    OWN_LO.store(own as u64, Relaxed);
}

fn own_uuid() -> u128 {
    u128::from(OWN_HI.load(Relaxed)) << 64 | u128::from(OWN_LO.load(Relaxed))
}

fn label(mode: Gamemode) -> &'static str {
    match mode {
        Gamemode::Survival => crate::lang::tr!("modmsg.gamemode.survival"),
        Gamemode::Creative => crate::lang::tr!("modmsg.gamemode.creative"),
        Gamemode::Adventure => crate::lang::tr!("modmsg.gamemode.adventure"),
        Gamemode::Spectator => crate::lang::tr!("modmsg.gamemode.spectator"),
    }
}

fn wanted(mode: Gamemode) -> bool {
    store().flag(match mode {
        Gamemode::Survival => setting::SURVIVAL,
        Gamemode::Creative => setting::CREATIVE,
        Gamemode::Adventure => setting::ADVENTURE,
        Gamemode::Spectator => setting::SPECTATOR,
    })
}

pub fn packet(shared: &SharedMutex, packet: &ClientboundGamePacket) {
    let ClientboundGamePacket::PlayerInfoUpdate(p) = packet else {
        return;
    };
    if !p.actions.update_game_mode || !store().enabled(Id::GamemodeNotifier) {
        return;
    }
    let own = own_uuid();
    let changes: Vec<(String, Gamemode, Gamemode)> = crate::client::tablist::with_lookup(|l| {
        p.entries
            .iter()
            .filter_map(|e| {
                let uuid = e.profile.uuid.as_u128();
                if uuid == own {
                    return None;
                }
                let old = l.gamemode(uuid)?;
                let name = l.name(uuid)?;
                let new = Gamemode::from_azalea(e.game_mode);
                (old != new && wanted(new)).then(|| (name.to_owned(), old, new))
            })
            .collect()
    });
    for (name, old, new) in changes {
        super::chat_math::say(
            shared,
            crate::text::YELLOW,
            crate::lang::trf!("modmsg.gamemode.changed", name, label(old), label(new)),
        );
    }
}
