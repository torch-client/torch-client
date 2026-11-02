#[cfg(target_arch = "wasm32")]
mod host;
#[cfg(target_arch = "wasm32")]
pub(crate) mod mirror;
#[cfg(target_arch = "wasm32")]
mod msg;
#[cfg(target_arch = "wasm32")]
pub(crate) mod tally;

#[cfg(not(target_arch = "wasm32"))]
mod seam {
    use super::*;

    pub(crate) fn active() -> bool {
        false
    }

    pub(crate) fn spawn(_assets: &[u8]) -> bool {
        false
    }

    pub(crate) fn offer_chunk_packet(_p: &ChunkPacket) -> bool {
        false
    }

    pub(crate) fn offer_light_update(_cx: i32, _cz: i32, _data: &LightPacket) -> bool {
        false
    }

    pub(crate) fn drop_chunk_packet(_cx: i32, _cz: i32) {}

    pub(crate) fn forward_chunk(
        _world: &WorldRef,
        _cx: i32,
        _cz: i32,
        _mesh: &[ColumnPos],
    ) -> bool {
        false
    }

    pub(crate) fn forward_block_change(_pos: BlockPos, _new: BlockState) -> bool {
        false
    }

    pub(crate) fn forward_unload(_cx: i32, _cz: i32) -> bool {
        false
    }

    pub(crate) fn forward_reset_light() -> bool {
        false
    }

    pub(crate) fn forward_reset_level() -> bool {
        false
    }

    pub(crate) fn note_backlog(_sections: usize) {}

    pub(crate) fn sweep_parked(_partial: &crate::client::viewwindow::PartialWorldRef) {}

    pub(crate) fn take_died() -> bool {
        false
    }

    pub(crate) fn sync_options(_lighting: bool, _smooth: bool) {}

    #[cfg(resource_packs)]
    pub(crate) fn forward_packs() -> bool {
        false
    }

    #[cfg(feature = "shader_support")]
    pub(crate) fn forward_pack_ids(_ids: Option<&crate::renderer::packvertex::BlockIds>) {}
}

#[cfg(target_arch = "wasm32")]
mod seam {
    use std::cell::RefCell;
    use std::collections::HashMap;

    use azalea_buf::AzBuf;

    use super::host;
    use super::msg::ToWorker;
    use super::*;

    pub(crate) use host::{active, spawn, take_died};

    thread_local! {
        static PARKED: RefCell<HashMap<(i32, i32), (Vec<u8>, u32)>> = RefCell::new(HashMap::new());
        static PARK_TICK: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
        static SENT_LEVEL: RefCell<Option<(u8, i32, u32)>> = const { RefCell::new(None) };
        static SENT_BIOMES: RefCell<Option<std::sync::Arc<Vec<u8>>>> =
            const { RefCell::new(None) };
        static SENT_BACKLOG: std::cell::Cell<Option<(u32, u32)>> =
            const { std::cell::Cell::new(None) };
        static RECEIVED: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
        static BLOCKS: RefCell<(Vec<(i32, i32, i32, u32)>, HashMap<(i32, i32, i32), usize>)> =
            RefCell::new((Vec::new(), HashMap::new()));
        #[cfg(resource_packs)]
        static SENT_PACKS: RefCell<Vec<std::sync::Arc<crate::platform::assets::archive::OwnedLayer>>> =
            const { RefCell::new(Vec::new()) };
    }

    pub(crate) fn offer_chunk_packet(p: &ChunkPacket) -> bool {
        if !active() {
            return false;
        }
        let mut bytes = Vec::new();
        let written = super::tally::time("re-encode", || p.azalea_write(&mut bytes), |_| 0);
        if written.is_err() {
            crate::log_error!(
                "mesh",
                "the chunk packet for {},{} would not encode",
                p.x,
                p.z
            );
            return false;
        }
        let tick = PARK_TICK.with(std::cell::Cell::get);
        PARKED.with(|parked| parked.borrow_mut().insert((p.x, p.z), (bytes, tick)));
        true
    }

    pub(crate) fn offer_light_update(cx: i32, cz: i32, data: &LightPacket) -> bool {
        if !active() {
            return false;
        }
        let mut bytes = Vec::new();
        if data.azalea_write(&mut bytes).is_err() {
            return false;
        }
        send(&ToWorker::LightUpdate {
            cx,
            cz,
            data: bytes,
        });
        true
    }

    pub(crate) fn drop_chunk_packet(cx: i32, cz: i32) {
        PARKED.with(|parked| parked.borrow_mut().remove(&(cx, cz)));
    }

    pub(crate) fn forward_chunk(world: &WorldRef, cx: i32, cz: i32, mesh: &[ColumnPos]) -> bool {
        if !active() {
            return false;
        }
        send_level(world);
        send_biomes(world);
        let packet = PARKED
            .with(|parked| parked.borrow_mut().remove(&(cx, cz)))
            .map(|(bytes, _)| bytes)
            .unwrap_or_default();
        send(&ToWorker::Chunk {
            cx,
            cz,
            mesh: mesh.iter().map(|c| (c.x, c.z)).collect(),
            packet,
        });
        true
    }

    fn send_level(world: &WorldRef) {
        let dim = crate::renderer::dimension::id_of(crate::renderer::dimension::current());
        let (min_y, height) = {
            let w = world.read();
            (w.chunks.min_y(), w.chunks.height())
        };
        SENT_LEVEL.with(|sent| {
            let mut sent = sent.borrow_mut();
            if *sent == Some((dim, min_y, height)) {
                return;
            }
            *sent = Some((dim, min_y, height));
            send(&ToWorker::Level { dim, min_y, height });
        });
    }

    fn send_biomes(world: &WorldRef) {
        let table = crate::client::worker::biome_table(&world.read().registries);
        if table.is_empty() {
            return;
        }
        SENT_BIOMES.with(|sent| {
            let mut sent = sent.borrow_mut();
            if sent
                .as_ref()
                .is_some_and(|t| std::sync::Arc::ptr_eq(t, &table))
            {
                return;
            }
            *sent = Some(table.clone());
            send(&ToWorker::BiomeTable((*table).clone()));
        });
    }

    pub(crate) fn forward_block_change(pos: BlockPos, new: BlockState) -> bool {
        if !active() {
            return false;
        }
        let state = new.id() as u32;
        BLOCKS.with(|blocks| {
            let (list, at) = &mut *blocks.borrow_mut();
            match at.get(&(pos.x, pos.y, pos.z)) {
                Some(&i) => list[i].3 = state,
                None => {
                    at.insert((pos.x, pos.y, pos.z), list.len());
                    list.push((pos.x, pos.y, pos.z, state));
                }
            }
        });
        true
    }

    fn flush_blocks() {
        let batch = BLOCKS.with(|blocks| {
            let (list, at) = &mut *blocks.borrow_mut();
            at.clear();
            std::mem::take(list)
        });
        if !batch.is_empty() {
            super::tally::count("block writes", batch.len() as u64);
            host::send(&ToWorker::Blocks(batch));
        }
    }

    fn send(msg: &ToWorker) {
        flush_blocks();
        host::send(msg);
    }

    pub(crate) fn forward_unload(cx: i32, cz: i32) -> bool {
        if !active() {
            return false;
        }
        drop_chunk_packet(cx, cz);
        send(&ToWorker::Unload { cx, cz });
        true
    }

    pub(crate) fn forward_reset_light() -> bool {
        if !active() {
            return false;
        }
        clear_light();
        send(&ToWorker::ResetLight);
        true
    }

    pub(crate) fn forward_reset_level() -> bool {
        if !active() {
            return false;
        }
        clear_light();
        BLOCKS.with(|blocks| {
            let (list, at) = &mut *blocks.borrow_mut();
            list.clear();
            at.clear();
        });
        PARKED.with(|parked| parked.borrow_mut().clear());
        SENT_LEVEL.with(|sent| *sent.borrow_mut() = None);
        SENT_BIOMES.with(|sent| *sent.borrow_mut() = None);
        send(&ToWorker::ResetLevel);
        true
    }

    fn clear_light() {
        *crate::client::worldsync::light_map().write() = Default::default();
    }

    pub(crate) fn note_backlog(sections: usize) {
        if !active() {
            return;
        }
        flush_blocks();
        let pending = sections.min(u32::MAX as usize) as u32;
        let received = RECEIVED.with(std::cell::Cell::get);
        SENT_BACKLOG.with(|sent| {
            if sent.get() == Some((pending, received)) {
                return;
            }
            sent.set(Some((pending, received)));
            send(&ToWorker::Backlog { pending, received });
        });
    }

    pub(crate) fn sweep_parked(partial: &crate::client::viewwindow::PartialWorldRef) {
        const PARK_TTL: u32 = 40;
        let now = PARK_TICK.with(|t| {
            let now = t.get().wrapping_add(1);
            t.set(now);
            now
        });
        PARKED.with(|parked| {
            let mut parked = parked.borrow_mut();
            if parked.is_empty() {
                return;
            }
            let partial = partial.read();
            parked.retain(|&(x, z), (_, at)| {
                if now.wrapping_sub(*at) < PARK_TTL {
                    return true;
                }
                let pos = azalea_core::position::ChunkPos::new(x, z);
                partial.chunks.in_range(&pos) && partial.chunks.limited_get(&pos).is_some()
            });
        });
    }

    pub(crate) fn note_received(n: usize) {
        RECEIVED.with(|r| r.set(r.get().wrapping_add(n as u32)));
    }

    pub(crate) fn sync_options(lighting: bool, smooth: bool) {
        if !active() {
            return;
        }
        send(&ToWorker::Options { lighting, smooth });
    }

    #[cfg(resource_packs)]
    pub(crate) fn forward_packs() -> bool {
        if !active() {
            return false;
        }
        let layers = crate::platform::assets::archive::mounted();
        SENT_PACKS.with(|sent| {
            let mut sent = sent.borrow_mut();
            let same = sent.len() == layers.len()
                && sent
                    .iter()
                    .zip(&layers)
                    .all(|(a, b)| std::sync::Arc::ptr_eq(a, b));
            if same {
                return;
            }
            send(&ToWorker::Packs(layers.clone()));
            *sent = layers;
        });
        true
    }

    #[cfg(feature = "shader_support")]
    pub(crate) fn forward_pack_ids(ids: Option<&crate::renderer::packvertex::BlockIds>) {
        if !active() {
            return;
        }
        send(&ToWorker::PackIds(
            ids.map(|ids| ids.encode()).unwrap_or_default(),
        ));
    }
}

pub(crate) use seam::*;

use crate::lighting::ColumnPos;
use azalea::block::BlockState;
use azalea_core::position::BlockPos;
use azalea_protocol::packets::game::c_level_chunk_with_light::ClientboundLevelChunkWithLight as ChunkPacket;
use azalea_protocol::packets::game::c_light_update::ClientboundLightUpdatePacketData as LightPacket;

type WorldRef = std::sync::Arc<parking_lot::RwLock<azalea_world::World>>;
