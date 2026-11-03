mod book;
mod craft;
mod display;

pub use book::{BookSettings, BookType, Ingredient, RecipeBook, RecipeEntry, Tab, TypeSettings};
pub use craft::{BookMenu, CraftableStatus, Selection, StackedContents};
pub use display::{ResolvedDisplay, Shape, Stacks, cycle, place_recipe};

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use azalea::Client;
use azalea::local_player::WorldHolder;
use azalea_protocol::common::recipe::Ingredient as WireIngredient;
use azalea_protocol::common::tags::TagMap;
use azalea_protocol::packets::game::ClientboundGamePacket;
use azalea_protocol::packets::game::c_recipe_book_add::RecipeDisplayEntry;
use azalea_registry::HolderSet;
use azalea_registry::builtin::ItemKind;
use azalea_registry::identifier::Identifier;

use crate::gui::toast::ToastEvent;
use crate::session::SharedMutex;
use crate::util::global;
use display::Ctx;

const FLAG_NOTIFICATION: u8 = 1;
const FLAG_HIGHLIGHT: u8 = 2;

#[derive(Default)]
pub struct RecipeBookState {
    pub book: Arc<RecipeBook>,
    pub ghost: Option<GhostRecipe>,
    outgoing: Vec<BookRequest>,
}

pub struct GhostRecipe {
    pub container_id: i32,
    pub display: ResolvedDisplay,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BookRequest {
    Place {
        container_id: i32,
        recipe: u32,
        use_max_items: bool,
    },
    Settings {
        book: BookType,
        settings: TypeSettings,
    },
    Seen(u32),
}

impl RecipeBookState {
    pub fn request(&mut self, request: BookRequest) {
        match request {
            BookRequest::Place {
                container_id,
                recipe,
                use_max_items,
            } => self.place(container_id, recipe, use_max_items),
            BookRequest::Settings { book, settings } => self.set_settings(book, settings),
            BookRequest::Seen(recipe) => self.mark_seen(recipe),
        }
    }

    pub fn place(&mut self, container_id: i32, recipe: u32, use_max_items: bool) {
        self.outgoing.push(BookRequest::Place {
            container_id,
            recipe,
            use_max_items,
        });
    }

    pub fn set_settings(&mut self, book: BookType, settings: TypeSettings) {
        let mut all = self.book.settings();
        all.set(book, settings);
        Arc::make_mut(&mut self.book).set_settings(all);
        self.outgoing.push(BookRequest::Settings { book, settings });
    }

    pub fn mark_seen(&mut self, recipe: u32) {
        if self.book.will_highlight(recipe) {
            Arc::make_mut(&mut self.book).remove_highlight(recipe);
            self.outgoing.push(BookRequest::Seen(recipe));
        }
    }

    pub fn take_ghost(&mut self, container_id: i32) -> Option<ResolvedDisplay> {
        self.ghost
            .take()
            .filter(|g| g.container_id == container_id)
            .map(|g| g.display)
    }
}

global! {
fn item_tags() -> RwLock<HashMap<Identifier, Box<[ItemKind]>>>
}

pub(crate) fn set_item_tags(tags: &TagMap) {
    let Some(entries) = tags.get(&Identifier::new("item")) else {
        return;
    };
    let map = entries
        .iter()
        .map(|t| {
            let items = t
                .elements
                .iter()
                .filter_map(|&id| ItemKind::try_from(id as u32).ok())
                .collect();
            (t.name.clone(), items)
        })
        .collect();
    *item_tags().write().unwrap() = map;
}

pub(crate) fn clear_item_tags() {
    item_tags().write().unwrap().clear();
}

pub(crate) fn item_in_tag(tag: &str, kind: ItemKind) -> Option<bool> {
    let tags = item_tags().read().unwrap();
    tags.get(&Identifier::new(tag))
        .map(|items| items.contains(&kind))
}

fn item_id(kind: ItemKind) -> &'static str {
    kind.to_str().trim_start_matches("minecraft:")
}

fn resolve_entry(ctx: &mut Ctx, raw: Arc<RecipeDisplayEntry>) -> RecipeEntry {
    let ingredient = |ctx: &Ctx, i: &WireIngredient| {
        Ingredient::new(match &i.allowed {
            HolderSet::Direct { contents } => contents.iter().map(|&k| item_id(k)).collect(),
            HolderSet::Named { key, .. } => ctx.tag(key).iter().map(|&k| item_id(k)).collect(),
        })
    };
    RecipeEntry {
        id: raw.id,
        group: raw.group.checked_sub(1),
        category: raw.category,
        display: ctx.display(&raw.display),
        requirements: raw
            .crafting_requirements
            .as_ref()
            .map(|r| r.iter().map(|i| ingredient(ctx, i)).collect()),
        raw,
    }
}

pub(crate) fn packet(
    world: &WorldHolder,
    shared: &Arc<SharedMutex>,
    packet: &ClientboundGamePacket,
) {
    match packet {
        ClientboundGamePacket::RecipeBookAdd(p) => {
            let added: Vec<(Arc<RecipeEntry>, u8)> = {
                let world = world.shared.read();
                let tags = item_tags().read().unwrap();
                let mut ctx = Ctx::new(Some(&world.registries), &tags);
                p.entries
                    .iter()
                    .map(|e| {
                        (
                            Arc::new(resolve_entry(&mut ctx, Arc::new(e.contents.clone()))),
                            e.flags,
                        )
                    })
                    .collect()
            };
            let toasts: Vec<ToastEvent> = added
                .iter()
                .filter(|(_, flags)| flags & FLAG_NOTIFICATION != 0)
                .map(|(e, _)| ToastEvent::Recipe {
                    station: e
                        .display
                        .station
                        .first()
                        .map(|s| s.item.to_owned())
                        .unwrap_or_default(),
                    result: e
                        .display
                        .result
                        .first()
                        .map(|s| s.item.to_owned())
                        .unwrap_or_default(),
                })
                .collect();
            {
                let mut s = shared.lock().unwrap();
                let book = Arc::make_mut(&mut s.session.recipes.book);
                if p.replace {
                    book.clear();
                }
                for (e, flags) in added {
                    let id = e.id;
                    book.add(e);
                    if flags & FLAG_HIGHLIGHT != 0 {
                        book.add_highlight(id);
                    }
                }
                book.rebuild_collections();
            }
            if !toasts.is_empty() {
                crate::session::queue_toasts(shared, toasts);
            }
        }
        ClientboundGamePacket::RecipeBookRemove(p) => {
            let mut s = shared.lock().unwrap();
            let book = Arc::make_mut(&mut s.session.recipes.book);
            book.remove(&p.recipes);
            book.rebuild_collections();
        }
        ClientboundGamePacket::RecipeBookSettings(p) => {
            let mut s = shared.lock().unwrap();
            Arc::make_mut(&mut s.session.recipes.book)
                .set_settings(BookSettings::from_wire(&p.book_settings));
        }
        ClientboundGamePacket::PlaceGhostRecipe(p) => {
            let display = {
                let world = world.shared.read();
                let tags = item_tags().read().unwrap();
                Ctx::new(Some(&world.registries), &tags).display(&p.recipe)
            };
            shared.lock().unwrap().session.recipes.ghost = Some(GhostRecipe {
                container_id: p.container_id,
                display,
            });
        }
        ClientboundGamePacket::UpdateTags(_) => {
            let entries = shared
                .lock()
                .unwrap()
                .session
                .recipes
                .book
                .entries()
                .to_vec();
            if entries.is_empty() {
                return;
            }
            let resolved: Vec<Arc<RecipeEntry>> = {
                let world = world.shared.read();
                let tags = item_tags().read().unwrap();
                let mut ctx = Ctx::new(Some(&world.registries), &tags);
                entries
                    .iter()
                    .map(|e| Arc::new(resolve_entry(&mut ctx, e.raw.clone())))
                    .collect()
            };
            let mut s = shared.lock().unwrap();
            let book = Arc::make_mut(&mut s.session.recipes.book);
            for e in resolved {
                book.replace_resolved(e);
            }
        }
        _ => {}
    }
}

pub(crate) fn send_queued(bot: &Client, shared: &Arc<SharedMutex>) {
    use azalea_protocol::packets::game::s_place_recipe::ServerboundPlaceRecipe;
    use azalea_protocol::packets::game::s_recipe_book_change_settings::ServerboundRecipeBookChangeSettings;
    use azalea_protocol::packets::game::s_recipe_book_seen_recipe::ServerboundRecipeBookSeenRecipe;

    let queued = std::mem::take(&mut shared.lock().unwrap().session.recipes.outgoing);
    for request in queued {
        match request {
            BookRequest::Place {
                container_id,
                recipe,
                use_max_items,
            } => {
                bot.write_packet(ServerboundPlaceRecipe {
                    container_id,
                    recipe,
                    shift_down: use_max_items,
                });
            }
            BookRequest::Settings { book, settings } => {
                bot.write_packet(ServerboundRecipeBookChangeSettings {
                    book_type: book.wire(),
                    is_open: settings.open,
                    is_filtering: settings.filtering,
                });
            }
            BookRequest::Seen(recipe) => {
                bot.write_packet(ServerboundRecipeBookSeenRecipe { recipe });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use azalea_protocol::common::recipe::{
        FurnaceRecipeDisplay, ItemSlotDisplay, RecipeDisplayData, ShapelessCraftingRecipeDisplay,
        SlotDisplayData, TagSlotDisplay,
    };
    use azalea_registry::builtin::RecipeBookCategory as C;

    fn item(k: ItemKind) -> SlotDisplayData {
        SlotDisplayData::Item(ItemSlotDisplay { item: k })
    }

    fn raw(
        id: u32,
        group: u32,
        category: C,
        result: ItemKind,
        req: Option<Vec<WireIngredient>>,
    ) -> RecipeDisplayEntry {
        RecipeDisplayEntry {
            id,
            display: RecipeDisplayData::Shapeless(ShapelessCraftingRecipeDisplay {
                ingredients: vec![SlotDisplayData::Tag(TagSlotDisplay {
                    tag: Identifier::new("planks"),
                })],
                result: item(result),
                crafting_station: item(ItemKind::CraftingTable),
            }),
            group,
            category,
            crafting_requirements: req,
        }
    }

    fn book(entries: Vec<RecipeDisplayEntry>) -> RecipeBook {
        let tags = HashMap::from([(
            Identifier::new("planks"),
            Box::from([ItemKind::OakPlanks, ItemKind::BirchPlanks]),
        )]);
        let mut ctx = Ctx::new(None, &tags);
        let mut b = RecipeBook::default();
        for e in entries {
            b.add(Arc::new(resolve_entry(&mut ctx, Arc::new(e))));
        }
        b.rebuild_collections();
        b
    }

    fn ids(b: &RecipeBook, tab: Tab) -> Vec<Vec<u32>> {
        b.tab(tab)
            .iter()
            .map(|&c| b.collection(c).iter().map(|&e| b.entry(e).id).collect())
            .collect()
    }

    #[test]
    fn groups_merge_per_category_in_id_order() {
        let b = book(vec![
            raw(5, 2, C::CraftingMisc, ItemKind::Stick, None),
            raw(1, 0, C::CraftingMisc, ItemKind::Bowl, None),
            raw(3, 2, C::CraftingMisc, ItemKind::Ladder, None),
            raw(4, 2, C::CraftingRedstone, ItemKind::Lever, None),
            raw(2, 0, C::CraftingEquipment, ItemKind::WoodenSword, None),
        ]);
        assert_eq!(
            ids(&b, Tab::Category(C::CraftingMisc)),
            [vec![1], vec![3, 5]]
        );
        assert_eq!(ids(&b, Tab::Category(C::CraftingRedstone)), [vec![4]]);
        assert_eq!(
            ids(&b, Tab::Search(BookType::Crafting)),
            [vec![2], vec![1], vec![3, 5], vec![4]]
        );
        assert!(ids(&b, Tab::Search(BookType::Smoker)).is_empty());
        assert_eq!(b.entry(b.index_of(5).unwrap()).group, Some(1));
    }

    #[test]
    fn add_replaces_and_remove_drops_highlight() {
        let mut b = book(vec![raw(1, 0, C::CraftingMisc, ItemKind::Bowl, None)]);
        b.add_highlight(1);
        let tags = HashMap::new();
        let mut ctx = Ctx::new(None, &tags);
        b.add(Arc::new(resolve_entry(
            &mut ctx,
            Arc::new(raw(1, 0, C::CraftingMisc, ItemKind::Stick, None)),
        )));
        b.rebuild_collections();
        assert_eq!(b.entries().len(), 1);
        assert_eq!(b.entry(0).display.result[0].item, "stick");
        b.remove(&[1]);
        b.rebuild_collections();
        assert!(!b.will_highlight(1));
        assert!(b.tab(Tab::Search(BookType::Crafting)).is_empty());
    }

    #[test]
    fn tag_ingredients_resolve_and_feed_the_craft_check() {
        let planks = WireIngredient {
            allowed: HolderSet::Named {
                key: Identifier::new("planks"),
                contents: Vec::new(),
            },
        };
        let b = book(vec![
            raw(
                1,
                0,
                C::CraftingMisc,
                ItemKind::Stick,
                Some(vec![planks.clone(), planks]),
            ),
            raw(2, 0, C::CraftingMisc, ItemKind::Bowl, None),
        ]);
        let mut slots = vec![crate::session::SlotStack::default(); 46];
        slots[9] = crate::session::SlotStack {
            item: "birch_planks",
            count: 2,
            ..Default::default()
        };
        let sel = Selection::compute(&b, BookMenu::INVENTORY, &slots);
        let (stick, bowl) = (b.index_of(1).unwrap(), b.index_of(2).unwrap());
        assert!(sel.is_craftable(stick));
        assert!(sel.is_selected(bowl) && !sel.is_craftable(bowl));
        slots[9].count = 1;
        assert!(!Selection::compute(&b, BookMenu::INVENTORY, &slots).is_craftable(stick));
    }

    #[test]
    fn furnace_menu_selects_only_furnace_displays() {
        let mut furnace = raw(7, 0, C::FurnaceFood, ItemKind::CookedBeef, None);
        furnace.display = RecipeDisplayData::Furnace(FurnaceRecipeDisplay {
            ingredient: item(ItemKind::Beef),
            fuel: SlotDisplayData::AnyFuel,
            result: item(ItemKind::CookedBeef),
            crafting_station: item(ItemKind::Furnace),
            duration: 200,
            experience: 0.35,
        });
        let b = book(vec![
            furnace,
            raw(1, 0, C::CraftingMisc, ItemKind::Stick, None),
        ]);
        let menu = BookMenu::furnace(crate::session::FurnaceKind::Furnace);
        let sel = Selection::compute(&b, menu, &vec![crate::session::SlotStack::default(); 39]);
        assert!(sel.is_selected(b.index_of(7).unwrap()));
        assert!(!sel.is_selected(b.index_of(1).unwrap()));
        let c = b.tab(Tab::Search(BookType::Furnace))[0];
        assert!(sel.has_any_selected(&b, c) && !sel.has_craftable(&b, c));
    }

    #[test]
    fn requests_queue_in_order() {
        let mut st = RecipeBookState::default();
        Arc::make_mut(&mut st.book).add_highlight(9);
        st.mark_seen(9);
        st.request(BookRequest::Seen(9));
        st.set_settings(
            BookType::Smoker,
            TypeSettings {
                open: true,
                filtering: false,
            },
        );
        st.place(3, 9, true);
        assert_eq!(
            st.outgoing,
            [
                BookRequest::Seen(9),
                BookRequest::Settings {
                    book: BookType::Smoker,
                    settings: TypeSettings {
                        open: true,
                        filtering: false
                    },
                },
                BookRequest::Place {
                    container_id: 3,
                    recipe: 9,
                    use_max_items: true
                },
            ]
        );
        assert!(st.book.settings().get(BookType::Smoker).open);
        assert!(!st.book.settings().get(BookType::Crafting).open);
    }
}
