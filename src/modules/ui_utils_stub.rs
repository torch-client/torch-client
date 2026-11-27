use super::registry::Id;
use super::{Edge, Phase};

static EDGE: Edge = Edge::new(Id::UiUtils);

pub fn tick(shared: &crate::session::SharedMutex) {
    if EDGE.poll(super::store()) == Phase::Started {
        let mut s = shared.lock().unwrap();
        super::chat_math::show_local_colored(
            &mut s.session,
            crate::lang::tr!("modmsg.ui_utils.not_built"),
            super::chat_math::RED,
        );
    }
}
