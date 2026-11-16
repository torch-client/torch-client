#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Role {
    Main,
    Layer,
}

pub(super) struct Entry {
    pub spec: &'static str,
    pub files: &'static [&'static str],
    pub role: Role,
    pub parts: &'static [(&'static str, &'static str)],
}

pub(super) fn entry(spec: &str) -> Option<&'static Entry> {
    ENTRIES.iter().find(|e| e.spec == spec)
}

type Parts = &'static [(&'static str, &'static str)];

const fn main(spec: &'static str, files: &'static [&'static str], parts: Parts) -> Entry {
    Entry {
        spec,
        files,
        role: Role::Main,
        parts,
    }
}

const fn layer(spec: &'static str, files: &'static [&'static str], parts: Parts) -> Entry {
    Entry {
        spec,
        files,
        role: Role::Layer,
        parts,
    }
}

const QUADRUPED: Parts = &[
    ("head", "head"),
    ("body", "body"),
    ("leg1", "right_hind_leg"),
    ("leg2", "left_hind_leg"),
    ("leg3", "right_front_leg"),
    ("leg4", "left_front_leg"),
];

const BIPED: Parts = &[
    ("head", "head"),
    ("headwear", "hat"),
    ("body", "body"),
    ("left_arm", "left_arm"),
    ("right_arm", "right_arm"),
    ("left_leg", "left_leg"),
    ("right_leg", "right_leg"),
];

const BOGGED: Parts = &[
    ("head", "head"),
    ("headwear", "hat"),
    ("body", "body"),
    ("left_arm", "left_arm"),
    ("right_arm", "right_arm"),
    ("left_leg", "left_leg"),
    ("right_leg", "right_leg"),
    ("mushrooms", "mushrooms"),
];

const ARMOR_STAND: Parts = &[
    ("head", "head"),
    ("headwear", "hat"),
    ("body", "body"),
    ("left_arm", "left_arm"),
    ("right_arm", "right_arm"),
    ("left_leg", "left_leg"),
    ("right_leg", "right_leg"),
    ("right", "right_body_stick"),
    ("left", "left_body_stick"),
    ("waist", "shoulder_stick"),
    ("base", "base_plate"),
];

const PIGLIN: Parts = &[
    ("head", "head"),
    ("headwear", "hat"),
    ("body", "body"),
    ("left_arm", "left_arm"),
    ("right_arm", "right_arm"),
    ("left_leg", "left_leg"),
    ("right_leg", "right_leg"),
    ("left_ear", "left_ear"),
    ("right_ear", "right_ear"),
    ("left_sleeve", "left_sleeve"),
    ("right_sleeve", "right_sleeve"),
    ("left_pants", "left_pants"),
    ("right_pants", "right_pants"),
    ("jacket", "jacket"),
];

const PIGLIN_BABY: Parts = &[
    ("head", "head"),
    ("headwear", "hat"),
    ("body", "body"),
    ("left_arm", "left_arm"),
    ("right_arm", "right_arm"),
    ("left_leg", "left_leg"),
    ("right_leg", "right_leg"),
    ("left_ear", "left_ear"),
    ("right_ear", "right_ear"),
    ("left_ear_r1", "left_ear_r1"),
    ("right_ear_r1", "right_ear_r1"),
];

const HORSE: Parts = &[
    ("body", "body"),
    ("head", "head"),
    ("tail", "tail"),
    ("mane", "mane"),
    ("mouth", "upper_mouth"),
    ("left_ear", "left_ear"),
    ("right_ear", "right_ear"),
    ("neck", "head_parts"),
    ("back_left_leg", "left_hind_leg"),
    ("back_right_leg", "right_hind_leg"),
    ("front_left_leg", "left_front_leg"),
    ("front_right_leg", "right_front_leg"),
];

const DONKEY: Parts = &[
    ("body", "body"),
    ("head", "head"),
    ("tail", "tail"),
    ("mane", "mane"),
    ("mouth", "upper_mouth"),
    ("left_ear", "left_ear"),
    ("right_ear", "right_ear"),
    ("neck", "head_parts"),
    ("back_left_leg", "left_hind_leg"),
    ("back_right_leg", "right_hind_leg"),
    ("front_left_leg", "left_front_leg"),
    ("front_right_leg", "right_front_leg"),
    ("right_chest", "right_chest"),
    ("left_chest", "left_chest"),
];

const HORSE_BABY: Parts = &[
    ("body", "body"),
    ("head", "head"),
    ("tail", "tail"),
    ("left_ear", "left_ear"),
    ("right_ear", "right_ear"),
    ("neck", "head_parts"),
    ("back_left_leg", "left_hind_leg"),
    ("back_right_leg", "right_hind_leg"),
    ("front_left_leg", "left_front_leg"),
    ("front_right_leg", "right_front_leg"),
];

const DONKEY_BABY: Parts = &[
    ("body", "body"),
    ("head", "head"),
    ("tail", "tail"),
    ("left_ear", "left_ear"),
    ("right_ear", "right_ear"),
    ("neck", "head_parts"),
    ("back_left_leg", "left_hind_leg"),
    ("back_right_leg", "right_hind_leg"),
    ("front_left_leg", "left_front_leg"),
    ("front_right_leg", "right_front_leg"),
    ("right_chest", "right_chest"),
    ("left_chest", "left_chest"),
];

const CAMEL: Parts = &[
    ("body", "body"),
    ("hump", "hump"),
    ("tail", "tail"),
    ("head", "head"),
    ("left_ear", "left_ear"),
    ("right_ear", "right_ear"),
    ("back_left_leg", "left_hind_leg"),
    ("back_right_leg", "right_hind_leg"),
    ("front_left_leg", "left_front_leg"),
    ("front_right_leg", "right_front_leg"),
];

const CAMEL_BABY: Parts = &[
    ("body", "body"),
    ("tail", "tail"),
    ("head", "head"),
    ("left_ear", "left_ear"),
    ("right_ear", "right_ear"),
    ("back_left_leg", "left_hind_leg"),
    ("back_right_leg", "right_hind_leg"),
    ("front_left_leg", "left_front_leg"),
    ("front_right_leg", "right_front_leg"),
];

const CAMEL_SADDLE: Parts = &[
    ("body", "body"),
    ("hump", "hump"),
    ("tail", "tail"),
    ("head", "head"),
    ("left_ear", "left_ear"),
    ("right_ear", "right_ear"),
    ("back_left_leg", "left_hind_leg"),
    ("back_right_leg", "right_hind_leg"),
    ("front_left_leg", "left_front_leg"),
    ("front_right_leg", "right_front_leg"),
    ("saddle", "saddle"),
    ("reins", "reins"),
    ("bridle", "bridle"),
];

const CAT: Parts = &[
    ("head", "head"),
    ("body", "body"),
    ("tail", "tail1"),
    ("tail2", "tail2"),
    ("back_left_leg", "left_hind_leg"),
    ("back_right_leg", "right_hind_leg"),
    ("front_left_leg", "left_front_leg"),
    ("front_right_leg", "right_front_leg"),
];

const LLAMA: Parts = &[
    ("head", "head"),
    ("body", "body"),
    ("leg1", "right_hind_leg"),
    ("leg2", "left_hind_leg"),
    ("leg3", "right_front_leg"),
    ("leg4", "left_front_leg"),
    ("chest_left", "left_chest"),
    ("chest_right", "right_chest"),
];

const WOLF: Parts = &[
    ("body", "body"),
    ("head", "head"),
    ("tail", "tail"),
    ("mane", "upper_body"),
    ("leg1", "right_hind_leg"),
    ("leg2", "left_hind_leg"),
    ("leg3", "right_front_leg"),
    ("leg4", "left_front_leg"),
];

const WOLF_BABY: Parts = &[
    ("body", "body"),
    ("head", "head"),
    ("tail", "tail"),
    ("leg1", "right_hind_leg"),
    ("leg2", "left_hind_leg"),
    ("leg3", "right_front_leg"),
    ("leg4", "left_front_leg"),
];

const FOX: Parts = &[
    ("head", "head"),
    ("body", "body"),
    ("leg1", "right_hind_leg"),
    ("leg2", "left_hind_leg"),
    ("leg3", "right_front_leg"),
    ("leg4", "left_front_leg"),
    ("tail", "tail"),
];

const GOAT: Parts = &[
    ("head", "head"),
    ("body", "body"),
    ("leg1", "right_hind_leg"),
    ("leg2", "left_hind_leg"),
    ("leg3", "right_front_leg"),
    ("leg4", "left_front_leg"),
    ("left_horn", "left_horn"),
    ("right_horn", "right_horn"),
    ("nose", "nose"),
];

const GOAT_BABY: Parts = &[
    ("head", "head"),
    ("body", "body"),
    ("leg1", "right_hind_leg"),
    ("leg2", "left_hind_leg"),
    ("leg3", "right_front_leg"),
    ("leg4", "left_front_leg"),
    ("left_horn", "left_horn"),
    ("right_horn", "right_horn"),
];

const HOGLIN: Parts = &[
    ("head", "head"),
    ("body", "body"),
    ("back_right_leg", "right_hind_leg"),
    ("back_left_leg", "left_hind_leg"),
    ("front_right_leg", "right_front_leg"),
    ("front_left_leg", "left_front_leg"),
    ("mane", "mane"),
    ("left_ear", "left_ear"),
    ("right_ear", "right_ear"),
];

const HOGLIN_BABY: Parts = &[
    ("head", "head"),
    ("body", "body"),
    ("back_right_leg", "right_hind_leg"),
    ("back_left_leg", "left_hind_leg"),
    ("front_right_leg", "right_front_leg"),
    ("front_left_leg", "left_front_leg"),
    ("left_ear", "left_ear"),
    ("right_ear", "right_ear"),
];

const RABBIT: Parts = &[
    ("body", "body"),
    ("left_foot", "left_haunch"),
    ("right_foot", "right_haunch"),
    ("left_thigh", "left_hind_leg"),
    ("right_thigh", "right_hind_leg"),
    ("left_arm", "left_front_leg"),
    ("right_arm", "right_front_leg"),
    ("head", "head"),
    ("frontlegs", "frontlegs"),
    ("backlegs", "backlegs"),
    ("right_ear", "right_ear"),
    ("left_ear", "left_ear"),
    ("tail", "tail"),
];

const RABBIT_BABY: Parts = &[
    ("body", "body_r1"),
    ("left_foot", "left_haunch"),
    ("right_foot", "right_haunch"),
    ("left_thigh", "left_hind_leg"),
    ("right_thigh", "right_hind_leg"),
    ("left_arm", "left_front_leg"),
    ("right_arm", "right_front_leg"),
    ("head", "head"),
    ("frontlegs", "frontlegs"),
    ("backlegs", "backlegs"),
    ("right_ear", "right_ear"),
    ("left_ear", "left_ear"),
    ("tail", "tail_r1"),
];

const SNIFFER: Parts = &[
    ("bone", "bone"),
    ("body", "body"),
    ("front_right_leg", "right_front_leg"),
    ("middle_right_leg", "right_mid_leg"),
    ("back_right_leg", "right_hind_leg"),
    ("front_left_leg", "left_front_leg"),
    ("middle_left_leg", "left_mid_leg"),
    ("back_left_leg", "left_hind_leg"),
    ("head", "head"),
    ("left_ear", "left_ear"),
    ("right_ear", "right_ear"),
    ("nose", "nose"),
    ("lower_beak", "lower_beak"),
];

const STRIDER: Parts = &[
    ("body", "body"),
    ("right_leg", "right_leg"),
    ("left_leg", "left_leg"),
    ("hair_right_top", "right_top_bristle"),
    ("hair_right_middle", "right_middle_bristle"),
    ("hair_right_bottom", "right_bottom_bristle"),
    ("hair_left_top", "left_top_bristle"),
    ("hair_left_middle", "left_middle_bristle"),
    ("hair_left_bottom", "left_bottom_bristle"),
];

const TURTLE: Parts = &[
    ("head", "head"),
    ("body", "body"),
    ("body2", "egg_belly"),
    ("leg1", "right_hind_leg"),
    ("leg2", "left_hind_leg"),
    ("leg3", "right_front_leg"),
    ("leg4", "left_front_leg"),
];

const TURTLE_BABY: Parts = &[
    ("head", "head"),
    ("body", "body"),
    ("leg1", "right_hind_leg"),
    ("leg2", "left_hind_leg"),
    ("leg3", "right_front_leg"),
    ("leg4", "left_front_leg"),
];

const ARMADILLO: Parts = &[
    ("body", "body"),
    ("head", "head"),
    ("left_ear", "left_ear"),
    ("right_ear", "right_ear"),
    ("left_ear_cube", "left_ear_cube"),
    ("right_ear_cube", "right_ear_cube"),
    ("back_left_leg", "left_hind_leg"),
    ("back_right_leg", "right_hind_leg"),
    ("front_left_leg", "left_front_leg"),
    ("front_right_leg", "right_front_leg"),
    ("cube", "cube"),
    ("tail", "tail"),
];

const CHICKEN: Parts = &[
    ("head", "head"),
    ("body", "body"),
    ("right_leg", "right_leg"),
    ("left_leg", "left_leg"),
    ("right_wing", "right_wing"),
    ("left_wing", "left_wing"),
    ("bill", "beak"),
    ("chin", "red_thing"),
];

const CHICKEN_BABY: Parts = &[
    ("body", "body"),
    ("right_leg", "right_leg"),
    ("left_leg", "left_leg"),
    ("right_wing", "right_wing"),
    ("left_wing", "left_wing"),
];

const ALLAY: Parts = &[
    ("head", "head"),
    ("body", "body"),
    ("left_arm", "left_arm"),
    ("right_arm", "right_arm"),
    ("left_wing", "left_wing"),
    ("right_wing", "right_wing"),
];

const AXOLOTL: Parts = &[
    ("head", "head"),
    ("body", "body"),
    ("leg1", "right_hind_leg"),
    ("leg2", "left_hind_leg"),
    ("leg3", "right_front_leg"),
    ("leg4", "left_front_leg"),
    ("right_gills", "right_gills"),
    ("top_gills", "top_gills"),
    ("left_gills", "left_gills"),
    ("tail", "tail"),
];

const BAT: Parts = &[
    ("body", "body"),
    ("head", "head"),
    ("right_wing", "right_wing"),
    ("left_wing", "left_wing"),
    ("outer_right_wing", "right_wing_tip"),
    ("outer_left_wing", "left_wing_tip"),
    ("feet", "feet"),
];

const BEE: Parts = &[
    ("body", "bone"),
    ("torso", "body"),
    ("right_wing", "right_wing"),
    ("left_wing", "left_wing"),
    ("front_legs", "front_legs"),
    ("middle_legs", "middle_legs"),
    ("back_legs", "back_legs"),
    ("stinger", "stinger"),
    ("left_antenna", "left_antenna"),
    ("right_antenna", "right_antenna"),
];

const BEE_BABY: Parts = &[
    ("body", "bone"),
    ("torso", "body"),
    ("stinger", "stinger"),
    ("right_wing", "right_wing"),
    ("left_wing", "left_wing"),
    ("front_legs", "front_legs"),
    ("middle_legs", "middle_legs"),
    ("back_legs", "back_legs"),
];

const DOLPHIN: Parts = &[
    ("body", "body"),
    ("tail", "tail"),
    ("tail_fin", "tail_fin"),
    ("head", "head"),
    ("right_fin", "right_fin"),
    ("left_fin", "left_fin"),
    ("back_fin", "back_fin"),
];

const COD: Parts = &[
    ("body", "body"),
    ("head", "head"),
    ("nose", "nose"),
    ("tail", "tail_fin"),
    ("fin_right", "right_fin"),
    ("fin_left", "left_fin"),
    ("fin_back", "top_fin"),
];

const SALMON: Parts = &[
    ("body_front", "body_front"),
    ("body_back", "body_back"),
    ("head", "head"),
    ("fin_back_1", "top_front_fin"),
    ("fin_back_2", "top_back_fin"),
    ("tail", "back_fin"),
    ("fin_right", "right_fin"),
    ("fin_left", "left_fin"),
];

const TROPICAL_A: Parts = &[
    ("body", "body"),
    ("tail", "tail"),
    ("fin_right", "right_fin"),
    ("fin_left", "left_fin"),
    ("fin_top", "top_fin"),
];

const TROPICAL_B: Parts = &[
    ("body", "body"),
    ("tail", "tail"),
    ("fin_right", "right_fin"),
    ("fin_left", "left_fin"),
    ("fin_top", "top_fin"),
    ("fin_bottom", "bottom_fin"),
];

const PUFFER_BIG: Parts = &[
    ("body", "body"),
    ("fin_right", "right_blue_fin"),
    ("fin_left", "left_blue_fin"),
    ("spikes_front_top", "top_front_fin"),
    ("spikes_middle_top", "top_middle_fin"),
    ("spikes_back_top", "top_back_fin"),
    ("spikes_front_right", "right_front_fin"),
    ("spikes_front_left", "left_front_fin"),
    ("spikes_front_bottom", "bottom_front_fin"),
    ("spikes_middle_bottom", "bottom_middle_fin"),
    ("spikes_back_bottom", "bottom_back_fin"),
    ("spikes_back_right", "right_back_fin"),
    ("spikes_back_left", "left_back_fin"),
];

const PUFFER_MEDIUM: Parts = &[
    ("body", "body"),
    ("fin_right", "right_blue_fin"),
    ("fin_left", "left_blue_fin"),
    ("spikes_front_top", "top_front_fin"),
    ("spikes_back_top", "top_back_fin"),
    ("spikes_front_right", "right_front_fin"),
    ("spikes_front_left", "left_front_fin"),
    ("spikes_front_bottom", "bottom_front_fin"),
    ("spikes_back_bottom", "bottom_back_fin"),
    ("spikes_back_right", "right_back_fin"),
    ("spikes_back_left", "left_back_fin"),
];

const PUFFER_SMALL: Parts = &[
    ("body", "body"),
    ("fin_right", "right_fin"),
    ("fin_left", "left_fin"),
    ("eye_right", "right_eye"),
    ("eye_left", "left_eye"),
    ("tail", "back_fin"),
];

const FROG: Parts = &[
    ("head", "head"),
    ("body", "body"),
    ("left_leg", "left_leg"),
    ("right_leg", "right_leg"),
    ("croaking_body", "croaking_body"),
    ("left_arm", "left_arm"),
    ("right_arm", "right_arm"),
    ("tongue", "tongue"),
    ("eyes", "eyes"),
];

const TADPOLE: Parts = &[("body", "body"), ("tail", "tail")];

const GHAST: Parts = &[
    ("body", "body"),
    ("tentacle1", "tentacle0"),
    ("tentacle2", "tentacle1"),
    ("tentacle3", "tentacle2"),
    ("tentacle4", "tentacle3"),
    ("tentacle5", "tentacle4"),
    ("tentacle6", "tentacle5"),
    ("tentacle7", "tentacle6"),
    ("tentacle8", "tentacle7"),
    ("tentacle9", "tentacle8"),
];

const HAPPY_GHAST_BABY: Parts = &[
    ("body", "body"),
    ("inner_body", "inner_body"),
    ("tentacle1", "tentacle0"),
    ("tentacle2", "tentacle1"),
    ("tentacle3", "tentacle2"),
    ("tentacle4", "tentacle3"),
    ("tentacle5", "tentacle4"),
    ("tentacle6", "tentacle5"),
    ("tentacle7", "tentacle6"),
    ("tentacle8", "tentacle7"),
    ("tentacle9", "tentacle8"),
];

const HAPPY_GHAST_HARNESS: Parts = &[("body", "harness"), ("goggles", "goggles")];

const NAUTILUS: Parts = &[
    ("body", "body"),
    ("shell", "shell"),
    ("upper_mouth", "upper_mouth"),
    ("inner_mouth", "inner_mouth"),
    ("lower_mouth", "lower_mouth"),
];

const NAUTILUS_CORAL: Parts = &[
    ("body", "body"),
    ("shell", "shell"),
    ("upper_mouth", "upper_mouth"),
    ("inner_mouth", "inner_mouth"),
    ("lower_mouth", "lower_mouth"),
    ("coral", "root"),
];

const PARROT: Parts = &[
    ("head", "head"),
    ("body", "body"),
    ("tail", "tail"),
    ("left_wing", "left_wing"),
    ("right_wing", "right_wing"),
    ("left_leg", "left_leg"),
    ("right_leg", "right_leg"),
];

const SQUID: Parts = &[
    ("body", "body"),
    ("tentacle1", "tentacle0"),
    ("tentacle2", "tentacle1"),
    ("tentacle3", "tentacle2"),
    ("tentacle4", "tentacle3"),
    ("tentacle5", "tentacle4"),
    ("tentacle6", "tentacle5"),
    ("tentacle7", "tentacle6"),
    ("tentacle8", "tentacle7"),
];

const VILLAGER: Parts = &[
    ("head", "head"),
    ("headwear", "hat"),
    ("headwear2", "hat_rim"),
    ("bodywear", "jacket"),
    ("body", "body"),
    ("arms", "arms"),
    ("right_leg", "right_leg"),
    ("left_leg", "left_leg"),
    ("nose", "nose"),
];

const VILLAGER_BABY: Parts = &[
    ("head", "head"),
    ("headwear", "hat"),
    ("headwear2", "hat_rim"),
    ("bodywear", "bb_main"),
    ("body", "body"),
    ("arms", "arms"),
    ("right_leg", "right_leg"),
    ("left_leg", "left_leg"),
    ("nose", "nose"),
];

const WITCH: Parts = &[
    ("head", "head"),
    ("headwear", "hat"),
    ("headwear2", "hat_rim"),
    ("bodywear", "jacket"),
    ("body", "body"),
    ("arms", "arms"),
    ("right_leg", "right_leg"),
    ("left_leg", "left_leg"),
    ("nose", "nose"),
    ("mole", "mole"),
];

const ILLAGER: Parts = &[
    ("head", "head"),
    ("headwear", "hat"),
    ("body", "body"),
    ("nose", "nose"),
    ("arms", "arms"),
    ("left_arm", "left_arm"),
    ("right_arm", "right_arm"),
    ("left_leg", "left_leg"),
    ("right_leg", "right_leg"),
];

const BLAZE: Parts = &[
    ("head", "head"),
    ("stick1", "part0"),
    ("stick2", "part1"),
    ("stick3", "part2"),
    ("stick4", "part3"),
    ("stick5", "part4"),
    ("stick6", "part5"),
    ("stick7", "part6"),
    ("stick8", "part7"),
    ("stick9", "part8"),
    ("stick10", "part9"),
    ("stick11", "part10"),
    ("stick12", "part11"),
];

const BREEZE: Parts = &[
    ("body", "body"),
    ("rods", "rods"),
    ("head", "head"),
    ("wind_body", "wind_body"),
    ("wind_middle", "wind_mid"),
    ("wind_bottom", "wind_bottom"),
    ("wind_top", "wind_top"),
];

const COPPER_GOLEM: Parts = &[
    ("body", "body"),
    ("head", "head"),
    ("right_arm", "right_arm"),
    ("left_arm", "left_arm"),
    ("right_leg", "right_leg"),
    ("left_leg", "left_leg"),
];

const DRAGON: Parts = &[
    ("head", "head"),
    ("jaw", "jaw"),
    ("body", "body"),
    ("neck1", "neck0"),
    ("neck2", "neck1"),
    ("neck3", "neck2"),
    ("neck4", "neck3"),
    ("neck5", "neck4"),
    ("tail1", "tail0"),
    ("tail2", "tail1"),
    ("tail3", "tail2"),
    ("tail4", "tail3"),
    ("tail5", "tail4"),
    ("tail6", "tail5"),
    ("tail7", "tail6"),
    ("tail8", "tail7"),
    ("tail9", "tail8"),
    ("tail10", "tail9"),
    ("tail11", "tail10"),
    ("tail12", "tail11"),
    ("left_wing", "left_wing"),
    ("left_wing_tip", "left_wing_tip"),
    ("right_wing", "right_wing"),
    ("right_wing_tip", "right_wing_tip"),
    ("front_left_leg", "left_front_leg"),
    ("front_left_shin", "left_front_leg_tip"),
    ("front_left_foot", "left_front_foot"),
    ("back_left_leg", "left_hind_leg"),
    ("back_left_shin", "left_hind_leg_tip"),
    ("back_left_foot", "left_hind_foot"),
    ("front_right_leg", "right_front_leg"),
    ("front_right_shin", "right_front_leg_tip"),
    ("front_right_foot", "right_front_foot"),
    ("back_right_leg", "right_hind_leg"),
    ("back_right_shin", "right_hind_leg_tip"),
    ("back_right_foot", "right_hind_foot"),
];

const GUARDIAN: Parts = &[
    ("tail1", "tail0"),
    ("tail2", "tail1"),
    ("tail3", "tail2"),
    ("body", "head"),
    ("eye", "eye"),
    ("spine1", "spike0"),
    ("spine2", "spike1"),
    ("spine3", "spike2"),
    ("spine4", "spike3"),
    ("spine5", "spike4"),
    ("spine6", "spike5"),
    ("spine7", "spike6"),
    ("spine8", "spike7"),
    ("spine9", "spike8"),
    ("spine10", "spike9"),
    ("spine11", "spike10"),
    ("spine12", "spike11"),
];

const IRON_GOLEM: Parts = &[
    ("head", "head"),
    ("body", "body"),
    ("left_arm", "left_arm"),
    ("right_arm", "right_arm"),
    ("left_leg", "left_leg"),
    ("right_leg", "right_leg"),
];

const PHANTOM: Parts = &[
    ("body", "body"),
    ("head", "head"),
    ("tail", "tail_base"),
    ("tail2", "tail_tip"),
    ("left_wing", "left_wing_base"),
    ("right_wing", "right_wing_base"),
    ("left_wing_tip", "left_wing_tip"),
    ("right_wing_tip", "right_wing_tip"),
];

const RAVAGER: Parts = &[
    ("head", "head"),
    ("body", "body"),
    ("leg1", "right_hind_leg"),
    ("leg2", "left_hind_leg"),
    ("leg3", "right_front_leg"),
    ("leg4", "left_front_leg"),
    ("jaw", "mouth"),
    ("neck", "neck"),
];

const SHULKER: Parts = &[("lid", "lid"), ("base", "base"), ("head", "head")];

const SILVERFISH: Parts = &[
    ("body1", "segment0"),
    ("body2", "segment1"),
    ("body3", "segment2"),
    ("body4", "segment3"),
    ("body5", "segment4"),
    ("body6", "segment5"),
    ("body7", "segment6"),
    ("wing1", "layer0"),
    ("wing2", "layer1"),
    ("wing3", "layer2"),
];

const ENDERMITE: Parts = &[
    ("body1", "segment0"),
    ("body2", "segment1"),
    ("body3", "segment2"),
    ("body4", "segment3"),
];

const SLIME: Parts = &[
    ("body", "cube"),
    ("left_eye", "left_eye"),
    ("right_eye", "right_eye"),
    ("mouth", "mouth"),
];

const SLIME_OUTER: Parts = &[("body", "cube")];

const MAGMA_CUBE: Parts = &[
    ("core", "inside_cube"),
    ("segment1", "cube0"),
    ("segment2", "cube1"),
    ("segment3", "cube2"),
    ("segment4", "cube3"),
    ("segment5", "cube4"),
    ("segment6", "cube5"),
    ("segment7", "cube6"),
    ("segment8", "cube7"),
];

const SNOW_GOLEM: Parts = &[
    ("body", "upper_body"),
    ("body_bottom", "lower_body"),
    ("head", "head"),
    ("left_hand", "left_arm"),
    ("right_hand", "right_arm"),
];

const SPIDER: Parts = &[
    ("head", "head"),
    ("neck", "body0"),
    ("body", "body1"),
    ("leg1", "right_hind_leg"),
    ("leg2", "left_hind_leg"),
    ("leg3", "right_middle_hind_leg"),
    ("leg4", "left_middle_hind_leg"),
    ("leg5", "right_middle_front_leg"),
    ("leg6", "left_middle_front_leg"),
    ("leg7", "right_front_leg"),
    ("leg8", "left_front_leg"),
];

const WARDEN: Parts = &[
    ("body", "bone"),
    ("torso", "body"),
    ("head", "head"),
    ("left_leg", "left_leg"),
    ("right_leg", "right_leg"),
    ("left_arm", "left_arm"),
    ("right_arm", "right_arm"),
    ("left_tendril", "left_tendril"),
    ("right_tendril", "right_tendril"),
    ("left_ribcage", "left_ribcage"),
    ("right_ribcage", "right_ribcage"),
];

const WITHER: Parts = &[
    ("body1", "shoulders"),
    ("body2", "ribcage"),
    ("body3", "tail"),
    ("head1", "center_head"),
    ("head2", "right_head"),
    ("head3", "left_head"),
];

const BOAT: Parts = &[
    ("bottom", "bottom"),
    ("back", "back"),
    ("front", "front"),
    ("right", "right"),
    ("left", "left"),
    ("paddle_left", "left_paddle"),
    ("paddle_right", "right_paddle"),
];

const CHEST_BOAT: Parts = &[
    ("bottom", "bottom"),
    ("back", "back"),
    ("front", "front"),
    ("right", "right"),
    ("left", "left"),
    ("paddle_left", "left_paddle"),
    ("paddle_right", "right_paddle"),
    ("chest_base", "chest_bottom"),
    ("chest_lid", "chest_lid"),
    ("chest_knob", "chest_lock"),
];

const RAFT: Parts = &[
    ("bottom", "bottom"),
    ("paddle_left", "left_paddle"),
    ("paddle_right", "right_paddle"),
];

const CHEST_RAFT: Parts = &[
    ("bottom", "bottom"),
    ("paddle_left", "left_paddle"),
    ("paddle_right", "right_paddle"),
    ("chest_base", "chest_bottom"),
    ("chest_lid", "chest_lid"),
    ("chest_knob", "chest_lock"),
];

const BOAT_PATCH: Parts = &[("water_patch", "water_patch")];

const MINECART: Parts = &[
    ("bottom", "bottom"),
    ("back", "back"),
    ("front", "front"),
    ("right", "right"),
    ("left", "left"),
];

const END_CRYSTAL: Parts = &[
    ("cube", "cube"),
    ("inner_glass", "inner_glass"),
    ("outer_glass", "outer_glass"),
    ("base", "base"),
];

const EVOKER_FANGS: Parts = &[
    ("base", "base"),
    ("upper_jaw", "upper_jaw"),
    ("lower_jaw", "lower_jaw"),
];

const LEAD_KNOT: Parts = &[("knot", "knot")];

const ARROW: Parts = &[
    ("back", "back"),
    ("cross_1", "cross_1"),
    ("cross_2", "cross_2"),
];

const TRIDENT: Parts = &[
    ("body", "pole"),
    ("base", "base"),
    ("left_spike", "left_spike"),
    ("middle_spike", "middle_spike"),
    ("right_spike", "right_spike"),
];

const LLAMA_SPIT: Parts = &[("body", "main")];

const SHULKER_BULLET: Parts = &[("bullet", "main")];

const WIND_CHARGE: Parts = &[("wind", "wind"), ("charge", "wind_charge")];

const HEAD_ONLY: Parts = &[("head", "head")];

static ENTRIES: &[Entry] = &[
    main("armadillo", &["armadillo"], ARMADILLO),
    main("armadillo_baby", &["armadillo_baby"], &[]),
    main("camel", &["camel"], CAMEL),
    main("camel_baby", &["camel_baby"], CAMEL_BABY),
    main("camel_husk", &["camel_husk", "camel"], CAMEL),
    main("camel_husk_saddle", &["camel_husk_saddle"], CAMEL_SADDLE),
    main("cat", &["cat"], CAT),
    main("cat_baby", &["cat_baby"], CAT),
    main("cat_collar", &["cat_collar", "cat"], CAT),
    main("cat_collar_baby", &["cat_baby_collar", "cat_baby"], CAT),
    main("ocelot", &["ocelot"], CAT),
    main("ocelot_baby", &["ocelot_baby"], CAT),
    main("chicken", &["temperate_chicken", "chicken"], CHICKEN),
    main("chicken_cold", &["cold_chicken"], CHICKEN),
    main(
        "chicken_baby",
        &["temperate_chicken_baby", "chicken_baby"],
        CHICKEN_BABY,
    ),
    main("cow", &["temperate_cow", "cow"], QUADRUPED),
    main("cow_warm", &["warm_cow"], QUADRUPED),
    main("cow_cold", &["cold_cow"], QUADRUPED),
    main("cow_baby", &["temperate_cow_baby", "cow_baby"], QUADRUPED),
    main("mooshroom", &["mooshroom"], QUADRUPED),
    main("mooshroom_baby", &["mooshroom_baby"], QUADRUPED),
    main("donkey", &["donkey"], DONKEY),
    main("donkey_baby", &["donkey_baby"], DONKEY_BABY),
    main("mule", &["mule"], DONKEY),
    main("mule_baby", &["mule_baby"], DONKEY_BABY),
    main("horse", &["horse"], HORSE),
    main("horse_baby", &["horse_baby"], HORSE_BABY),
    main("horse_armor", &["horse_armor"], HORSE),
    layer("horse_markings", &["horse"], HORSE),
    layer("horse_markings_baby", &["horse_baby"], HORSE_BABY),
    main("fox", &["fox"], FOX),
    main("fox_baby", &["fox_baby"], FOX),
    main("goat", &["goat"], GOAT),
    main("goat_baby", &["goat_baby"], GOAT_BABY),
    main("hoglin", &["hoglin"], HOGLIN),
    main("hoglin_baby", &["hoglin_baby"], HOGLIN_BABY),
    main("llama", &["llama"], LLAMA),
    main("llama_baby", &["llama_baby"], LLAMA),
    main("llama_decor", &["llama_decor"], LLAMA),
    main("llama_baby_decor", &["llama_baby_decor"], LLAMA),
    main("panda", &["panda"], QUADRUPED),
    main("panda_baby", &["panda_baby"], QUADRUPED),
    main("pig", &["temperate_pig", "pig"], QUADRUPED),
    main("pig_cold", &["cold_pig"], QUADRUPED),
    main("pig_baby", &["temperate_pig_baby", "pig_baby"], QUADRUPED),
    main("polar_bear", &["polar_bear"], QUADRUPED),
    main("polar_bear_baby", &["polar_bear_baby"], QUADRUPED),
    main("rabbit", &["rabbit"], RABBIT),
    main("rabbit_baby", &["rabbit_baby"], RABBIT_BABY),
    main("sheep", &["sheep"], QUADRUPED),
    main("sheep_baby", &["sheep_baby"], QUADRUPED),
    main("sheep_wool", &["sheep_wool"], QUADRUPED),
    main("sheep_wool_baby", &["sheep_baby_wool"], QUADRUPED),
    main(
        "sheep_wool_undercoat",
        &["sheep_wool_undercoat", "sheep"],
        QUADRUPED,
    ),
    main("sniffer", &["sniffer"], SNIFFER),
    main("snifflet", &["sniffer_baby"], SNIFFER),
    main("strider", &["strider"], STRIDER),
    main("strider_baby", &["strider_baby"], &[]),
    main("turtle", &["turtle"], TURTLE),
    main("turtle_baby", &["turtle_baby"], TURTLE_BABY),
    main("wolf", &["wolf"], WOLF),
    main("wolf_baby", &["wolf_baby"], WOLF_BABY),
    main("wolf_armor", &["wolf_armor"], WOLF),
    main("wolf_collar", &["wolf_collar", "wolf"], WOLF),
    main(
        "wolf_collar_baby",
        &["wolf_baby_collar", "wolf_baby"],
        WOLF_BABY,
    ),
    main("allay", &["allay"], ALLAY),
    main("axolotl", &["axolotl"], AXOLOTL),
    main("axolotl_baby", &["axolotl_baby"], AXOLOTL),
    main("bat", &["bat"], BAT),
    main("bee", &["bee"], BEE),
    main("bee_baby", &["bee_baby"], BEE_BABY),
    main("dolphin", &["dolphin"], DOLPHIN),
    main("dolphin_baby", &["dolphin_baby"], DOLPHIN),
    main("cod", &["cod"], COD),
    main("salmon", &["salmon"], SALMON),
    main("salmon_small", &["salmon_small", "salmon"], SALMON),
    main("salmon_large", &["salmon_large", "salmon"], SALMON),
    main("tropical_fish_small", &["tropical_fish_a"], TROPICAL_A),
    main(
        "tropical_fish_small_pattern",
        &["tropical_fish_pattern_a"],
        TROPICAL_A,
    ),
    main("tropical_fish_large", &["tropical_fish_b"], TROPICAL_B),
    main(
        "tropical_fish_large_pattern",
        &["tropical_fish_pattern_b"],
        TROPICAL_B,
    ),
    main("pufferfish_small", &["puffer_fish_small"], PUFFER_SMALL),
    main("pufferfish_mid", &["puffer_fish_medium"], PUFFER_MEDIUM),
    main("pufferfish_big", &["puffer_fish_big"], PUFFER_BIG),
    main("frog", &["frog"], FROG),
    main("tadpole", &["tadpole"], TADPOLE),
    main("ghast", &["ghast"], GHAST),
    main("happy_ghast", &["happy_ghast"], GHAST),
    main("happy_ghast_baby", &["happy_ghast_baby"], HAPPY_GHAST_BABY),
    main(
        "happy_ghast_harness",
        &["happy_ghast_harness"],
        HAPPY_GHAST_HARNESS,
    ),
    main(
        "happy_ghast_baby_harness",
        &["happy_ghast_baby_harness"],
        HAPPY_GHAST_HARNESS,
    ),
    main("happy_ghast_ropes", &["happy_ghast_ropes"], GHAST),
    main(
        "happy_ghast_baby_ropes",
        &["happy_ghast_baby_ropes"],
        HAPPY_GHAST_BABY,
    ),
    main("nautilus", &["nautilus"], NAUTILUS),
    main("nautilus_baby", &["nautilus_baby"], NAUTILUS),
    main("nautilus_armor", &["nautilus_armor"], &[]),
    main("zombie_nautilus", &["zombie_nautilus"], NAUTILUS),
    main(
        "zombie_nautilus_coral",
        &["zombie_nautilus_coral"],
        NAUTILUS_CORAL,
    ),
    main("zombie_nautilus_armor", &["nautilus_armor"], &[]),
    main("parrot", &["parrot"], PARROT),
    main("squid", &["squid"], SQUID),
    main("squid_baby", &["squid_baby"], SQUID),
    main("glow_squid", &["glow_squid"], SQUID),
    main("glow_squid_baby", &["glow_squid_baby"], SQUID),
    main("armor_stand", &["armor_stand"], ARMOR_STAND),
    main("armor_stand_small", &["armor_stand_small"], ARMOR_STAND),
    main("elytra", &["elytra"], &[]),
    main("elytra_baby", &["elytra_baby"], &[]),
    main("evoker", &["evoker", "evocation_illager"], ILLAGER),
    main("illusioner", &["illusioner"], ILLAGER),
    main("pillager", &["pillager"], ILLAGER),
    main(
        "vindicator",
        &["vindicator", "vindication_illager"],
        ILLAGER,
    ),
    main("vex", &["vex"], ALLAY),
    main("piglin", &["piglin"], PIGLIN),
    main("piglin_baby", &["piglin_baby"], PIGLIN_BABY),
    main("zombified_piglin", &["zombified_piglin"], PIGLIN),
    main(
        "zombified_piglin_baby",
        &["zombified_piglin_baby"],
        PIGLIN_BABY,
    ),
    main("skeleton", &["skeleton"], BIPED),
    main("wither_skeleton", &["wither_skeleton"], BIPED),
    main("stray", &["stray"], BIPED),
    main("stray_overlay", &["stray_outer"], BIPED),
    main("bogged", &["bogged"], BOGGED),
    main("bogged_overlay", &["bogged_outer"], BIPED),
    main("parched", &["parched"], BIPED),
    main("villager", &["villager"], VILLAGER),
    main("villager_baby", &["villager_baby"], VILLAGER_BABY),
    layer("villager_type", &["villager"], VILLAGER),
    layer(
        "villager_type_no_hat",
        &["villager_no_hat", "villager"],
        VILLAGER,
    ),
    layer("villager_baby_type", &["villager_baby"], VILLAGER_BABY),
    layer(
        "villager_baby_type_no_hat",
        &["villager_baby_no_hat", "villager_baby", "villager"],
        VILLAGER_BABY,
    ),
    layer("villager_profession", &["villager"], VILLAGER),
    layer("villager_profession_level", &["villager"], VILLAGER),
    main("wandering_trader", &["wandering_trader"], VILLAGER),
    main("witch", &["witch"], WITCH),
    main("zombie_villager", &["zombie_villager"], BIPED),
    main("zombie_villager_baby", &["zombie_villager_baby"], BIPED),
    layer("zombie_villager_type", &["zombie_villager"], BIPED),
    layer(
        "zombie_villager_type_no_hat",
        &["zombie_villager_no_hat", "zombie_villager"],
        BIPED,
    ),
    layer(
        "zombie_villager_baby_type",
        &["zombie_villager_baby"],
        BIPED,
    ),
    layer(
        "zombie_villager_baby_type_no_hat",
        &[
            "zombie_villager_baby_no_hat",
            "zombie_villager_baby",
            "zombie_villager",
        ],
        BIPED,
    ),
    layer("zombie_villager_profession", &["zombie_villager"], BIPED),
    layer(
        "zombie_villager_profession_level",
        &["zombie_villager"],
        BIPED,
    ),
    main("zombie", &["zombie"], BIPED),
    main("zombie_baby", &["zombie_baby"], BIPED),
    main("husk", &["husk"], BIPED),
    main("husk_baby", &["husk_baby"], BIPED),
    main("drowned", &["drowned"], BIPED),
    main("drowned_baby", &["drowned_baby"], BIPED),
    main("drowned_outer", &["drowned_outer"], BIPED),
    main("drowned_baby_outer", &["drowned_baby_outer"], BIPED),
    main("giant", &["giant"], BIPED),
    main("blaze", &["blaze"], BLAZE),
    main("breeze", &["breeze"], BREEZE),
    main("breeze_wind", &["breeze_wind"], BREEZE),
    main("breeze_eyes", &["breeze_eyes"], BREEZE),
    main("copper_golem", &["copper_golem"], COPPER_GOLEM),
    layer("copper_golem_eyes", &["copper_golem"], COPPER_GOLEM),
    main("creaking", &["creaking"], &[]),
    layer("creaking_eyes", &["creaking"], &[]),
    main("creeper", &["creeper"], QUADRUPED),
    main("creeper_charge", &["creeper_charge"], QUADRUPED),
    main("ender_dragon", &["dragon"], DRAGON),
    layer("ender_dragon_eyes", &["dragon"], DRAGON),
    main("enderman", &["enderman"], BIPED),
    layer("enderman_eyes", &["enderman"], BIPED),
    main("guardian", &["guardian"], GUARDIAN),
    main("elder_guardian", &["elder_guardian"], GUARDIAN),
    main("iron_golem", &["iron_golem"], IRON_GOLEM),
    layer("iron_golem_crackiness", &["iron_golem"], IRON_GOLEM),
    main("phantom", &["phantom"], PHANTOM),
    layer("phantom_eyes", &["phantom"], PHANTOM),
    main("ravager", &["ravager"], RAVAGER),
    main("shulker", &["shulker"], SHULKER),
    main("silverfish", &["silverfish"], SILVERFISH),
    main("endermite", &["endermite"], ENDERMITE),
    main("slime", &["slime"], SLIME),
    main("slime_outer", &["slime_outer"], SLIME_OUTER),
    main("magma_cube", &["magma_cube"], MAGMA_CUBE),
    main("snow_golem", &["snow_golem"], SNOW_GOLEM),
    main("spider", &["spider"], SPIDER),
    layer("spider_eyes", &["spider"], SPIDER),
    main("cave_spider", &["cave_spider"], SPIDER),
    layer("cave_spider_eyes", &["cave_spider"], SPIDER),
    main("warden", &["warden"], WARDEN),
    main("warden_bioluminescent", &["warden_bioluminescent"], WARDEN),
    main(
        "warden_pulsating_spots_1",
        &["warden_pulsating_spots"],
        WARDEN,
    ),
    main(
        "warden_pulsating_spots_2",
        &["warden_pulsating_spots"],
        WARDEN,
    ),
    main("warden_tendrils", &["warden_tendrils"], WARDEN),
    main("warden_heart", &["warden_heart"], WARDEN),
    main("wither", &["wither"], WITHER),
    main("wither_armor", &["wither_armor"], WITHER),
    main("boat", &["boat"], BOAT),
    main("chest_boat", &["chest_boat"], CHEST_BOAT),
    main("boat_water_patch", &["boat_patch"], BOAT_PATCH),
    main("raft", &["raft"], RAFT),
    main("chest_raft", &["chest_raft"], CHEST_RAFT),
    main("minecart", &["minecart"], MINECART),
    main("end_crystal", &["end_crystal"], END_CRYSTAL),
    main(
        "evoker_fangs",
        &["evoker_fangs", "evocation_fangs"],
        EVOKER_FANGS,
    ),
    main("leash_knot", &["lead_knot"], LEAD_KNOT),
    main("arrow", &["arrow"], ARROW),
    main("spectral_arrow", &["spectral_arrow", "arrow"], ARROW),
    main("trident", &["trident"], TRIDENT),
    main("llama_spit", &["llama_spit"], LLAMA_SPIT),
    main("shulker_bullet", &["shulker_bullet"], SHULKER_BULLET),
    layer("shulker_bullet_glow", &["shulker_bullet"], SHULKER_BULLET),
    main("wind_charge", &["wind_charge"], WIND_CHARGE),
    main("wither_skull", &["wither_skull"], HEAD_ONLY),
];
