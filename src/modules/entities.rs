use std::borrow::Cow;

use azalea_registry::Registry;
use azalea_registry::builtin::EntityKind;

use super::list::{BitList, Built, Entry, Preset};
use crate::lang::Key;

const PLAYERS_HEADER: Key = Key::of("lists.entities.group.players");
const MONSTERS_HEADER: Key = Key::of("lists.entities.group.monsters");
const ANIMALS_HEADER: Key = Key::of("lists.entities.group.animals");
const VILLAGERS_HEADER: Key = Key::of("lists.entities.group.villagers");
const OTHER_HEADER: Key = Key::of("lists.entities.group.other");

static GROUPS: [Key; 5] = [
    PLAYERS_HEADER,
    MONSTERS_HEADER,
    ANIMALS_HEADER,
    VILLAGERS_HEADER,
    OTHER_HEADER,
];

const PLAYERS: u8 = 0;
const MONSTERS: u8 = 1;
const ANIMALS: u8 = 2;
const VILLAGERS: u8 = 3;
const OTHER: u8 = 4;

fn group_of(kind: EntityKind) -> u8 {
    use EntityKind::*;
    match kind {
        Player => PLAYERS,
        Villager | WanderingTrader => VILLAGERS,
        Blaze | Bogged | Breeze | CaveSpider | Creaking | Creeper | Drowned | ElderGuardian
        | EnderDragon | Enderman | Endermite | Evoker | Ghast | Guardian | Hoglin | Husk
        | Illusioner | MagmaCube | Phantom | Piglin | PiglinBrute | Pillager | Ravager
        | Shulker | Silverfish | Skeleton | Slime | Spider | Stray | Vex | Vindicator | Warden
        | Witch | Wither | WitherSkeleton | Zoglin | Zombie | ZombieVillager | ZombifiedPiglin => {
            MONSTERS
        }
        Allay | Armadillo | Axolotl | Bat | Bee | Camel | Cat | Chicken | Cod | Cow | Dolphin
        | Donkey | Fox | Frog | GlowSquid | Goat | Horse | IronGolem | Llama | Mooshroom | Mule
        | Ocelot | Panda | Parrot | Pig | PolarBear | Pufferfish | Rabbit | Salmon | Sheep
        | SkeletonHorse | Sniffer | SnowGolem | Squid | Strider | Tadpole | TraderLlama
        | TropicalFish | Turtle | Wolf | ZombieHorse => ANIMALS,
        _ => OTHER,
    }
}

fn kinds() -> impl Iterator<Item = EntityKind> {
    (0u32..)
        .take_while(|&id| EntityKind::is_valid_id(id))
        .map_while(<EntityKind as Registry>::from_u32)
}

fn build_with(row: impl Fn(EntityKind) -> Option<(u8, bool, u32)>) -> Built {
    let mut entries = Vec::with_capacity(160);
    let mut keys = Vec::with_capacity(160);
    for kind in kinds() {
        let Some((group, on, color)) = row(kind) else {
            continue;
        };
        keys.push((kind.to_u32(), entries.len() as u32));
        entries.push(Entry {
            group,
            label: Cow::Owned(kind.to_string()),
            shown: Key::NONE,
            on,
            color,
        });
    }
    (entries, keys)
}

fn build() -> Built {
    build_with(|kind| {
        let group = group_of(kind);
        Some((group, group == PLAYERS, 0))
    })
}

static ESP_GROUPS: [Key; 4] = [
    MONSTERS_HEADER,
    ANIMALS_HEADER,
    VILLAGERS_HEADER,
    OTHER_HEADER,
];

const ESP_COLORS: [u32; 5] = [0xFF_FF_FF, 0xFF_55_55, 0x55_FF_55, 0xFF_AA_00, 0xAA_AA_AA];

fn build_esp() -> Built {
    build_with(|kind| {
        let group = group_of(kind);
        (group != PLAYERS).then(|| (group - 1, group != OTHER, ESP_COLORS[group as usize]))
    })
}

const TITLE: Key = Key::of("lists.entities.title");

pub static LIST: BitList = BitList::new("entities", TITLE, 168.0, &GROUPS, false, &[], build);

pub static ESP: BitList = BitList::new(
    "entity_esp",
    TITLE,
    168.0,
    &ESP_GROUPS,
    true,
    &[Preset::Defaults, Preset::Clear],
    build_esp,
);

pub fn kind_enabled(kind: EntityKind) -> bool {
    LIST.key_enabled(kind.to_u32())
}

pub fn esp_color(kind: EntityKind) -> Option<u32> {
    ESP.key_color(kind.to_u32())
}
