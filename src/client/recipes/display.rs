use std::collections::HashMap;
use std::sync::Arc;

use azalea_core::registry_holder::RegistryHolder;
use azalea_inventory::ItemStack;
use azalea_protocol::common::recipe::{RecipeDisplayData, SlotDisplayData};
use azalea_registry::builtin::{DataComponentKind, ItemKind, Potion};
use azalea_registry::identifier::Identifier;

use crate::session::SlotStack;

pub type Stacks = Arc<[SlotStack]>;

pub fn cycle(stacks: &[SlotStack], index: usize) -> Option<&SlotStack> {
    (!stacks.is_empty()).then(|| &stacks[index % stacks.len()])
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    Shaped { width: u8, height: u8 },
    Shapeless,
    Furnace,
    Stonecutter,
    Smithing,
}

#[derive(Clone, Debug)]
pub struct ResolvedDisplay {
    pub shape: Shape,
    pub ingredients: Arc<[Stacks]>,
    pub result: Stacks,
    pub station: Stacks,
}

pub struct Ctx<'a> {
    registries: Option<&'a RegistryHolder>,
    tags: &'a HashMap<Identifier, Box<[ItemKind]>>,
    by_item: HashMap<ItemKind, Stacks>,
    by_tag: HashMap<Identifier, Stacks>,
    fuel: Option<Stacks>,
    empty: Stacks,
}

impl<'a> Ctx<'a> {
    pub fn new(
        registries: Option<&'a RegistryHolder>,
        tags: &'a HashMap<Identifier, Box<[ItemKind]>>,
    ) -> Self {
        Self {
            registries,
            tags,
            by_item: HashMap::new(),
            by_tag: HashMap::new(),
            fuel: None,
            empty: Arc::from(Vec::new()),
        }
    }

    pub fn tag(&self, tag: &Identifier) -> &[ItemKind] {
        self.tags.get(tag).map_or(&[], |t| t)
    }

    pub fn resolve(&mut self, display: &SlotDisplayData) -> Stacks {
        match display {
            SlotDisplayData::Empty => self.empty.clone(),
            SlotDisplayData::Item(d) => {
                if let Some(s) = self.by_item.get(&d.item) {
                    return s.clone();
                }
                let s = self.convert(vec![ItemStack::new(d.item, 1)]);
                self.by_item.insert(d.item, s.clone());
                s
            }
            SlotDisplayData::Tag(d) => {
                if let Some(s) = self.by_tag.get(&d.tag) {
                    return s.clone();
                }
                let stacks = self
                    .tag(&d.tag)
                    .iter()
                    .map(|&k| ItemStack::new(k, 1))
                    .collect();
                let s = self.convert(stacks);
                self.by_tag.insert(d.tag.clone(), s.clone());
                s
            }
            SlotDisplayData::AnyFuel => {
                if let Some(s) = &self.fuel {
                    return s.clone();
                }
                let stacks = fuel_items(self)
                    .into_iter()
                    .map(|k| ItemStack::new(k, 1))
                    .collect();
                let s = self.convert(stacks);
                self.fuel = Some(s.clone());
                s
            }
            _ => {
                let stacks = self.stacks(display);
                self.convert(stacks)
            }
        }
    }

    #[cfg(test)]
    pub fn first_item(&mut self, display: &SlotDisplayData) -> Option<&'static str> {
        self.resolve(display).first().map(|s| s.item)
    }

    pub fn display(&mut self, display: &RecipeDisplayData) -> ResolvedDisplay {
        let (shape, ingredients, result, station): (Shape, Vec<&SlotDisplayData>, _, _) =
            match display {
                RecipeDisplayData::Shaped(d) => (
                    Shape::Shaped {
                        width: d.width.min(u8::MAX as u32) as u8,
                        height: d.height.min(u8::MAX as u32) as u8,
                    },
                    d.ingredients.iter().collect(),
                    &d.result,
                    &d.crafting_station,
                ),
                RecipeDisplayData::Shapeless(d) => (
                    Shape::Shapeless,
                    d.ingredients.iter().collect(),
                    &d.result,
                    &d.crafting_station,
                ),
                RecipeDisplayData::Furnace(d) => (
                    Shape::Furnace,
                    vec![&d.ingredient, &d.fuel],
                    &d.result,
                    &d.crafting_station,
                ),
                RecipeDisplayData::Stonecutter(d) => (
                    Shape::Stonecutter,
                    vec![&d.input],
                    &d.result,
                    &d.crafting_station,
                ),
                RecipeDisplayData::Smithing(d) => (
                    Shape::Smithing,
                    vec![&d.template, &d.base, &d.addition],
                    &d.result,
                    &d.crafting_station,
                ),
            };
        ResolvedDisplay {
            shape,
            ingredients: ingredients.into_iter().map(|d| self.resolve(d)).collect(),
            result: self.resolve(result),
            station: self.resolve(station),
        }
    }

    fn stacks(&self, display: &SlotDisplayData) -> Vec<ItemStack> {
        match display {
            SlotDisplayData::Empty => Vec::new(),
            SlotDisplayData::Item(d) => vec![ItemStack::new(d.item, 1)],
            SlotDisplayData::ItemStack(d) => {
                if d.stack.is_present() {
                    vec![d.stack.clone()]
                } else {
                    Vec::new()
                }
            }
            SlotDisplayData::Tag(d) => self
                .tag(&d.tag)
                .iter()
                .map(|&k| ItemStack::new(k, 1))
                .collect(),
            SlotDisplayData::AnyFuel => fuel_items(self)
                .into_iter()
                .map(|k| ItemStack::new(k, 1))
                .collect(),
            SlotDisplayData::WithRemainder(d) => self.stacks(&d.input),
            SlotDisplayData::Composite(d) => {
                d.contents.iter().flat_map(|c| self.stacks(c)).collect()
            }
            SlotDisplayData::OnlyWithComponent(d) => self
                .stacks(&d.contents)
                .into_iter()
                .filter(|s| has_component(s, d.component))
                .collect(),
            SlotDisplayData::WithAnyPotion(d) => {
                let items = self.stacks(&d.contents);
                (0u32..)
                    .map_while(|i| Potion::try_from(i).ok())
                    .flat_map(|potion| {
                        items.iter().map(move |item| {
                            item.clone().with_component(
                                azalea_inventory::components::PotionContents {
                                    potion: Some(potion),
                                    custom_color: None,
                                    custom_effects: Vec::new(),
                                    custom_name: None,
                                },
                            )
                        })
                    })
                    .collect()
            }
            SlotDisplayData::Dyed(d) => self.demo(&d.target, &d.dye),
            SlotDisplayData::SmithingTrim(d) => self.demo(&d.base, &d.material),
        }
    }

    fn demo(&self, first: &SlotDisplayData, second: &SlotDisplayData) -> Vec<ItemStack> {
        const DEMO_LIMIT: usize = 16;
        let first = self.stacks(first);
        let second = self.stacks(second);
        if first.is_empty() || second.is_empty() {
            return Vec::new();
        }
        (0..first.len() * second.len())
            .map(|i| first[i % first.len()].clone())
            .take(DEMO_LIMIT)
            .collect()
    }

    fn convert(&self, stacks: Vec<ItemStack>) -> Stacks {
        stacks
            .iter()
            .map(|s| {
                let mut out = crate::play::inventory_bridge::slot_stack(s, self.registries, false);
                out.cooldown = 0;
                out.raw = None;
                out
            })
            .filter(|s| !s.is_empty())
            .collect()
    }
}

fn has_component(stack: &ItemStack, kind: DataComponentKind) -> bool {
    if kind == DataComponentKind::Dye {
        return stack
            .get_component::<azalea_inventory::components::Dye>()
            .is_some();
    }
    stack.component_patch().has_kind(kind)
}

enum Fuel {
    Item(ItemKind),
    Tag(&'static str),
}

const VANILLA_FUELS: &[Fuel] = {
    use Fuel::{Item as I, Tag as T};
    use ItemKind as K;
    &[
        I(K::LavaBucket),
        I(K::CoalBlock),
        I(K::BlazeRod),
        I(K::Coal),
        I(K::Charcoal),
        T("logs"),
        T("bamboo_blocks"),
        T("planks"),
        I(K::BambooMosaic),
        T("wooden_stairs"),
        I(K::BambooMosaicStairs),
        T("wooden_slabs"),
        I(K::BambooMosaicSlab),
        T("wooden_trapdoors"),
        T("wooden_pressure_plates"),
        T("wooden_shelves"),
        T("wooden_fences"),
        T("fence_gates"),
        I(K::NoteBlock),
        I(K::Bookshelf),
        I(K::ChiseledBookshelf),
        I(K::Lectern),
        I(K::Jukebox),
        I(K::Chest),
        I(K::TrappedChest),
        I(K::CraftingTable),
        I(K::DaylightDetector),
        T("banners"),
        I(K::Bow),
        I(K::FishingRod),
        I(K::Ladder),
        T("signs"),
        T("hanging_signs"),
        I(K::WoodenShovel),
        I(K::WoodenSword),
        I(K::WoodenSpear),
        I(K::WoodenHoe),
        I(K::WoodenAxe),
        I(K::WoodenPickaxe),
        T("wooden_doors"),
        T("boats"),
        T("wool"),
        T("wooden_buttons"),
        I(K::Stick),
        T("saplings"),
        I(K::Bowl),
        T("wool_carpets"),
        I(K::DriedKelpBlock),
        I(K::Crossbow),
        I(K::Bamboo),
        I(K::DeadBush),
        I(K::ShortDryGrass),
        I(K::TallDryGrass),
        I(K::Scaffolding),
        I(K::Loom),
        I(K::Barrel),
        I(K::CartographyTable),
        I(K::FletchingTable),
        I(K::SmithingTable),
        I(K::Composter),
        I(K::Azalea),
        I(K::FloweringAzalea),
        I(K::MangroveRoots),
        I(K::LeafLitter),
    ]
};

fn fuel_items(ctx: &Ctx) -> Vec<ItemKind> {
    let mut out: Vec<ItemKind> = Vec::new();
    let mut push = |k: ItemKind| {
        if !out.contains(&k) {
            out.push(k);
        }
    };
    for fuel in VANILLA_FUELS {
        match fuel {
            Fuel::Item(k) => push(*k),
            Fuel::Tag(t) => ctx.tag(&Identifier::new(*t)).iter().for_each(|&k| push(k)),
        }
    }
    let excluded = ctx.tag(&Identifier::new("non_flammable_wood"));
    out.retain(|k| !excluded.contains(k));
    out
}

pub fn place_recipe(
    grid_width: usize,
    grid_height: usize,
    recipe_width: usize,
    recipe_height: usize,
    count: usize,
    mut out: impl FnMut(usize, usize),
) {
    let (gw, gh, rw, rh) = (
        grid_width as i32,
        grid_height as i32,
        recipe_width as i32,
        recipe_height as i32,
    );
    let mut next = 0usize;
    let mut grid_index = 0i32;
    let mut y = 0i32;
    while y < gh {
        let centre = (rh as f32) < gh as f32 / 2.0;
        let start = (gh as f32 / 2.0 - rh as f32 / 2.0).floor() as i32;
        if centre && start > y {
            grid_index += gw;
            y += 1;
        }
        let mut x = 0i32;
        while x < gw {
            if next >= count {
                return;
            }
            let centre = (rw as f32) < gw as f32 / 2.0;
            let start = (gw as f32 / 2.0 - rw as f32 / 2.0).floor() as i32;
            let mut total = rw;
            let mut add = x < rw;
            if centre {
                total = start + rw;
                add = start <= x && x < start + rw;
            }
            if add {
                out(next, grid_index as usize);
                next += 1;
            } else if total == x {
                grid_index += gw - x;
                break;
            }
            grid_index += 1;
            x += 1;
        }
        y += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use azalea_protocol::common::recipe::{
        CompositeSlotDisplay, ItemSlotDisplay, ShapedCraftingRecipeDisplay, TagSlotDisplay,
        WithRemainderSlotDisplay,
    };

    fn item(k: ItemKind) -> SlotDisplayData {
        SlotDisplayData::Item(ItemSlotDisplay { item: k })
    }

    fn tag(t: &str) -> SlotDisplayData {
        SlotDisplayData::Tag(TagSlotDisplay {
            tag: Identifier::new(t),
        })
    }

    fn ids(s: &[SlotStack]) -> Vec<&'static str> {
        s.iter().map(|s| s.item).collect()
    }

    fn tags() -> HashMap<Identifier, Box<[ItemKind]>> {
        let mut t = HashMap::new();
        t.insert(
            Identifier::new("planks"),
            vec![
                ItemKind::OakPlanks,
                ItemKind::SprucePlanks,
                ItemKind::CrimsonPlanks,
            ]
            .into(),
        );
        t.insert(
            Identifier::new("logs"),
            vec![ItemKind::OakLog, ItemKind::CrimsonStem].into(),
        );
        t.insert(
            Identifier::new("non_flammable_wood"),
            vec![ItemKind::CrimsonPlanks, ItemKind::CrimsonStem].into(),
        );
        t
    }

    #[test]
    fn tag_resolves_to_its_items_and_is_shared() {
        let t = tags();
        let mut ctx = Ctx::new(None, &t);
        let a = ctx.resolve(&tag("planks"));
        assert_eq!(ids(&a), ["oak_planks", "spruce_planks", "crimson_planks"]);
        assert!(Arc::ptr_eq(&a, &ctx.resolve(&tag("planks"))));
        assert!(ctx.resolve(&tag("no_such_tag")).is_empty());
    }

    #[test]
    fn composite_and_remainder_flatten() {
        let t = tags();
        let mut ctx = Ctx::new(None, &t);
        let d = SlotDisplayData::Composite(CompositeSlotDisplay {
            contents: vec![
                item(ItemKind::Stick),
                SlotDisplayData::WithRemainder(Box::new(WithRemainderSlotDisplay {
                    input: item(ItemKind::WaterBucket),
                    remainder: item(ItemKind::Bucket),
                })),
                SlotDisplayData::Empty,
            ],
        });
        assert_eq!(ids(&ctx.resolve(&d)), ["stick", "water_bucket"]);
        assert_eq!(ctx.first_item(&d), Some("stick"));
    }

    #[test]
    fn any_fuel_keeps_builder_order_and_drops_non_flammable_wood() {
        let t = tags();
        let mut ctx = Ctx::new(None, &t);
        let fuel = ids(&ctx.resolve(&SlotDisplayData::AnyFuel));
        assert_eq!(
            &fuel[..6],
            [
                "lava_bucket",
                "coal_block",
                "blaze_rod",
                "coal",
                "charcoal",
                "oak_log"
            ]
        );
        assert_eq!(fuel[6], "oak_planks");
        assert!(!fuel.contains(&"crimson_planks"));
        assert!(!fuel.contains(&"crimson_stem"));
    }

    #[test]
    fn shaped_display_keeps_blank_cells() {
        let t = tags();
        let mut ctx = Ctx::new(None, &t);
        let d = ctx.display(&RecipeDisplayData::Shaped(ShapedCraftingRecipeDisplay {
            width: 2,
            height: 2,
            ingredients: vec![
                tag("planks"),
                SlotDisplayData::Empty,
                SlotDisplayData::Empty,
                tag("planks"),
            ],
            result: item(ItemKind::Stick),
            crafting_station: item(ItemKind::CraftingTable),
        }));
        assert_eq!(
            d.shape,
            Shape::Shaped {
                width: 2,
                height: 2
            }
        );
        assert_eq!(d.ingredients.len(), 4);
        assert!(d.ingredients[1].is_empty());
        assert_eq!(ids(&d.result), ["stick"]);
        assert_eq!(ids(&d.station), ["crafting_table"]);
    }

    fn placed(gw: usize, gh: usize, rw: usize, rh: usize) -> Vec<usize> {
        let mut out = vec![usize::MAX; rw * rh];
        place_recipe(gw, gh, rw, rh, rw * rh, |cell, slot| out[cell] = slot);
        out
    }

    #[test]
    fn place_recipe_matches_vanilla() {
        assert_eq!(placed(3, 3, 2, 2), [0, 1, 3, 4]);
        assert_eq!(placed(3, 3, 3, 3), [0, 1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(placed(3, 3, 1, 1), [4]);
        assert_eq!(placed(3, 3, 1, 3), [1, 4, 7]);
        assert_eq!(placed(2, 2, 1, 2), [0, 2]);
        assert_eq!(placed(2, 2, 2, 1), [0, 1]);
    }

    #[test]
    fn cycle_wraps() {
        let s = [
            SlotStack {
                item: "a",
                count: 1,
                ..Default::default()
            },
            SlotStack {
                item: "b",
                count: 1,
                ..Default::default()
            },
        ];
        assert_eq!(cycle(&s, 3).unwrap().item, "b");
        assert!(cycle(&[], 3).is_none());
    }
}
