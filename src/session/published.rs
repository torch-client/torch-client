use super::*;
use crate::renderer::HumanoidAnim;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ChatTag {
    #[default]
    Secure,
    Modified,
    NotSecure,
    System,
    Error,
}

impl ChatTag {
    pub fn indicator_color(self) -> Option<u32> {
        match self {
            ChatTag::Secure => None,
            ChatTag::Modified => Some(0x606060),
            ChatTag::NotSecure | ChatTag::System => Some(0xD0D0D0),
            ChatTag::Error => Some(0xFF5555),
        }
    }

    pub fn tooltip(self) -> Option<&'static str> {
        match self {
            ChatTag::Secure => Some("Verified message. Signed by the sender:"),
            ChatTag::Modified => Some("Message modified by the server. Original:"),
            ChatTag::NotSecure => Some("Unverified message. Cannot be reported."),
            ChatTag::System => Some("Server message. Cannot be reported."),
            ChatTag::Error => Some("Chat Error"),
        }
    }
}

#[derive(Clone)]
pub struct ChatEntry {
    pub spans: Vec<crate::text::Span>,
    pub events: crate::text::events::Events,
    pub tag: ChatTag,
    pub original: Option<String>,
}

#[derive(Clone, PartialEq)]
pub struct TargetedBlock {
    pub pos: [i32; 3],
    pub boxes: Vec<[f32; 6]>,
}

#[derive(Clone, PartialEq)]
pub struct BreakingBlock {
    pub pos: [i32; 3],
    pub stage: u8,
    pub boxes: Vec<[f32; 6]>,
}

pub struct RemoteBreak {
    pub pos: [i32; 3],
    pub stage: u8,
    pub age: u32,
}

#[derive(Clone)]
pub enum ParticleEmit {
    Block(ParticleSpawn),
    Level(LevelParticle),
    Event(LevelEventEmit),
    Explosion(ExplosionEmit),
    Tracking(TrackingEmit),
}

#[derive(Clone)]
pub struct TrackingEmit {
    pub particle: azalea::entity::particle::Particle,
    pub pos: [f64; 3],
    pub size: [f32; 2],
}

#[derive(Clone, Debug, PartialEq)]
pub struct ItemActivation {
    pub serial: u32,
    pub item: crate::renderer::HeldItem,
}

pub const MAX_EXPLOSION_BLOCK_PARTICLES: usize = 16;

#[derive(Clone)]
pub struct ExplosionEmit {
    pub center: [f64; 3],
    pub radius: f32,
    pub block_count: i32,
    pub particle: azalea::entity::particle::Particle,
    pub block_particles: Vec<
        azalea_protocol::packets::game::c_explode::Weighted<
            azalea_protocol::packets::game::c_explode::ExplosionParticleInfo,
        >,
    >,
}

#[derive(Clone, Copy)]
pub struct LevelEventEmit {
    pub id: u32,
    pub pos: [i32; 3],
    pub data: u32,
    pub global: bool,
}

pub const MAX_PENDING_PARTICLE_EMITS: usize = 256;

pub fn push_particle_emit(queue: &mut Vec<ParticleEmit>, emit: ParticleEmit) {
    push_particle_emits(queue, std::iter::once(emit));
}

pub fn push_particle_emits(
    queue: &mut Vec<ParticleEmit>,
    emits: impl IntoIterator<Item = ParticleEmit>,
) {
    let room = MAX_PENDING_PARTICLE_EMITS.saturating_sub(queue.len());
    queue.extend(emits.into_iter().take(room));
}

#[derive(Clone, Copy)]
pub struct ParticleSpawn {
    pub pos: [i32; 3],
    pub state: azalea::block::BlockState,
    pub mining: bool,
    pub face: [f32; 3],
}

#[derive(Clone)]
pub struct LevelParticle {
    pub particle: azalea::entity::particle::Particle,
    pub pos: [f64; 3],
    pub dist: [f32; 3],
    pub max_speed: f32,
    pub count: u32,
    pub override_limiter: bool,
    pub always_show: bool,
}

#[derive(Clone)]
pub struct MapUpdate {
    pub id: i32,
    pub colors: std::sync::Arc<Vec<u8>>,
    pub decorations: Option<std::sync::Arc<Vec<MapDecoration>>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MapDecoration {
    pub kind: u8,
    pub x: i8,
    pub y: i8,
    pub rot: u8,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SlotStack {
    pub item: &'static str,
    pub count: u8,
    pub cooldown: u8,
    pub damage: u16,
    pub max_damage: u16,
    pub custom_name: Option<Vec<crate::text::Span>>,
    pub lore: Vec<Vec<crate::text::Span>>,
    pub enchantments: Vec<(String, i32)>,
    pub potion: Option<crate::items::potions::PotionContents>,
    pub unbreakable: bool,
    pub glint: bool,
    pub charged: bool,
    pub death_protection: bool,
    pub kinetic: Option<crate::items::kinetic::Kinetic>,
    pub swing_ticks: u8,
    pub map_id: Option<i32>,
    pub component_count: u16,
    pub nbt_bytes: usize,
    pub banner_patterns: Option<Box<[Box<str>]>>,
    pub book: Option<Box<Book>>,
    pub banner_layers: Option<Box<[crate::blockentities::banner::BannerLayer]>>,
    pub raw: Option<std::sync::Arc<azalea_inventory::ItemStack>>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Book {
    pub pages: Box<[Vec<crate::text::Span>]>,
    pub title: String,
    pub author: String,
    pub generation: u8,
    pub signed: bool,
}

impl Book {
    pub fn raw_pages(&self) -> Vec<String> {
        self.pages
            .iter()
            .map(|spans| spans.iter().map(|s| s.text.as_str()).collect())
            .collect()
    }
}

impl SlotStack {
    pub fn is_empty(&self) -> bool {
        self.item.is_empty() || self.count == 0
    }

    pub fn model_key(&self) -> std::borrow::Cow<'_, str> {
        use crate::items::potions;
        use std::borrow::Cow;

        if let Some(layers) = &self.banner_layers
            && !layers.is_empty()
        {
            return Cow::Owned(crate::blockentities::banner::model_key(self.item, layers));
        }

        match &self.potion {
            Some(contents) if potions::is_potion_item(self.item) => {
                Cow::Owned(potions::tint_key(self.item, contents.color()))
            }
            _ => Cow::Borrowed(self.item),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SkinPrefs {
    pub main_hand_left: bool,
    #[cfg(feature = "skins")]
    pub skin_parts: u8,
}

impl Default for SkinPrefs {
    fn default() -> Self {
        Self {
            main_hand_left: false,
            #[cfg(feature = "skins")]
            skin_parts: crate::client::skins::ALL_PARTS,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct PlayerBody {
    pub invisible: bool,
    pub team_sees: bool,
    pub on_fire: bool,
    pub scale: f32,
    pub bounding_box: [f32; 2],
    pub eye_height: f32,
}

impl PlayerBody {
    pub const STANDING: PlayerBody = PlayerBody {
        invisible: false,
        team_sees: false,
        on_fire: false,
        scale: 1.0,
        bounding_box: [0.6, 1.8],
        eye_height: 1.62,
    };

    pub fn hidden_to(&self, viewer: Gamemode) -> bool {
        invisible_to(self.invisible && !self.team_sees, viewer)
    }
}

impl Default for PlayerBody {
    fn default() -> Self {
        Self::STANDING
    }
}

pub fn player_dimensions(
    pose: azalea::entity::Pose,
    scale: f32,
) -> azalea::entity::dimensions::EntityDimensions {
    let d = azalea::entity::dimensions::calculate_dimensions(
        azalea_registry::builtin::EntityKind::Player,
        pose,
    );
    let s = if d.fixed { 1.0 } else { scale };
    azalea::entity::dimensions::EntityDimensions {
        width: d.width * s,
        height: d.height * s,
        eye_height: d.eye_height * s,
        fixed: d.fixed,
    }
}

pub fn invisible_to(invisible: bool, viewer: Gamemode) -> bool {
    invisible && viewer != Gamemode::Spectator
}

#[derive(Clone, Default)]
pub struct OtherPlayerInfo {
    pub id: i32,
    pub username: String,
    pub name_tag: Vec<crate::text::Span>,
    pub hidden_by_team: bool,
    pub health: Option<f32>,
    pub max_health: f32,
    pub body: PlayerBody,
    pub gamemode: Option<Gamemode>,
    pub below_name: Vec<crate::text::Span>,
    pub discrete: bool,
    #[cfg(feature = "skins")]
    pub skin: crate::client::skins::SkinState,
    pub anim: HumanoidAnim,
}

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum ContainerKind {
    #[default]
    None,
    Chest(u8),
    Crafting,
    Furnace(FurnaceKind),
    Hopper,
    ShulkerBox,
    Dispenser,
    Crafter,
    Grindstone,
    Enchantment,
    Loom,
    Stonecutter,
    Cartography,
    Smithing,
    Beacon,
    BrewingStand,
    Merchant,
    Lectern,
    Horse(u8),
    Anvil,
}

impl ContainerKind {
    pub fn is_some(self) -> bool {
        self != ContainerKind::None
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FurnaceKind {
    Furnace,
    BlastFurnace,
    Smoker,
}

impl FurnaceKind {
    pub fn texture_key(self) -> &'static str {
        match self {
            FurnaceKind::Furnace => "furnace",
            FurnaceKind::BlastFurnace => "blast_furnace",
            FurnaceKind::Smoker => "smoker",
        }
    }
}

pub type ContainerData = [i16; CONTAINER_DATA_LEN];

pub const CONTAINER_DATA_LEN: usize = 10;

#[derive(Clone, Default)]
pub struct MerchantOffers {
    pub container_id: i32,
    pub offers: Vec<MerchantOffer>,
    pub level: u32,
    pub xp: u32,
    pub show_progress: bool,
    pub can_restock: bool,
}

impl MerchantOffers {
    pub fn for_window(
        offers: &Option<std::sync::Arc<MerchantOffers>>,
        container_id: i32,
    ) -> Option<&std::sync::Arc<MerchantOffers>> {
        offers.as_ref().filter(|o| o.container_id == container_id)
    }

    pub fn future_xp(&self, buy_a: &SlotStack, buy_b: &SlotStack, hint: i32) -> u32 {
        self.active_offer(buy_a, buy_b, hint)
            .map_or(0, |i| self.offers[i].xp)
    }

    pub fn active_offer(&self, buy_a: &SlotStack, buy_b: &SlotStack, hint: i32) -> Option<usize> {
        let empty = SlotStack::default();
        let (buy_a, buy_b) = if buy_a.is_empty() {
            (buy_b, &empty)
        } else {
            (buy_a, buy_b)
        };
        self.recipe_for(buy_a, buy_b, hint)
            .filter(|i| !self.offers[*i].out_of_stock)
            .or_else(|| self.recipe_for(buy_b, buy_a, hint))
            .filter(|i| !self.offers[*i].out_of_stock)
    }

    pub fn notify_trade(&mut self, index: usize) {
        let Some(offer) = self.offers.get_mut(index) else {
            return;
        };
        offer.uses = offer.uses.saturating_add(1);
        offer.out_of_stock = offer.uses >= offer.max_uses;
        self.xp = self.xp.saturating_add(offer.xp);
    }

    pub fn quick_move_trades(&mut self, slots: &[SlotStack], hint: i32) -> u32 {
        let Some([buy_a, buy_b, result]) = slots.get(..3) else {
            return 0;
        };
        let (mut buy_a, mut buy_b, mut result) = (buy_a.clone(), buy_b.clone(), result.clone());
        let mut inventory: Vec<SlotStack> = slots.get(3..39).map_or_else(Vec::new, <[_]>::to_vec);
        let max_stack =
            |s: &SlotStack| crate::generated_items::item(s.item).map_or(64, |d| d.max_stack.max(1));
        let mut trades = 0;
        for _ in 0..256 {
            if result.is_empty() {
                break;
            }
            let before = result.count;
            let max = max_stack(&result);
            for slot in inventory.iter_mut().rev() {
                if result.count == 0 {
                    break;
                }
                if slot.item == result.item && slot.potion == result.potion && slot.count < max {
                    let moved = (max - slot.count).min(result.count);
                    slot.count += moved;
                    result.count -= moved;
                }
            }
            if result.count > 0
                && let Some(slot) = inventory.iter_mut().rev().find(|s| s.is_empty())
            {
                let moved = result.count.min(max);
                *slot = SlotStack {
                    count: moved,
                    ..result.clone()
                };
                result.count -= moved;
            }
            if result.count == before {
                break;
            }
            let Some(i) = self.active_offer(&buy_a, &buy_b, hint) else {
                break;
            };
            let offer = &self.offers[i];
            let (cost_a, cost_b) = (offer.cost_a.count, offer.cost_b.count);
            if offer.satisfied_by(&buy_a, &buy_b) {
                buy_a.count -= cost_a;
                buy_b.count = buy_b.count.saturating_sub(cost_b);
            } else if offer.satisfied_by(&buy_b, &buy_a) {
                buy_b.count -= cost_a;
                buy_a.count = buy_a.count.saturating_sub(cost_b);
            } else {
                break;
            }
            for slot in [&mut buy_a, &mut buy_b] {
                if slot.count == 0 {
                    *slot = SlotStack::default();
                }
            }
            self.notify_trade(i);
            trades += 1;
            if result.count > 0 {
                break;
            }
            result = self
                .active_offer(&buy_a, &buy_b, hint)
                .map_or_else(SlotStack::default, |i| self.offers[i].result.clone());
        }
        trades
    }

    fn recipe_for(&self, buy_a: &SlotStack, buy_b: &SlotStack, hint: i32) -> Option<usize> {
        if hint > 0 && (hint as usize) < self.offers.len() {
            let i = hint as usize;
            return self.offers[i].satisfied_by(buy_a, buy_b).then_some(i);
        }
        self.offers
            .iter()
            .position(|o| o.satisfied_by(buy_a, buy_b))
    }
}

pub fn empty_spans() -> std::sync::Arc<[crate::text::Span]> {
    EMPTY_SPANS.clone()
}

static EMPTY_SPANS: std::sync::LazyLock<std::sync::Arc<[crate::text::Span]>> =
    std::sync::LazyLock::new(|| std::sync::Arc::from(Vec::new()));

#[derive(Clone, Default)]
pub struct MerchantOffer {
    pub cost_a: SlotStack,
    pub base_cost_a: SlotStack,
    pub cost_b: SlotStack,
    pub result: SlotStack,
    pub out_of_stock: bool,
    pub uses: u32,
    pub max_uses: u32,
    pub xp: u32,
}

impl MerchantOffer {
    fn satisfied_by(&self, buy_a: &SlotStack, buy_b: &SlotStack) -> bool {
        if self.cost_a.item != buy_a.item || buy_a.count < self.cost_a.count {
            return false;
        }
        if self.cost_b.is_empty() {
            buy_b.is_empty()
        } else {
            self.cost_b.item == buy_b.item && buy_b.count >= self.cost_b.count
        }
    }
}

#[derive(Clone, Default)]
pub struct HorseMenu {
    pub container_id: i32,
    pub columns: u8,
    pub state_id: u32,
    pub slots: Vec<SlotStack>,
}

impl HorseMenu {
    pub fn player_slots_start(&self) -> usize {
        2 + self.columns as usize * 3
    }

    pub fn player_slots(&self) -> &[SlotStack] {
        let start = self.player_slots_start();
        self.slots.get(start..).unwrap_or(&[])
    }
}

pub struct StonecutterRecipe {
    pub input: Box<[Box<str>]>,
    pub result: SlotStack,
}

#[derive(Clone, Copy, Default)]
pub struct HealthDisplay {
    pub max_health: f32,
    pub absorption: f32,
    pub armor: u8,
    pub kind: HeartKind,
    pub regenerating: bool,
    pub hunger_effect: bool,
    pub ticks_frozen: i32,
    pub vehicle: Option<(f32, f32)>,
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum HeartKind {
    #[default]
    Normal,
    Poisoned,
    Withered,
    Frozen,
}

#[derive(Clone, Copy)]
pub struct MineIntent {
    pub aim: Option<[f64; 3]>,
    pub forward: bool,
    pub turn: f32,
}

#[derive(Clone, Copy)]
pub struct MaceAim {
    pub aim: [f64; 3],
    pub turn: f32,
}

pub const GAME_MASTER_LEVEL: u8 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Gamemode {
    #[default]
    Survival,
    Creative,
    Adventure,
    Spectator,
}

impl Gamemode {
    #[cfg(feature = "shader_support")]
    pub fn is_survival(self) -> bool {
        matches!(self, Gamemode::Survival | Gamemode::Adventure)
    }

    pub fn from_azalea(mode: azalea_core::game_type::GameMode) -> Gamemode {
        match mode {
            azalea_core::game_type::GameMode::Survival => Gamemode::Survival,
            azalea_core::game_type::GameMode::Creative => Gamemode::Creative,
            azalea_core::game_type::GameMode::Adventure => Gamemode::Adventure,
            azalea_core::game_type::GameMode::Spectator => Gamemode::Spectator,
        }
    }

    pub fn to_azalea(self) -> azalea_core::game_type::GameMode {
        match self {
            Gamemode::Survival => azalea_core::game_type::GameMode::Survival,
            Gamemode::Creative => azalea_core::game_type::GameMode::Creative,
            Gamemode::Adventure => azalea_core::game_type::GameMode::Adventure,
            Gamemode::Spectator => azalea_core::game_type::GameMode::Spectator,
        }
    }
}

#[cfg(test)]
mod merchant_tests {
    use super::*;

    fn cost(item: &'static str, count: u8) -> SlotStack {
        SlotStack {
            item,
            count,
            ..SlotStack::default()
        }
    }

    fn offer(a: SlotStack, b: SlotStack, xp: u32) -> MerchantOffer {
        MerchantOffer {
            cost_a: a,
            cost_b: b,
            xp,
            ..MerchantOffer::default()
        }
    }

    fn offers(list: Vec<MerchantOffer>) -> MerchantOffers {
        MerchantOffers {
            offers: list,
            ..MerchantOffers::default()
        }
    }

    fn redstone_trade(emeralds: u8) -> (MerchantOffers, Vec<SlotStack>) {
        let mut o = offer(cost("emerald", 1), SlotStack::default(), 1);
        o.result = cost("redstone", 2);
        o.max_uses = 12;
        let mut slots = vec![SlotStack::default(); 39];
        slots[0] = cost("emerald", emeralds);
        slots[2] = cost("redstone", 2);
        (offers(vec![o]), slots)
    }

    #[test]
    fn shift_click_trades_until_the_stock_runs_out() {
        let (mut list, slots) = redstone_trade(40);
        assert_eq!(list.quick_move_trades(&slots, -1), 12);
        assert_eq!(list.offers[0].uses, 12);
        assert!(list.offers[0].out_of_stock);
        assert_eq!(list.xp, 12);
    }

    #[test]
    fn shift_click_trades_until_the_payment_runs_out() {
        let (mut list, slots) = redstone_trade(5);
        assert_eq!(list.quick_move_trades(&slots, -1), 5);
        assert!(!list.offers[0].out_of_stock);
    }

    #[test]
    fn shift_click_into_a_full_inventory_trades_nothing() {
        let (mut list, mut slots) = redstone_trade(40);
        for slot in &mut slots[3..39] {
            *slot = cost("dirt", 64);
        }
        assert_eq!(list.quick_move_trades(&slots, -1), 0);
        assert_eq!(list.offers[0].uses, 0);
    }

    #[test]
    fn a_trade_is_satisfied_by_its_price() {
        let o = offer(cost("emerald", 3), SlotStack::default(), 2);
        assert!(o.satisfied_by(&cost("emerald", 3), &SlotStack::default()));
        assert!(o.satisfied_by(&cost("emerald", 9), &SlotStack::default()));
        assert!(!o.satisfied_by(&cost("emerald", 2), &SlotStack::default()));
        assert!(!o.satisfied_by(&cost("diamond", 9), &SlotStack::default()));
        assert!(!o.satisfied_by(&cost("emerald", 3), &cost("book", 1)));

        let two = offer(cost("emerald", 3), cost("book", 1), 5);
        assert!(two.satisfied_by(&cost("emerald", 3), &cost("book", 1)));
        assert!(!two.satisfied_by(&cost("emerald", 3), &SlotStack::default()));
    }

    #[test]
    fn future_xp_finds_the_trade_whichever_slot_was_filled() {
        let list = offers(vec![offer(cost("emerald", 3), cost("book", 1), 5)]);
        let (em, bk) = (cost("emerald", 3), cost("book", 1));
        assert_eq!(list.future_xp(&em, &bk, -1), 5);
        assert_eq!(list.future_xp(&bk, &em, -1), 5);

        let single = offers(vec![offer(cost("emerald", 1), SlotStack::default(), 1)]);
        assert_eq!(
            single.future_xp(&SlotStack::default(), &cost("emerald", 1), -1),
            1
        );
        assert_eq!(
            single.future_xp(&SlotStack::default(), &SlotStack::default(), -1),
            0
        );
    }

    #[test]
    fn taking_a_trade_uses_it_up_and_pays_the_trader() {
        let mut list = offers(vec![MerchantOffer {
            cost_a: cost("emerald", 1),
            max_uses: 2,
            xp: 3,
            ..MerchantOffer::default()
        }]);
        list.notify_trade(0);
        assert_eq!(list.offers[0].uses, 1);
        assert!(!list.offers[0].out_of_stock);
        assert_eq!(list.xp, 3);
        list.notify_trade(0);
        assert!(list.offers[0].out_of_stock);
        assert_eq!(list.xp, 6);
        assert_eq!(
            list.active_offer(&cost("emerald", 1), &SlotStack::default(), -1),
            None
        );
        list.notify_trade(9);
        assert_eq!(list.xp, 6);
    }

    #[test]
    fn future_xp_honours_stock_and_the_selection_hint() {
        let mut sold_out = offer(cost("emerald", 1), SlotStack::default(), 4);
        sold_out.out_of_stock = true;
        let list = offers(vec![
            sold_out,
            offer(cost("emerald", 1), SlotStack::default(), 7),
        ]);
        let em = cost("emerald", 1);
        assert_eq!(list.future_xp(&em, &SlotStack::default(), 0), 0);
        assert_eq!(list.future_xp(&em, &SlotStack::default(), 1), 7);
    }
}
