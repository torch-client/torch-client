use std::borrow::Cow;

use azalea_registry::Registry;
use azalea_registry::builtin::EntityKind;

use super::list::{BitList, Built, Entry};

static GROUPS: [&str; 5] = ["Players", "Monsters", "Animals", "Villagers", "Other"];

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

fn build() -> Built {
    let mut entries = Vec::with_capacity(160);
    let mut keys = Vec::with_capacity(160);
    for id in 0u32.. {
        if !EntityKind::is_valid_id(id) {
            break;
        }
        let Some(kind) = <EntityKind as Registry>::from_u32(id) else {
            break;
        };
        let group = group_of(kind);
        keys.push((kind.to_u32(), entries.len() as u32));
        entries.push(Entry {
            group,
            label: Cow::Owned(kind.to_string()),
            on: group == PLAYERS,
            color: 0,
        });
    }
    (entries, keys)
}

pub static LIST: BitList = BitList::new("entities", "Entities", 168.0, &GROUPS, false, &[], build);

pub fn kind_enabled(kind: EntityKind) -> bool {
    LIST.key_enabled(kind.to_u32())
}
