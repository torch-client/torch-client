use std::{
    fmt::Debug,
    io::Cursor,
    mem,
    sync::{
        Arc,
        atomic::{self, AtomicBool},
    },
};

use azalea_crypto::Aes128CfbEnc;
use azalea_protocol::{
    connect::{RawReadConnection, RawWriteConnection},
    packets::{
        ConnectionProtocol, Packet, ProtocolPacket,
        config::ClientboundConfigPacket,
        game::{ClientboundBundleDelimiter, ClientboundGamePacket},
        login::ClientboundLoginPacket,
    },
    read::{ReadPacketError, deserialize_packet},
    write::serialize_packet,
};
use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use bevy_tasks::{IoTaskPool, futures_lite::future};
use thiserror::Error;
use tokio::{
    io::AsyncWriteExt,
    sync::mpsc::{self},
};
use tracing::{debug, error, info, trace};

use super::packet::{
    config::ReceiveConfigPacketEvent, game::ReceiveGamePacketEvent, login::ReceiveLoginPacketEvent,
};
use crate::packet::{config, game, login};

#[cfg(feature = "multiversion")]
pub trait PacketTranslator: Send + Sync {
    fn inbound(&mut self, phase: ConnectionProtocol, raw: Box<[u8]>) -> Box<[u8]>;
    fn outbound(&mut self, phase: ConnectionProtocol, raw: Box<[u8]>) -> Box<[u8]>;
}

pub struct ConnectionPlugin;
impl Plugin for ConnectionPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreUpdate, (read_packets, poll_all_writer_tasks).chain());
    }
}

pub fn read_packets(ecs: &mut World) {
    let mut entity_and_conn_query = ecs.query::<(Entity, &mut RawConnection)>();
    let mut conn_query = ecs.query::<&mut RawConnection>();

    let mut entities_handling_packets = Vec::new();
    let mut entities_with_injected_packets = Vec::new();
    for (entity, mut raw_conn) in entity_and_conn_query.iter_mut(ecs) {
        if !raw_conn.injected_clientbound_packets.is_empty() {
            entities_with_injected_packets.push((
                entity,
                mem::take(&mut raw_conn.injected_clientbound_packets),
            ));
        }

        if raw_conn.network.is_none() {
            if !raw_conn.warned_no_network {
                raw_conn.warned_no_network = true;
                tracing::warn!(
                    "this connection has no network any more, so no further packet will be read on it; whatever dropped it should have logged above"
                );
            }
            continue;
        }

        entities_handling_packets.push(entity);
    }

    let mut queued_packet_events = QueuedPacketEvents::default();

    for (entity, raw_packets) in entities_with_injected_packets {
        for raw_packet in raw_packets {
            let conn = conn_query.get(ecs, entity).unwrap();
            let state = conn.state;

            trace!("Received injected packet with bytes: {raw_packet:?}");
            if let Err(e) =
                handle_raw_packet(ecs, &raw_packet, entity, state, &mut queued_packet_events)
            {
                error!("Error reading injected packet: {e}");
            }
        }
    }

    #[cfg(target_arch = "wasm32")]
    let read_started = bevy_platform::time::Instant::now();

    let bundle_delimiter = bundle_delimiter_byte();

    for entity in entities_handling_packets {
        loop {
            let mut conn = conn_query.get_mut(ecs, entity).unwrap();
            #[cfg(target_arch = "wasm32")]
            if matches!(conn.state, ConnectionProtocol::Game) {
                let lag = azalea_protocol::traffic::read_lag();
                if lag >= READ_LAG_HARD && !conn.read_lag_warned {
                    conn.read_lag_warned = true;
                    tracing::warn!(
                        "the packet reader is {} ms behind the server ({} packets queued); lifting the read budget until it catches up",
                        lag.as_millis(),
                        azalea_protocol::traffic::rx_queued()
                    );
                }
                if web_read_budget(lag).is_some_and(|budget| read_started.elapsed() >= budget) {
                    conn.read_backlog = true;
                    break;
                }
            }
            let net_conn = conn.net_conn().unwrap();
            let read_res = net_conn.reader.try_read();
            let state = conn.state;
            match read_res {
                Ok(Some(raw_packet)) => {
                    #[cfg(feature = "multiversion")]
                    let raw_packet = match conn.translator.as_deref_mut() {
                        Some(translator) => translator.inbound(state, raw_packet),
                        None => raw_packet,
                    };
                    let raw_packet = Arc::<[u8]>::from(raw_packet);
                    if matches!(state, ConnectionProtocol::Game) {
                        if raw_packet.len() == 1 && Some(raw_packet[0]) == bundle_delimiter {
                            if mem::take(&mut conn.bundle_overflowed) {
                                continue;
                            }
                            match conn.bundle.take() {
                                None => conn.bundle = Some(Vec::new()),
                                Some(frames) => {
                                    handle_bundle(ecs, entity, &frames, &mut queued_packet_events)
                                }
                            }
                            continue;
                        }
                        if let Some(frames) = conn.bundle.as_mut() {
                            frames.push(raw_packet);
                            if frames.len() < BUNDLE_SIZE_LIMIT {
                                continue;
                            }
                            error!(
                                "a packet bundle reached {BUNDLE_SIZE_LIMIT} packets without its closing delimiter; handling the rest of it unbundled"
                            );
                            let frames = conn.bundle.take().unwrap_or_default();
                            conn.bundle_overflowed = true;
                            handle_bundle(ecs, entity, &frames, &mut queued_packet_events);
                            continue;
                        }
                    } else if conn.bundle.is_some() || conn.bundle_overflowed {
                        let dropped = conn.bundle.take().map_or(0, |frames| frames.len());
                        conn.bundle_overflowed = false;
                        if dropped > 0 {
                            error!(
                                "dropped {dropped} bundled game packets: the connection left the game state ({state:?}) before the bundle closed"
                            );
                        }
                    }
                    handle_read_frame(ecs, &raw_packet, entity, state, &mut queued_packet_events);
                }
                Ok(None) => {
                    #[cfg(target_arch = "wasm32")]
                    {
                        conn.read_backlog = false;
                        conn.read_lag_warned = false;
                    }
                    break;
                }
                Err(err) => {
                    log_for_error(&err);

                    if matches!(
                        &*err,
                        ReadPacketError::IoError { .. } | ReadPacketError::ConnectionClosed
                    ) {
                        info!("Server closed connection");
                        conn.network = None;
                        conn.is_alive = false;
                        conn.bundle = None;
                        conn.bundle_overflowed = false;
                    }

                    break;
                }
            }
        }
    }

    queued_packet_events.write_messages(ecs);
}

#[cfg(target_arch = "wasm32")]
const WEB_READ_BUDGET: std::time::Duration = std::time::Duration::from_millis(4);
#[cfg(target_arch = "wasm32")]
const WEB_READ_BUDGET_MAX: std::time::Duration = std::time::Duration::from_millis(50);
#[cfg(target_arch = "wasm32")]
const READ_LAG_SOFT: std::time::Duration = std::time::Duration::from_millis(200);
#[cfg(target_arch = "wasm32")]
const READ_LAG_HARD: std::time::Duration = std::time::Duration::from_millis(600);

#[cfg(target_arch = "wasm32")]
fn web_read_budget(lag: std::time::Duration) -> Option<std::time::Duration> {
    if lag <= READ_LAG_SOFT {
        return Some(WEB_READ_BUDGET);
    }
    if lag >= READ_LAG_HARD {
        return None;
    }
    let t = (lag - READ_LAG_SOFT).as_secs_f64() / (READ_LAG_HARD - READ_LAG_SOFT).as_secs_f64();
    Some(WEB_READ_BUDGET + (WEB_READ_BUDGET_MAX - WEB_READ_BUDGET).mul_f64(t))
}

fn poll_all_writer_tasks(mut conn_query: Query<&mut RawConnection>) {
    for mut conn in conn_query.iter_mut() {
        if let Some(net_conn) = &mut conn.network {

            if net_conn.poll_writer().is_some() {
                error!(
                    "the packet writer task ended, so this connection is now dead: no packet                      will be read or sent on it again"
                );
                conn.network = None;
                conn.is_alive = false;
            }
        }
    }
}

#[derive(Default)]
pub struct QueuedPacketEvents {
    login: Vec<ReceiveLoginPacketEvent>,
    config: Vec<ReceiveConfigPacketEvent>,
    game: Vec<ReceiveGamePacketEvent>,
}
impl QueuedPacketEvents {
    fn write_messages(&mut self, ecs: &mut World) {
        ecs.write_message_batch(self.login.drain(..));
        ecs.write_message_batch(self.config.drain(..));
        ecs.write_message_batch(self.game.drain(..));
    }
}

fn log_for_error(error: &ReadPacketError) {
    if !matches!(*error, ReadPacketError::ConnectionClosed) {
        error!("Error reading packet from Client: {error:?}");
    }
}

const BUNDLE_SIZE_LIMIT: usize = 4096;

fn bundle_delimiter_byte() -> Option<u8> {
    let id = ClientboundGamePacket::BundleDelimiter(ClientboundBundleDelimiter).id();
    u8::try_from(id).ok().filter(|id| *id < 0x80)
}

fn handle_bundle(
    ecs: &mut World,
    entity: Entity,
    frames: &[Arc<[u8]>],
    queued_packet_events: &mut QueuedPacketEvents,
) {
    for frame in frames {
        let Some(state) = ecs.get::<RawConnection>(entity).map(|conn| conn.state) else {
            return;
        };
        handle_read_frame(ecs, frame, entity, state, queued_packet_events);
    }
}

fn handle_read_frame(
    ecs: &mut World,
    raw_packet: &[u8],
    entity: Entity,
    state: ConnectionProtocol,
    queued_packet_events: &mut QueuedPacketEvents,
) {
    #[cfg(target_arch = "wasm32")]
    let packet_started = bevy_platform::time::Instant::now();
    if let Err(e) = handle_raw_packet(ecs, raw_packet, entity, state, queued_packet_events) {
        let first = azalea_registry::unknown::note_packet(match &*e {
            ReadPacketError::Parse { packet_name, .. }
            | ReadPacketError::LeftoverData { packet_name, .. } => packet_name.as_str(),
            _ => "unnamed",
        });
        if first {
            let head = &raw_packet[..raw_packet.len().min(16)];
            error!(
                "Error reading packet: {e} ({state:?}, {} bytes, starts {head:02x?})",
                raw_packet.len()
            );
        }
    }
    #[cfg(target_arch = "wasm32")]
    warn_slow_packet(state, raw_packet, packet_started.elapsed());
}

#[cfg(target_arch = "wasm32")]
fn warn_slow_packet(state: ConnectionProtocol, raw_packet: &[u8], took: std::time::Duration) {
    use std::sync::atomic::{AtomicU32, Ordering};

    const SLOW: std::time::Duration = std::time::Duration::from_millis(8);
    const MAX_WARNINGS: u32 = 64;
    static WARNED: AtomicU32 = AtomicU32::new(0);

    if took < SLOW || WARNED.load(Ordering::Relaxed) >= MAX_WARNINGS {
        return;
    }
    WARNED.fetch_add(1, Ordering::Relaxed);
    let mut id = 0u32;
    for (i, byte) in raw_packet.iter().take(5).enumerate() {
        id |= u32::from(byte & 0x7f) << (7 * i);
        if byte & 0x80 == 0 {
            break;
        }
    }
    tracing::warn!(
        "slow packet: {state:?} id {id:#04x}, {} bytes, {:.1} ms",
        raw_packet.len(),
        took.as_secs_f32() * 1000.0
    );
}

#[derive(Component)]
pub struct RawConnection {
    pub(crate) network: Option<NetworkConnection>,
    pub(crate) warned_no_network: bool,
    #[cfg(target_arch = "wasm32")]
    pub(crate) read_backlog: bool,
    #[cfg(target_arch = "wasm32")]
    pub(crate) read_lag_warned: bool,
    pub(crate) bundle: Option<Vec<Arc<[u8]>>>,
    pub(crate) bundle_overflowed: bool,
    pub state: ConnectionProtocol,
    pub(crate) is_alive: bool,

    pub injected_clientbound_packets: Vec<Box<[u8]>>,

    #[cfg(feature = "multiversion")]
    pub translator: Option<Box<dyn PacketTranslator>>,
}
impl RawConnection {
    pub fn new(
        reader: RawReadConnection,
        writer: RawWriteConnection,
        state: ConnectionProtocol,
    ) -> Self {
        let task_pool = IoTaskPool::get();

        let (network_packet_writer_tx, network_packet_writer_rx) =
            mpsc::unbounded_channel::<Box<[u8]>>();

        let writer_task =
            task_pool.spawn(write_task(network_packet_writer_rx, writer.write_stream));

        let mut conn = Self::new_networkless(state);
        conn.network = Some(NetworkConnection {
            reader,
            enc_cipher: writer.enc_cipher,
            network_packet_writer_tx,
            writer_task,
        });

        conn
    }

    pub fn new_networkless(state: ConnectionProtocol) -> Self {
        Self {
            network: None,
            warned_no_network: false,
            #[cfg(target_arch = "wasm32")]
            read_backlog: false,
            #[cfg(target_arch = "wasm32")]
            read_lag_warned: false,
            bundle: None,
            bundle_overflowed: false,
            state,
            is_alive: true,
            injected_clientbound_packets: Vec::new(),
            #[cfg(feature = "multiversion")]
            translator: None,
        }
    }

    pub fn is_alive(&self) -> bool {
        self.is_alive
    }

    pub fn write<P: ProtocolPacket + Debug>(
        &mut self,
        packet: impl Packet<P>,
    ) -> Result<(), WritePacketError> {
        if let Some(network) = &mut self.network {
            #[cfg(feature = "multiversion")]
            match self.translator.as_deref_mut() {
                Some(translator) => network.write_translated(packet, P::PHASE, translator)?,
                None => network.write(packet)?,
            }
            #[cfg(not(feature = "multiversion"))]
            network.write(packet)?;
        } else {
            static WARNED: AtomicBool = AtomicBool::new(false);
            if !WARNED.swap(true, atomic::Ordering::Relaxed) {
                debug!(
                    "tried to write packet to the network but there is no NetworkConnection. if you're trying to send a packet from the handler function, use self.write instead"
                );
            }
        }
        Ok(())
    }

    pub fn net_conn(&mut self) -> Option<&mut NetworkConnection> {
        self.network.as_mut()
    }
}

pub type GamePacketObserver = fn(&mut World, Entity, &ClientboundGamePacket);

static GAME_PACKET_OBSERVER: std::sync::OnceLock<GamePacketObserver> = std::sync::OnceLock::new();

pub fn set_game_packet_observer(observer: GamePacketObserver) -> bool {
    GAME_PACKET_OBSERVER.set(observer).is_ok()
}

pub fn handle_raw_packet(
    ecs: &mut World,
    raw_packet: &[u8],
    entity: Entity,
    state: ConnectionProtocol,
    queued_packet_events: &mut QueuedPacketEvents,
) -> Result<(), Box<ReadPacketError>> {
    let stream = &mut Cursor::new(raw_packet);
    match state {
        ConnectionProtocol::Handshake => {
            unreachable!()
        }
        ConnectionProtocol::Game => {
            let packet = Arc::new(traced(state, raw_packet, || {
                deserialize_packet::<ClientboundGamePacket>(stream)
            })?);
            trace!("Packet: {packet:?}");
            game::process_packet(ecs, entity, packet.as_ref());
            if let Some(observer) = GAME_PACKET_OBSERVER.get() {
                observer(ecs, entity, packet.as_ref());
            }
            queued_packet_events
                .game
                .push(ReceiveGamePacketEvent { entity, packet });
        }
        ConnectionProtocol::Status => {
            unreachable!()
        }
        ConnectionProtocol::Login => {
            let packet = Arc::new(traced(state, raw_packet, || {
                deserialize_packet::<ClientboundLoginPacket>(stream)
            })?);
            trace!("Packet: {packet:?}");
            login::process_packet(ecs, entity, &packet);
            queued_packet_events
                .login
                .push(ReceiveLoginPacketEvent { entity, packet });
        }
        ConnectionProtocol::Configuration => {
            let packet = Arc::new(traced(state, raw_packet, || {
                deserialize_packet::<ClientboundConfigPacket>(stream)
            })?);
            trace!("Packet: {packet:?}");
            config::process_packet(ecs, entity, &packet);
            queued_packet_events
                .config
                .push(ReceiveConfigPacketEvent { entity, packet });
        }
    };

    Ok(())
}

fn traced<P: ProtocolPacket>(
    state: ConnectionProtocol,
    raw_packet: &[u8],
    decode: impl FnOnce() -> Result<P, Box<ReadPacketError>>,
) -> Result<P, Box<ReadPacketError>> {
    #[cfg(not(feature = "packet-trace-recv"))]
    let _ = (state, raw_packet);

    let res = decode();

    #[cfg(feature = "packet-trace-recv")]
    if azalea_protocol::packet_trace::recv_hooked() {
        azalea_protocol::packet_trace::note_recv(
            state,
            res.as_ref().ok().map(|p| p.name()),
            raw_packet,
        );
    }
    res
}

pub struct NetworkConnection {
    reader: RawReadConnection,
    pub enc_cipher: Option<Aes128CfbEnc>,

    pub writer_task: bevy_tasks::Task<()>,
    network_packet_writer_tx: mpsc::UnboundedSender<Box<[u8]>>,
}
impl NetworkConnection {
    pub fn write<P: ProtocolPacket + Debug>(
        &mut self,
        packet: impl Packet<P>,
    ) -> Result<(), WritePacketError> {
        let packet = packet.into_variant();
        let raw_packet = serialize_packet(&packet)?;
        #[cfg(feature = "packet-trace")]
        azalea_protocol::packet_trace::note_sent(P::PHASE, packet.name(), &raw_packet);
        self.write_raw(&raw_packet)?;

        Ok(())
    }

    #[cfg(feature = "multiversion")]
    pub fn write_translated<P: ProtocolPacket + Debug>(
        &mut self,
        packet: impl Packet<P>,
        phase: ConnectionProtocol,
        translator: &mut (dyn PacketTranslator + 'static),
    ) -> Result<(), WritePacketError> {
        let packet = packet.into_variant();
        let raw_packet = serialize_packet(&packet)?;
        let raw_packet = translator.outbound(phase, raw_packet);
        if raw_packet.is_empty() {
            return Ok(());
        }
        #[cfg(feature = "packet-trace")]
        azalea_protocol::packet_trace::note_sent(phase, packet.name(), &raw_packet);
        self.write_raw(&raw_packet)?;

        Ok(())
    }

    pub fn write_raw(&mut self, raw_packet: &[u8]) -> Result<(), WritePacketError> {
        let network_packet = azalea_protocol::write::encode_to_network_packet(
            raw_packet,
            self.reader.compression_threshold,
            &mut self.enc_cipher,
        );
        self.network_packet_writer_tx
            .send(network_packet.into_boxed_slice())?;
        Ok(())
    }

    pub fn poll_writer(&mut self) -> Option<()> {
        let poll_once_res = future::poll_once(&mut self.writer_task);
        future::block_on(poll_once_res)
    }

    pub fn set_compression_threshold(&mut self, threshold: Option<u32>) {
        trace!("Set compression threshold to {threshold:?}");
        self.reader.compression_threshold = threshold;
    }
    pub fn set_encryption_key(&mut self, key: [u8; 16]) {
        trace!("Enabled protocol encryption");
        let (enc_cipher, dec_cipher) = azalea_crypto::create_cipher(&key);
        self.reader.dec_cipher = Some(dec_cipher);
        self.enc_cipher = Some(enc_cipher);
    }
}

async fn write_task(
    mut network_packet_writer_rx: mpsc::UnboundedReceiver<Box<[u8]>>,
    mut write_half: azalea_protocol::connect::BoxedWriteStream,
) {
    while let Some(network_packet) = network_packet_writer_rx.recv().await {
        if let Err(e) = write_half.write_all(&network_packet).await {
            debug!("Error writing packet to server: {e}");
            break;
        };
    }

    trace!("write task is done");
}

#[derive(Debug, Error)]
pub enum WritePacketError {
    #[error("Wrong protocol state: expected {expected:?}, got {got:?}")]
    WrongState {
        expected: ConnectionProtocol,
        got: ConnectionProtocol,
    },
    #[error(transparent)]
    Encoding(#[from] azalea_protocol::write::PacketEncodeError),
    #[error(transparent)]
    SendError {
        #[from]
        #[backtrace]
        source: mpsc::error::SendError<Box<[u8]>>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundle_delimiter_frame_is_one_byte() {
        let packet = ClientboundGamePacket::BundleDelimiter(ClientboundBundleDelimiter);
        let byte = bundle_delimiter_byte().expect("delimiter id fits one VarInt byte");
        assert_eq!(ClientboundGamePacket::NAMES[byte as usize], "bundle_delimiter");
        let mut body = Vec::new();
        packet.write(&mut body).unwrap();
        assert!(body.is_empty());
    }
}
