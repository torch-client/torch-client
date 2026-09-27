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
}

impl SoundCategory {
    pub const ALL: [SoundCategory; Self::COUNT] = [
        SoundCategory::Master,
        SoundCategory::Music,
        SoundCategory::Records,
        SoundCategory::Weather,
        SoundCategory::Blocks,
        SoundCategory::Hostile,
        SoundCategory::Neutral,
        SoundCategory::Players,
        SoundCategory::Ambient,
        SoundCategory::Voice,
    ];

    pub const COUNT: usize = 10;

    pub const fn index(self) -> usize {
        self as usize
    }

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
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            SoundCategory::Master => "Master Volume",
            SoundCategory::Music => "Music",
            SoundCategory::Records => "Jukebox/Note Blocks",
            SoundCategory::Weather => "Weather",
            SoundCategory::Blocks => "Blocks",
            SoundCategory::Hostile => "Hostile Creatures",
            SoundCategory::Neutral => "Friendly Creatures",
            SoundCategory::Players => "Players",
            SoundCategory::Ambient => "Ambient/Environment",
            SoundCategory::Voice => "Voice/Speech",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_category_indexes_its_own_slot() {
        for (slot, category) in SoundCategory::ALL.iter().enumerate() {
            assert_eq!(category.index(), slot, "{category:?}");
        }
        assert_eq!(SoundCategory::ALL.len(), SoundCategory::COUNT);
    }

    #[test]
    fn no_two_categories_share_a_saved_key() {
        let mut keys: Vec<&str> = SoundCategory::ALL.iter().map(|c| c.key()).collect();
        keys.sort_unstable();
        let before = keys.len();
        keys.dedup();
        assert_eq!(keys.len(), before, "duplicate soundCategory key");
    }
}
