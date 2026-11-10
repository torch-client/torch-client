use std::{
    io,
    pin::Pin,
    sync::Mutex,
    task::{Context, Poll},
};

use azalea_auth::game_profile::GameProfile;
use azalea_protocol::{
    connect::{Connection, ConnectionError, transport},
    packets::login::ClientboundLoginFinished,
};
use tokio::{
    io::{AsyncRead, AsyncWrite, ReadBuf},
    sync::{
        mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel},
        oneshot,
    },
};

use super::socket::{Frame, Rx, Socket, Tx};
use super::wisp;
use crate::util::wire::{Reader, write_varint};
use crate::{log_error, log_info, log_warn};

static PREPARED: Mutex<Option<transport::HandshakeConn>> = Mutex::new(None);

pub(super) fn prepare(socket: Socket, profile: GameProfile, eagler_version: u16) {
    let bridge = spawn_bridge(socket, Some(profile), None);
    #[cfg(eagler_skins)]
    super::skins::begin(eagler_version, bridge.to_server.clone());
    #[cfg(not(eagler_skins))]
    let _ = eagler_version;
    park(Connection::wrap_stream(bridge));
}

pub(super) fn prepare_raw(socket: Socket, wisp: Option<wisp::Stream>) {
    park(raw_connection(socket, wisp));
}

pub(super) fn raw_connection(
    socket: Socket,
    wisp: Option<wisp::Stream>,
) -> transport::HandshakeConn {
    Connection::wrap_stream(spawn_bridge(socket, None, wisp))
}

fn spawn_bridge(
    socket: Socket,
    profile: Option<GameProfile>,
    wisp: Option<wisp::Stream>,
) -> Bridge {
    let raw = profile.is_none();
    let (to_server, outgoing) = unbounded_channel();
    let (from_server, incoming) = unbounded_channel();
    let (open_gate, gate) = oneshot::channel();
    let login = if raw {
        let _ = open_gate.send(());
        Login::Done
    } else {
        Login::Intention(open_gate)
    };

    let (tx, rx) = socket.split();
    let pump_side = from_server.clone();
    crate::platform::executor::spawn_task(async move {
        let reason = match pump(tx, rx, outgoing, pump_side.clone(), gate, raw, wisp).await {
            Ok(()) => None,
            Err(e) => {
                log_error!("net", "eagler: the connection stopped: {e}");
                Some(e.to_string())
            }
        };
        let _ = pump_side.send(FromServer::Ended(reason));
    });

    Bridge {
        to_server,
        incoming,
        inject: from_server,
        pending: Vec::new(),
        taken: 0,
        partial: Vec::new(),
        login,
        profile: profile.unwrap_or_default(),
        raw,
        closed: false,
    }
}

fn park(connection: transport::HandshakeConn) {
    *PREPARED.lock().unwrap() = Some(connection);

    transport::set_opener(Box::new(|_address| {
        Box::pin(async move {
            PREPARED.lock().unwrap().take().ok_or_else(|| {
                ConnectionError::Io(io::Error::other(
                    "the eagler transport was asked for a connection it had not been given one \
                     for; `eagler::prepare` runs before azalea is started, and only once per join",
                ))
            })
        })
    }));
}

enum FromServer {
    Bytes(Vec<u8>),
    Ended(Option<String>),
}

async fn pump(
    mut tx: Tx,
    mut rx: Rx,
    mut outgoing: UnboundedReceiver<Vec<u8>>,
    incoming: UnboundedSender<FromServer>,
    gate: oneshot::Receiver<()>,
    raw: bool,
    mut wisp: Option<wisp::Stream>,
) -> eyre::Result<()> {
    if gate.await.is_err() {
        return Ok(());
    }
    if !raw {
        log_info!("net", "eagler: past the faked login, relaying packets");
    }

    loop {
        tokio::select! {
            body = outgoing.recv(), if wisp.as_ref().is_none_or(wisp::Stream::can_send) => {
                let Some(body) = body else {
                    log_info!("net", "eagler: azalea closed the connection");
                    return Ok(());
                };
                let body = match wisp.as_mut() {
                    Some(stream) => stream.data(&body),
                    None => body,
                };
                tx.send_binary(body).await?;
            }
            message = rx.recv() => {
                let frame = message?.unwrap_or(Frame::Close(None));
                let frame = match (frame, wisp.as_mut()) {
                    (Frame::Binary(message), Some(stream)) => match stream.receive(&message) {
                        wisp::Inbound::Data(bytes) => Frame::Binary(bytes),
                        wisp::Inbound::Closed(reason) => eyre::bail!("{reason}"),
                        wisp::Inbound::Nothing => Frame::Other,
                    },
                    (frame, _) => frame,
                };
                match frame {
                    Frame::Binary(payload) if raw => {
                        if incoming.send(FromServer::Bytes(payload)).is_err() {
                            log_info!("net", "eagler: azalea dropped the connection");
                            return Ok(());
                        }
                    }
                    Frame::Binary(payload) if is_injected(&payload) => {
                        note_injected(&payload);
                        #[cfg(eagler_skins)]
                        super::skins::receive_injected(&payload);
                    }
                    Frame::Binary(payload) => {
                        for body in unprefixed(payload) {
                            if incoming.send(FromServer::Bytes(body)).is_err() {
                                log_info!("net", "eagler: azalea dropped the connection");
                                return Ok(());
                            }
                        }
                    }
                    Frame::Close(reason) => {
                        match reason {
                            Some(reason) => log_warn!(
                                "net",
                                "eagler: the server closed the websocket: {reason}"
                            ),
                            None => log_info!("net", "eagler: the server closed the websocket"),
                        }
                        return Ok(());
                    }
                    Frame::Text(_) | Frame::Other => {}
                }
            }
        }
    }
}

fn unprefixed(payload: Vec<u8>) -> Vec<Vec<u8>> {
    let first = match read_varint(&payload) {
        Some((first, _)) if first > NOT_AN_ID => first,
        _ => return vec![payload],
    };

    let mut bodies = Vec::new();
    let mut at = 0;
    while at < payload.len() {
        let Some((len, header)) = read_varint(&payload[at..]) else {
            return unwrappable(payload, first, at, "a length is not a whole VarInt");
        };
        let start = at + header;
        let Some(end) = start
            .checked_add(len as usize)
            .filter(|end| *end <= payload.len())
        else {
            return unwrappable(
                payload,
                first,
                at,
                "a length runs past the end of the frame",
            );
        };
        bodies.push(payload[start..end].to_vec());
        at = end;
    }

    static SAID: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if !SAID.swap(true, std::sync::atomic::Ordering::Relaxed) {
        log_info!(
            "net",
            "eagler: this server sends some frames with the vanilla length prefix still on them \
             ({} bytes, {} packet(s) in one frame); unwrapping them",
            payload.len(),
            bodies.len()
        );
    }
    bodies
}

const INJECTED: u8 = 0xEE;

fn is_injected(payload: &[u8]) -> bool {
    payload.first() == Some(&INJECTED)
}

fn note_injected(payload: &[u8]) {
    static SEEN: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let nth = SEEN.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    if nth == 0 {
        let multi = payload.get(1) == Some(&0xFF);
        log_info!(
            "net",
            "eagler: this server sends EaglerX messages of its own over the game socket \
             (first was {} bytes, {}); reading skin replies, dropping the rest",
            payload.len(),
            if multi {
                "a multi-packet"
            } else {
                "one packet"
            }
        );
    }
}

fn unwrappable(payload: Vec<u8>, first: u32, at: usize, why: &str) -> Vec<Vec<u8>> {
    const WHOLE: usize = 256;

    static SAID: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if !SAID.swap(true, std::sync::atomic::Ordering::Relaxed) {
        let shown = payload.len().min(WHOLE);
        log_warn!(
            "net",
            "eagler: a {} byte frame opens with {first}, which is no packet id, but {why} \
             after {at} bytes. Passing it to azalea, which will call {first} an unknown packet \
             id. {}: {:02X?}",
            payload.len(),
            if shown == payload.len() {
                "Whole frame"
            } else {
                "First bytes"
            },
            &payload[..shown]
        );
    }
    vec![payload]
}

enum Login {
    Intention(oneshot::Sender<()>),
    Hello(oneshot::Sender<()>),
    Acknowledged(oneshot::Sender<()>),
    Done,
}

const NOT_AN_ID: u32 = 0xFF;

const ID_INTENTION: u32 = 0x00;
const ID_HELLO: u32 = 0x00;
const ID_LOGIN_ACKNOWLEDGED: u32 = 0x03;

pub(super) struct Bridge {
    to_server: UnboundedSender<Vec<u8>>,
    incoming: UnboundedReceiver<FromServer>,
    inject: UnboundedSender<FromServer>,
    pending: Vec<u8>,
    taken: usize,
    partial: Vec<u8>,
    login: Login,
    profile: GameProfile,
    raw: bool,
    closed: bool,
}

impl AsyncRead for Bridge {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let me = &mut *self;
        if me.taken == me.pending.len() {
            if me.closed {
                return Poll::Ready(Ok(()));
            }
            match me.incoming.poll_recv(cx) {
                Poll::Ready(Some(FromServer::Bytes(body))) => {
                    me.pending = if me.raw { body } else { framed(&body) };
                    me.taken = 0;
                }
                Poll::Ready(Some(FromServer::Ended(reason))) => {
                    me.closed = true;
                    return Poll::Ready(reason.map_or(Ok(()), |r| Err(io::Error::other(r))));
                }
                Poll::Ready(None) => {
                    me.closed = true;
                    return Poll::Ready(Ok(()));
                }
                Poll::Pending => return Poll::Pending,
            }
        }
        let n = buf.remaining().min(me.pending.len() - me.taken);
        buf.put_slice(&me.pending[me.taken..me.taken + n]);
        me.taken += n;
        Poll::Ready(Ok(()))
    }
}

impl AsyncWrite for Bridge {
    fn poll_write(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let me = &mut *self;
        if me.raw {
            me.to_server.send(buf.to_vec()).map_err(|_| {
                io::Error::new(io::ErrorKind::BrokenPipe, "the tunnel connection is gone")
            })?;
            return Poll::Ready(Ok(buf.len()));
        }
        me.partial.extend_from_slice(buf);
        while let Some((len, header)) = read_varint(&me.partial) {
            let len = len as usize;
            if me.partial.len() < header + len {
                break;
            }
            let body = me.partial[header..header + len].to_vec();
            me.partial.drain(..header + len);
            if let Err(e) = me.absorb(body) {
                return Poll::Ready(Err(e));
            }
        }
        Poll::Ready(Ok(buf.len()))
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.closed = true;
        Poll::Ready(Ok(()))
    }
}

impl Bridge {
    fn absorb(&mut self, body: Vec<u8>) -> io::Result<()> {
        let expected = match self.login {
            Login::Intention(_) => Some((ID_INTENTION, "intention")),
            Login::Hello(_) => Some((ID_HELLO, "hello")),
            Login::Acknowledged(_) => Some((ID_LOGIN_ACKNOWLEDGED, "login acknowledged")),
            Login::Done => None,
        };
        let Some((id, name)) = expected else {
            return self.to_server.send(body).map_err(|_| {
                io::Error::new(io::ErrorKind::BrokenPipe, "the eagler connection is gone")
            });
        };

        match read_varint(&body) {
            Some((got, _)) if got == id => {}
            Some((got, _)) => {
                return Err(io::Error::other(format!(
                    "eagler: expected azalea's {name} packet (0x{id:02x}) during the faked login, \
                     got 0x{got:02x}. The login phase this bridge absorbs has changed; see \
                     src/eagler/stream.rs."
                )));
            }
            None => {
                return Err(io::Error::other(
                    "eagler: azalea sent a packet with no id while logging in",
                ));
            }
        }

        if let Login::Hello(_) = self.login {
            let packet = azalea_protocol::packets::Packet::into_variant(ClientboundLoginFinished {
                game_profile: self.profile.clone(),
                session_id: uuid::Uuid::nil(),
            });
            let raw = azalea_protocol::write::serialize_packet(&packet)
                .map_err(|e| io::Error::other(format!("eagler: {e}")))?;
            self.inject
                .send(FromServer::Bytes(raw.into_vec()))
                .map_err(|_| {
                    io::Error::new(io::ErrorKind::BrokenPipe, "the eagler connection is gone")
                })?;
        }

        self.login = match std::mem::replace(&mut self.login, Login::Done) {
            Login::Intention(gate) => Login::Hello(gate),
            Login::Hello(gate) => Login::Acknowledged(gate),
            Login::Acknowledged(gate) => {
                let _ = gate.send(());
                Login::Done
            }
            Login::Done => unreachable!("handled above"),
        };
        Ok(())
    }
}

fn read_varint(buf: &[u8]) -> Option<(u32, usize)> {
    let mut r = Reader::new(buf);
    let value = r.varint()? as u32;
    Some((value, r.pos()))
}

fn framed(body: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(body.len() + 5);
    write_varint(&mut out, body.len() as i32);
    out.extend_from_slice(body);
    out
}

#[cfg(test)]
mod tests {
    use super::{framed, read_varint, write_varint};

    #[test]
    fn an_ordinary_frame_is_one_packet_and_is_left_alone() {
        let frame = vec![0x0a, 0x01, 0x02, 0x03];
        assert_eq!(super::unprefixed(frame.clone()), vec![frame]);
    }

    #[test]
    fn a_frame_that_kept_its_length_prefix_is_unwrapped() {
        let body: Vec<u8> = (0..300u32).map(|i| i as u8).collect();
        assert_eq!(super::unprefixed(super::framed(&body)), vec![body]);
    }

    #[test]
    fn several_prefixed_packets_in_one_frame_come_out_separately() {
        let first: Vec<u8> = vec![7u8; 300];
        let second: Vec<u8> = vec![9u8; 5];
        let mut frame = super::framed(&first);
        frame.extend_from_slice(&super::framed(&second));
        assert_eq!(super::unprefixed(frame), vec![first, second]);
    }

    #[test]
    fn a_frame_that_is_neither_shape_reaches_azalea_unchanged() {
        let frame = vec![0xAC, 0x02, 0x01];
        assert_eq!(super::unprefixed(frame.clone()), vec![frame]);
    }

    #[test]
    fn an_eaglerx_multi_packet_is_not_a_minecraft_frame() {
        let mut frame = vec![0xEE, 0xFF, 0x02];
        for _ in 0..2 {
            frame.push(18);
            frame.extend(std::iter::repeat_n(0xABu8, 18));
        }
        assert_eq!(frame.len(), 41, "the frame that was actually observed");
        assert!(super::is_injected(&frame));
        assert!(super::is_injected(&[0xEE, 0x01, 0x02]));
    }

    #[test]
    fn no_packet_id_can_look_like_an_eaglerx_message() {
        for id in 0..=255u32 {
            let mut out = Vec::new();
            write_varint(&mut out, id as i32);
            assert_ne!(out[0], super::INJECTED, "id {id} collides with the marker");
        }
    }

    #[test]
    fn a_run_of_packets_with_something_after_it_is_left_alone() {
        let first: Vec<u8> = vec![7u8; 300];
        let second: Vec<u8> = vec![9u8; 5];
        let mut frame = super::framed(&first);
        frame.extend_from_slice(&super::framed(&second));
        frame.push(0x7F);
        assert_eq!(super::unprefixed(frame.clone()), vec![frame]);
    }

    #[test]
    fn a_two_byte_packet_id_is_not_mistaken_for_a_length() {
        let mut frame = Vec::new();
        super::write_varint(&mut frame, 0x8c);
        frame.extend(std::iter::repeat_n(0u8, 0x8c));
        assert_eq!(super::unprefixed(frame.clone()), vec![frame]);
    }

    #[test]
    fn framing_prefixes_the_length_and_nothing_else() {
        assert_eq!(framed(&[0x01, 0x02]), vec![0x02, 0x01, 0x02]);
        let long = vec![0u8; 300];
        assert_eq!(&framed(&long)[..2], &[0xAC, 0x02]);
    }
}
