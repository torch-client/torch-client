#[cfg(target_arch = "wasm32")]
mod host;
#[cfg(target_arch = "wasm32")]
pub(crate) mod mirror;
#[cfg(target_arch = "wasm32")]
mod msg;

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

    pub(crate) fn sync_options(_lighting: bool, _smooth: bool) {}
}

#[cfg(target_arch = "wasm32")]
mod seam {
    use std::cell::RefCell;
    use std::collections::HashMap;

    use azalea_buf::AzBuf;

    use super::host;
    use super::msg::ToWorker;
    use super::*;

    pub(crate) use host::{active, spawn};

    thread_local! {
        static PARKED: RefCell<HashMap<(i32, i32), Vec<u8>>> = RefCell::new(HashMap::new());
        static SENT_LEVEL: RefCell<Option<(u8, i32, u32)>> = const { RefCell::new(None) };
        static SENT_BIOMES: RefCell<Option<std::sync::Arc<Vec<u8>>>> =
            const { RefCell::new(None) };
        static SENT_BACKLOG: RefCell<u32> = const { RefCell::new(u32::MAX) };
    }

    pub(crate) fn offer_chunk_packet(p: &ChunkPacket) -> bool {
        if !active() {
            return false;
        }
        let mut bytes = Vec::new();
        if p.azalea_write(&mut bytes).is_err() {
            crate::log_error!(
                "mesh",
                "the chunk packet for {},{} would not encode",
                p.x,
                p.z
            );
            return false;
        }
        PARKED.with(|parked| parked.borrow_mut().insert((p.x, p.z), bytes));
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
        host::send(&ToWorker::LightUpdate {
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
            .unwrap_or_default();
        host::send(&ToWorker::Chunk {
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
            host::send(&ToWorker::Level { dim, min_y, height });
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
            host::send(&ToWorker::BiomeTable((*table).clone()));
        });
    }

    pub(crate) fn forward_block_change(pos: BlockPos, new: BlockState) -> bool {
        if !active() {
            return false;
        }
        host::send(&ToWorker::BlockChange {
            x: pos.x,
            y: pos.y,
            z: pos.z,
            state: new.id() as u32,
        });
        true
    }

    pub(crate) fn forward_unload(cx: i32, cz: i32) -> bool {
        if !active() {
            return false;
        }
        drop_chunk_packet(cx, cz);
        host::send(&ToWorker::Unload { cx, cz });
        true
    }

    pub(crate) fn forward_reset_light() -> bool {
        if !active() {
            return false;
        }
        clear_light();
        host::send(&ToWorker::ResetLight);
        true
    }

    pub(crate) fn forward_reset_level() -> bool {
        if !active() {
            return false;
        }
        clear_light();
        PARKED.with(|parked| parked.borrow_mut().clear());
        SENT_LEVEL.with(|sent| *sent.borrow_mut() = None);
        SENT_BIOMES.with(|sent| *sent.borrow_mut() = None);
        host::send(&ToWorker::ResetLevel);
        true
    }

    fn clear_light() {
        *crate::client::worldsync::light_map().write() = Default::default();
    }

    pub(crate) fn note_backlog(sections: usize) {
        if !active() {
            return;
        }
        let sections = sections.min(u32::MAX as usize) as u32;
        SENT_BACKLOG.with(|sent| {
            let mut sent = sent.borrow_mut();
            if *sent == sections {
                return;
            }
            *sent = sections;
            host::send(&ToWorker::Backlog(sections));
        });
    }

    pub(crate) fn sync_options(lighting: bool, smooth: bool) {
        if !active() {
            return;
        }
        host::send(&ToWorker::Options { lighting, smooth });
    }
}

pub(crate) use seam::*;

use crate::lighting::ColumnPos;
use azalea::block::BlockState;
use azalea_core::position::BlockPos;
use azalea_protocol::packets::game::c_level_chunk_with_light::ClientboundLevelChunkWithLight as ChunkPacket;
use azalea_protocol::packets::game::c_light_update::ClientboundLightUpdatePacketData as LightPacket;

type WorldRef = std::sync::Arc<parking_lot::RwLock<azalea_world::World>>;
