use azalea_core::math;
use azalea_core::position::Vec3;
use azalea_registry::builtin::{EntityKind, ItemKind};

use crate::util::mth::{DEG_TO_RAD, wrap_degrees};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Mount {
    Horse,
    Camel,
    Pig,
    Strider,
    Nautilus,
    HappyGhast,
    Boat(Hull),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Hull {
    Boat,
    ChestBoat,
    Raft,
    ChestRaft,
}

impl Hull {
    pub(crate) fn of(kind: EntityKind) -> Option<Hull> {
        use EntityKind as K;
        Some(match kind {
            K::OakBoat
            | K::SpruceBoat
            | K::BirchBoat
            | K::JungleBoat
            | K::AcaciaBoat
            | K::CherryBoat
            | K::DarkOakBoat
            | K::PaleOakBoat
            | K::MangroveBoat => Hull::Boat,
            K::OakChestBoat
            | K::SpruceChestBoat
            | K::BirchChestBoat
            | K::JungleChestBoat
            | K::AcaciaChestBoat
            | K::CherryChestBoat
            | K::DarkOakChestBoat
            | K::PaleOakChestBoat
            | K::MangroveChestBoat => Hull::ChestBoat,
            K::BambooRaft => Hull::Raft,
            K::BambooChestRaft => Hull::ChestRaft,
            _ => return None,
        })
    }

    fn ride_height(self, height: f64) -> f64 {
        match self {
            Hull::Boat | Hull::ChestBoat => height / 3.0,
            Hull::Raft | Hull::ChestRaft => height * 0.8888889,
        }
    }

    fn single_offset(self) -> f64 {
        match self {
            Hull::Boat | Hull::Raft => 0.0,
            Hull::ChestBoat | Hull::ChestRaft => 0.15,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Jump {
    None,
    Leap,
    Dash { cooldown: i32 },
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Rider {
    pub xxa: f32,
    pub zza: f32,
    pub x_rot: f32,
    pub jumping: bool,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Gear {
    pub saddled: bool,
    pub harnessed: bool,
    pub still: bool,
    pub held: [ItemKind; 2],
}

impl Mount {
    pub(crate) fn of(kind: EntityKind) -> Option<Mount> {
        use EntityKind as K;
        Some(match kind {
            K::Horse | K::Donkey | K::Mule | K::SkeletonHorse | K::ZombieHorse => Mount::Horse,
            K::Camel | K::CamelHusk => Mount::Camel,
            K::Pig => Mount::Pig,
            K::Strider => Mount::Strider,
            K::Nautilus | K::ZombieNautilus => Mount::Nautilus,
            K::HappyGhast => Mount::HappyGhast,
            _ => Mount::Boat(Hull::of(kind)?),
        })
    }

    pub(crate) fn steered(self, gear: &Gear) -> bool {
        let holding = |item| gear.held.contains(&item);
        match self {
            Mount::Horse | Mount::Camel | Mount::Nautilus => gear.saddled,
            Mount::Pig => gear.saddled && holding(ItemKind::CarrotOnAStick),
            Mount::Strider => gear.saddled && holding(ItemKind::WarpedFungusOnAStick),
            Mount::HappyGhast => gear.harnessed && !gear.still,
            Mount::Boat(_) => true,
        }
    }

    pub(crate) fn jump(self) -> Jump {
        match self {
            Mount::Horse => Jump::Leap,
            Mount::Camel => Jump::Dash { cooldown: 55 },
            Mount::Nautilus => Jump::Dash { cooldown: 40 },
            _ => Jump::None,
        }
    }

    pub(crate) fn ridden_input(
        self,
        rider: Rider,
        refuse: bool,
        rearing_idle: bool,
        flying_speed: f64,
    ) -> Vec3 {
        match self {
            Mount::Horse | Mount::Camel => {
                if refuse || rearing_idle {
                    return Vec3::ZERO;
                }
                let forward = if rider.zza <= 0.0 {
                    rider.zza * 0.25
                } else {
                    rider.zza
                };
                Vec3::new((rider.xxa * 0.5) as f64, 0.0, forward as f64)
            }
            Mount::Pig | Mount::Strider => Vec3::new(0.0, 0.0, 1.0),
            Mount::Nautilus => look_input(rider),
            Mount::HappyGhast => {
                let mut input = look_input(rider);
                if rider.jumping {
                    input.y += 0.5;
                }
                input * (3.9000000953674316 * flying_speed)
            }
            Mount::Boat(_) => Vec3::ZERO,
        }
    }

    pub(crate) fn ridden_rotation(
        self,
        mount: (f32, f32),
        rider: (f32, f32),
        refuse: bool,
    ) -> (f32, f32) {
        let ease = |factor: f32| {
            (
                mount.0 + wrap_degrees(rider.0 - mount.0) * factor,
                rider.1 * 0.5,
            )
        };
        match self {
            Mount::Camel if refuse => mount,
            Mount::Horse | Mount::Camel | Mount::Pig | Mount::Strider => (rider.0, rider.1 * 0.5),
            Mount::Nautilus => ease(0.5),
            Mount::HappyGhast => ease(0.08),
            Mount::Boat(_) => mount,
        }
    }

    pub(crate) fn ridden_speed(
        self,
        movement_speed: f64,
        boost: f32,
        sprint_bonus: bool,
        suffocating: bool,
        in_water: bool,
    ) -> f64 {
        match self {
            Mount::Horse | Mount::HappyGhast | Mount::Boat(_) => movement_speed,
            Mount::Camel => movement_speed + if sprint_bonus { 0.1 } else { 0.0 },
            Mount::Pig => movement_speed * 0.225 * boost as f64,
            Mount::Strider => movement_speed * if suffocating { 0.35 } else { 0.55 } * boost as f64,
            Mount::Nautilus => movement_speed * if in_water { 0.0325 } else { 0.02 },
        }
    }

    pub(crate) fn can_sprint(self) -> bool {
        self == Mount::Camel
    }

    pub(crate) fn floats_while_ridden(self) -> bool {
        matches!(self, Mount::Horse | Mount::Camel)
    }

    pub(crate) fn pushed_by_fluid(self) -> bool {
        self != Mount::Nautilus
    }
}

fn look_input(rider: Rider) -> Vec3 {
    let (mut forward, mut up) = (0.0, 0.0);
    if rider.zza != 0.0 {
        let pitch = rider.x_rot * DEG_TO_RAD;
        forward = math::cos(pitch);
        up = -math::sin(pitch);
        if rider.zza < 0.0 {
            forward *= -0.5;
            up *= -0.5;
        }
    }
    Vec3::new(rider.xxa as f64, up as f64, forward as f64)
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct SeatPose {
    pub stand: f32,
    pub camel_sitting: bool,
    pub camel_pose_time: i64,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Passenger {
    pub kind: EntityKind,
    pub animal: bool,
    pub index: usize,
    pub of: usize,
}

pub(crate) fn passenger_attachment(
    kind: EntityKind,
    yaw: f32,
    scale: f64,
    p: Passenger,
    pose: SeatPose,
) -> Vec3 {
    use EntityKind as K;

    let (index, riders) = (p.index, p.of);
    let animal = if riders > 1 && p.animal { 0.2 } else { 0.0 };
    let height = azalea::entity::dimensions::EntityDimensions::from(kind).height as f64;
    let mut seat = match kind {
        K::Cat => Vec3::new(0.0, 0.5125, 0.0),
        K::Chicken => Vec3::new(0.0, 0.7, -0.1),
        K::Cow | K::Mooshroom => Vec3::new(0.0, 1.36875, 0.0),
        K::Zombie | K::Drowned | K::Piglin | K::PiglinBrute => Vec3::new(0.0, 2.0125, 0.0),
        K::Husk => Vec3::new(0.0, 2.075, 0.0),
        K::ZombieVillager => Vec3::new(0.0, 2.125, 0.0),
        K::ZombifiedPiglin | K::Evoker | K::Illusioner | K::Pillager | K::Vindicator => {
            Vec3::new(0.0, 2.0, 0.0)
        }
        K::ElderGuardian => Vec3::new(0.0, 2.350625, 0.0),
        K::Guardian => Vec3::new(0.0, 0.975, 0.0),
        K::Enderman => Vec3::new(0.0, 2.80625, 0.0),
        K::Endermite | K::Silverfish => Vec3::new(0.0, 0.2375, 0.0),
        K::EnderDragon => Vec3::new(0.0, 3.0, 0.0),
        K::Fox => Vec3::new(0.0, 0.6375, -0.25),
        K::Frog => Vec3::new(0.0, 0.375, -0.25),
        K::Turtle => Vec3::new(0.0, 0.55625, -0.25),
        K::Ghast => Vec3::new(0.0, 4.0625, 0.0),
        K::Goat => Vec3::new(0.0, 1.1125, 0.0),
        K::Hoglin | K::Zoglin => Vec3::new(0.0, 1.49375, 0.0),
        K::Ocelot => Vec3::new(0.0, 0.6375, 0.0),
        K::Parrot => Vec3::new(0.0, 0.4625, 0.0),
        K::Phantom => Vec3::new(0.0, 0.3375, 0.0),
        K::Ravager => Vec3::new(0.0, 2.2625, -0.0625),
        K::Sheep => Vec3::new(0.0, 1.2375, 0.0),
        K::Sniffer => Vec3::new(0.0, 2.09375, 0.0),
        K::Spider => Vec3::new(0.0, 0.765, 0.0),
        K::Vex => Vec3::new(0.0, 0.7375, 0.0),
        K::Warden => Vec3::new(0.0, 3.15, 0.0),
        K::Witch => Vec3::new(0.0, 2.2625, 0.0),
        K::Wolf => Vec3::new(0.0, 0.81875, -0.0625),
        K::Horse => Vec3::new(0.0, 1.44375, 0.0),
        K::SkeletonHorse | K::ZombieHorse => Vec3::new(0.0, 1.31875, 0.0),
        K::Donkey => Vec3::new(0.0, 1.1125, 0.0),
        K::Mule => Vec3::new(0.0, 1.2125, 0.0),
        K::Llama | K::TraderLlama => Vec3::new(0.0, 1.37, -0.3),
        K::Pig => Vec3::new(0.0, 0.86875, 0.0),
        K::Nautilus | K::ZombieNautilus => Vec3::new(0.0, 1.1375, 0.0),
        K::HappyGhast => {
            const SEATS: [(f64, f64); 4] = [(0.0, 1.7), (-1.7, 0.0), (0.0, -1.7), (1.7, 0.0)];
            let (x, z) = SEATS[index.min(SEATS.len() - 1)];
            Vec3::new(x, 4.0, z)
        }
        K::Camel | K::CamelHusk => Vec3::new(
            0.0,
            camel_body_anchor(index == 0, height, pose),
            (if riders > 1 && index > 0 { -0.7 } else { 0.5 }) + animal,
        ),
        K::Minecart
        | K::ChestMinecart
        | K::FurnaceMinecart
        | K::TntMinecart
        | K::HopperMinecart
        | K::SpawnerMinecart
        | K::CommandBlockMinecart => match p.kind {
            K::Villager | K::WanderingTrader => Vec3::ZERO,
            _ => Vec3::new(0.0, 0.1875, 0.0),
        },
        _ => match Hull::of(kind) {
            Some(hull) => {
                let z = match (riders, index) {
                    (0 | 1, _) => hull.single_offset(),
                    (_, 0) => 0.2,
                    _ => -0.6,
                };
                Vec3::new(0.0, hull.ride_height(height), z + animal)
            }
            None => Vec3::new(0.0, height, 0.0),
        },
    };
    if Mount::of(kind) == Some(Mount::Horse) && pose.stand > 0.0 {
        seat.y += 0.15 * pose.stand as f64;
        seat.z -= 0.7 * pose.stand as f64;
    }
    (seat * scale).y_rot(-yaw * DEG_TO_RAD)
}

pub(crate) fn vehicle_attachment(
    rider: EntityKind,
    scale: f64,
    width: f64,
    vehicle_width: f64,
) -> f64 {
    use EntityKind as K;

    let y = match rider {
        K::Player | K::Mannequin => 0.6,
        K::Spider if vehicle_width <= width => 0.3125,
        K::CaveSpider if vehicle_width <= width => 0.21875,
        K::Zombie
        | K::Husk
        | K::Drowned
        | K::ZombieVillager
        | K::ZombifiedPiglin
        | K::Piglin
        | K::PiglinBrute
        | K::Skeleton
        | K::Stray
        | K::Bogged
        | K::Parched => 0.7,
        K::WitherSkeleton => 0.875,
        K::Evoker | K::Illusioner | K::Pillager | K::Vindicator => 0.6,
        K::Phantom => 0.125,
        K::Giant => 3.75,
        K::Allay | K::Vex => -0.04,
        K::Ghast | K::HappyGhast => -0.5,
        _ => 0.0,
    };
    y * scale
}

fn camel_body_anchor(front: bool, height: f64, pose: SeatPose) -> f64 {
    const SITTING_DIFFERENCE: f32 = 1.43;
    const VERTICAL_DROP: f32 = SITTING_DIFFERENCE - 0.2;
    const BOTTOM: f32 = SITTING_DIFFERENCE - VERTICAL_DROP;

    let height = if pose.camel_sitting {
        height - SITTING_DIFFERENCE as f64
    } else {
        height
    };
    let mut offset = height - 0.375;
    let duration = if pose.camel_sitting { 40 } else { 52 };
    let in_transition = pose.camel_pose_time < duration;
    if in_transition {
        let (half, flex_offset) = match (pose.camel_sitting, front) {
            (true, true) => (28, 0.5),
            (true, false) => (28, 0.1),
            (false, true) => (24, 0.6),
            (false, false) => (32, 0.35),
        };
        let time = (pose.camel_pose_time as f32).clamp(0.0, duration as f32);
        let first = time < half as f32;
        let part = if first {
            time / half as f32
        } else {
            (time - half as f32) / (duration - half) as f32
        };
        let flex = SITTING_DIFFERENCE - flex_offset * VERTICAL_DROP;
        let lerp = |a: f32, b: f32| a + part * (b - a);
        offset += if pose.camel_sitting {
            if first {
                lerp(SITTING_DIFFERENCE, flex)
            } else {
                lerp(flex, BOTTOM)
            }
        } else if first {
            lerp(BOTTOM - SITTING_DIFFERENCE, BOTTOM - flex)
        } else {
            lerp(BOTTOM - flex, 0.0)
        } as f64;
    }
    if pose.camel_sitting && !in_transition {
        offset += BOTTOM as f64;
    }
    offset
}

pub(crate) fn default_movement_speed(kind: EntityKind) -> f64 {
    use EntityKind as K;

    match kind {
        K::Camel | K::CamelHusk => 0.09000000357627869,
        K::Pig => 0.25,
        K::Strider => 0.17499999701976776,
        K::Nautilus => 1.0,
        K::ZombieNautilus => 1.100000023841858,
        _ => 0.22499999403953552,
    }
}

pub(crate) const DEFAULT_FLYING_SPEED: f64 = 0.05;

pub(crate) fn default_jump_strength(mount: Mount) -> f64 {
    match mount {
        Mount::Camel => 0.41999998688697815,
        _ => 0.7,
    }
}

pub(crate) fn default_step_height(mount: Mount) -> f64 {
    match mount {
        Mount::Boat(_) => 0.0,
        Mount::Camel => 1.5,
        Mount::Horse => 1.0,
        _ => 0.6,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rider(xxa: f32, zza: f32, x_rot: f32) -> Rider {
        Rider {
            xxa,
            zza,
            x_rot,
            jumping: false,
        }
    }

    #[test]
    fn a_llama_is_never_steered() {
        assert_eq!(Mount::of(EntityKind::Llama), None);
        assert_eq!(Mount::of(EntityKind::CamelHusk), Some(Mount::Camel));
        assert_eq!(
            Mount::of(EntityKind::BambooChestRaft),
            Some(Mount::Boat(Hull::ChestRaft))
        );
    }

    #[test]
    fn the_stick_counts_in_the_offhand() {
        let gear = Gear {
            saddled: true,
            harnessed: false,
            still: false,
            held: [ItemKind::Air, ItemKind::CarrotOnAStick],
        };
        assert!(Mount::Pig.steered(&gear));
        assert!(!Mount::Strider.steered(&gear));
    }

    #[test]
    fn a_horse_backs_up_at_a_quarter_pace() {
        let input = Mount::Horse.ridden_input(rider(1.0, -1.0, 0.0), false, false, 0.0);
        assert_eq!((input.x, input.z), (0.5, -0.25));
        assert_eq!(
            Mount::Camel.ridden_input(rider(1.0, 1.0, 0.0), true, false, 0.0),
            Vec3::ZERO
        );
    }

    #[test]
    fn the_nautilus_swims_where_the_rider_looks() {
        let down = Mount::Nautilus.ridden_input(rider(0.0, 1.0, 90.0), false, false, 0.0);
        assert!(down.z.abs() < 1e-6 && (down.y + 1.0).abs() < 1e-6);
        let back = Mount::Nautilus.ridden_input(rider(0.0, -1.0, 0.0), false, false, 0.0);
        assert!((back.z + 0.5).abs() < 1e-6);
    }

    #[test]
    fn the_ghast_eases_through_the_short_way_round() {
        let (yaw, pitch) = Mount::HappyGhast.ridden_rotation((170.0, 0.0), (-170.0, 20.0), false);
        assert!((yaw - 171.6).abs() < 1e-4);
        assert_eq!(pitch, 10.0);
    }

    #[test]
    fn a_seated_camel_lowers_the_seat() {
        let stand = SeatPose {
            camel_pose_time: 1000,
            ..SeatPose::default()
        };
        let sit = SeatPose {
            camel_sitting: true,
            ..stand
        };
        let h = 2.375;
        assert!((camel_body_anchor(true, h, stand) - 2.0).abs() < 1e-6);
        assert!((camel_body_anchor(true, h, sit) - 0.77).abs() < 1e-5);
    }
}
