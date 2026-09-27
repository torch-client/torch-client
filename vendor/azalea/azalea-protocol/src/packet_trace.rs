use std::sync::OnceLock;

use crate::packets::ConnectionProtocol;

pub struct TracedPacket<'a> {
    pub phase: ConnectionProtocol,
    pub name: Option<&'static str>,
    pub bytes: &'a [u8],
}

pub type SentPacketHook = Box<dyn Fn(&TracedPacket) + Send + Sync>;

pub type RecvPacketHook = Box<dyn Fn(&TracedPacket) + Send + Sync>;

static SENT: OnceLock<SentPacketHook> = OnceLock::new();
static RECV: OnceLock<RecvPacketHook> = OnceLock::new();

pub fn set_sent_packet_hook(hook: SentPacketHook) -> Result<(), SentPacketHook> {
    SENT.set(hook)
}

pub fn set_recv_packet_hook(hook: RecvPacketHook) -> Result<(), RecvPacketHook> {
    RECV.set(hook)
}

#[inline]
pub fn sent_hooked() -> bool {
    SENT.get().is_some()
}

#[inline]
pub fn recv_hooked() -> bool {
    RECV.get().is_some()
}

pub fn note_sent(phase: ConnectionProtocol, name: &'static str, bytes: &[u8]) {
    if let Some(hook) = SENT.get() {
        hook(&TracedPacket {
            phase,
            name: Some(name),
            bytes,
        });
    }
}

pub fn note_recv(phase: ConnectionProtocol, name: Option<&'static str>, bytes: &[u8]) {
    if let Some(hook) = RECV.get() {
        hook(&TracedPacket { phase, name, bytes });
    }
}
