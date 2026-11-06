crate::util::indexed_enum! {
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub enum SoundCategory {
        Master,
        Music,
        Records,
        Weather,
        Blocks,
        Hostile,
        Neutral,
        Players,
        Ambient,
        Voice,
        Ui,
    }
}

impl SoundCategory {
    pub const fn key(self) -> &'static str {
        match self {
            SoundCategory::Master => "master",
            SoundCategory::Music => "music",
            SoundCategory::Records => "record",
            SoundCategory::Weather => "weather",
            SoundCategory::Blocks => "block",
            SoundCategory::Hostile => "hostile",
            SoundCategory::Neutral => "neutral",
            SoundCategory::Players => "player",
            SoundCategory::Ambient => "ambient",
            SoundCategory::Voice => "voice",
            SoundCategory::Ui => "ui",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            SoundCategory::Master => "Master Volume",
            SoundCategory::Music => "Music",
            SoundCategory::Records => "Jukebox/Note Blocks",
            SoundCategory::Weather => "Weather",
            SoundCategory::Blocks => "Blocks",
            SoundCategory::Hostile => "Hostile Mobs",
            SoundCategory::Neutral => "Friendly Mobs",
            SoundCategory::Players => "Players",
            SoundCategory::Ambient => "Ambient/Environment",
            SoundCategory::Voice => "Narrator/Voice",
            SoundCategory::Ui => "UI",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_two_categories_share_a_saved_key() {
        let mut keys: Vec<&str> = SoundCategory::ALL.iter().map(|c| c.key()).collect();
        keys.sort_unstable();
        let before = keys.len();
        keys.dedup();
        assert_eq!(keys.len(), before, "duplicate soundCategory key");
    }
}
