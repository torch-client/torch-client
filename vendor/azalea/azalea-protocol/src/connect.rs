use std::{
    fmt::{self, Debug, Display},
    io::{self, Cursor},
    marker::PhantomData,
    net::SocketAddr,
    time::Duration,
};

#[cfg(feature = "online-mode")]
use azalea_auth::{
    game_profile::GameProfile,
    sessionserver::{ClientSessionServerError, ServerSessionServerError},
};
use azalea_crypto::{Aes128CfbDec, Aes128CfbEnc};
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
#[cfg(not(target_arch = "wasm32"))]
use tokio::{io::BufStream, net::TcpStream};
use tracing::{error, info};
#[cfg(feature = "online-mode")]
use uuid::Uuid;

#[cfg(feature = "online-mode")]
use crate::packets::login::ClientboundHello;
use crate::{
    packets::{
        ProtocolPacket,
        config::{ClientboundConfigPacket, ServerboundConfigPacket},
        game::{ClientboundGamePacket, ServerboundGamePacket},
        handshake::{ClientboundHandshakePacket, ServerboundHandshakePacket},
        login::{ClientboundLoginPacket, ServerboundLoginPacket},
        status::{ClientboundStatusPacket, ServerboundStatusPacket},
    },
    read::{ReadPacketError, deserialize_packet, read_raw_packet, try_read_raw_packet},
    write::{serialize_packet, write_raw_packet, write_raw_packets},
};

pub type BoxedReadStream = Box<dyn AsyncRead + Unpin + Send + Sync>;

pub type BoxedWriteStream = Box<dyn AsyncWrite + Unpin + Send>;

#[cfg(unix)]
pub static LAST_TCP_FD: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(-1);

#[cfg(unix)]
pub fn force_close_last_tcp() {
    use std::sync::atomic::Ordering;

    let fd = LAST_TCP_FD.swap(-1, Ordering::SeqCst);
    if fd < 0 {
        return;
    }
    unsafe extern "C" {
        fn shutdown(socket: i32, how: i32) -> i32;
    }
    const SHUT_RDWR: i32 = 2;
    unsafe {
        shutdown(fd, SHUT_RDWR);
    }
}

#[cfg(unix)]
pub fn forget_last_tcp() {
    LAST_TCP_FD.store(-1, std::sync::atomic::Ordering::SeqCst);
}

pub struct RawReadConnection {
    pub read_stream: BoxedReadStream,
    pub buffer: Cursor<Vec<u8>>,
    pub compression_threshold: Option<u32>,
    pub dec_cipher: Option<Aes128CfbDec>,
}

pub struct RawWriteConnection {
    pub write_stream: BoxedWriteStream,
    pub compression_threshold: Option<u32>,
    pub enc_cipher: Option<Aes128CfbEnc>,
}

pub struct ReadConnection<R: ProtocolPacket> {
    pub raw: RawReadConnection,
    _reading: PhantomData<R>,
}

pub struct WriteConnection<W: ProtocolPacket> {
    pub raw: RawWriteConnection,
    _writing: PhantomData<W>,
}

pub struct Connection<R: ProtocolPacket, W: ProtocolPacket> {
    pub reader: ReadConnection<R>,
    pub writer: WriteConnection<W>,
}

pub mod transport {
    use std::{future::Future, pin::Pin, sync::OnceLock};

    use super::{
        ClientboundHandshakePacket, Connection, ConnectionError, ServerboundHandshakePacket,
    };
    use crate::address::ResolvedAddr;

    pub type HandshakeConn = Connection<ClientboundHandshakePacket, ServerboundHandshakePacket>;

    type Opener = Box<
        dyn Fn(
                ResolvedAddr,
            )
                -> Pin<Box<dyn Future<Output = Result<HandshakeConn, ConnectionError>> + Send>>
            + Send
            + Sync,
    >;

    static OPENER: OnceLock<Opener> = OnceLock::new();

    pub fn set_opener(opener: Opener) {
        let _ = OPENER.set(opener);
    }

    pub fn has_opener() -> bool {
        OPENER.get().is_some()
    }

    pub async fn open(address: ResolvedAddr) -> Result<HandshakeConn, ConnectionError> {
        match OPENER.get() {
            Some(open) => open(address).await,
            None => Err(ConnectionError::Io(std::io::Error::other(
                "no transport was installed, and this target has no TCP sockets of its own",
            ))),
        }
    }
}

impl RawReadConnection {
    pub async fn read(&mut self) -> Result<Box<[u8]>, Box<ReadPacketError>> {
        read_raw_packet::<_>(
            &mut self.read_stream,
            &mut self.buffer,
            self.compression_threshold,
            &mut self.dec_cipher,
        )
        .await
    }

    pub fn try_read(&mut self) -> Result<Option<Box<[u8]>>, Box<ReadPacketError>> {
        try_read_raw_packet::<_>(
            &mut self.read_stream,
            &mut self.buffer,
            self.compression_threshold,
            &mut self.dec_cipher,
        )
    }
}

impl RawWriteConnection {
    pub async fn write(&mut self, packet: &[u8]) -> io::Result<()> {
        if let Err(e) = write_raw_packet(
            packet,
            &mut self.write_stream,
            self.compression_threshold,
            &mut self.enc_cipher,
        )
        .await
        {
            if e.kind() == io::ErrorKind::BrokenPipe {
                info!("Broken pipe, shutting down connection.");
                if let Err(e) = self.shutdown().await {
                    error!("Couldn't shut down: {}", e);
                }
            }
            return Err(e);
        }
        Ok(())
    }

    pub async fn write_batch(&mut self, packets: impl Iterator<Item = &[u8]>) -> io::Result<()> {
        if let Err(e) = write_raw_packets(
            packets,
            &mut self.write_stream,
            self.compression_threshold,
            &mut self.enc_cipher,
        )
        .await
        {
            if e.kind() == io::ErrorKind::BrokenPipe {
                info!("Broken pipe, shutting down connection.");
                if let Err(e) = self.shutdown().await {
                    error!("Couldn't shut down: {}", e);
                }
            }
            return Err(e);
        }
        Ok(())
    }

    pub async fn shutdown(&mut self) -> io::Result<()> {
        self.write_stream.shutdown().await
    }
}

impl<R> ReadConnection<R>
where
    R: ProtocolPacket + Debug,
{
    pub async fn read(&mut self) -> Result<R, Box<ReadPacketError>> {
        let raw_packet = self.raw.read().await?;
        deserialize_packet(&mut Cursor::new(&raw_packet))
    }

    pub fn try_read(&mut self) -> Result<Option<R>, Box<ReadPacketError>> {
        let Some(raw_packet) = self.raw.try_read()? else {
            return Ok(None);
        };
        Ok(Some(deserialize_packet(&mut Cursor::new(&raw_packet))?))
    }
}
impl<W> WriteConnection<W>
where
    W: ProtocolPacket + Debug,
{
    pub async fn write(&mut self, packet: W) -> io::Result<()> {
        self.raw.write(&serialize_packet(&packet).unwrap()).await
    }

    pub async fn write_batch(&mut self, packets: &[W]) -> io::Result<()> {
        let serialized_packets: Vec<Box<[u8]>> = packets
            .into_iter()
            .map(|packet| serialize_packet(packet).unwrap())
            .collect();
        self.raw
            .write_batch(serialized_packets.iter().map(|data| data.as_ref()))
            .await
    }

    pub async fn shutdown(&mut self) -> io::Result<()> {
        self.raw.shutdown().await
    }
}

impl<R, W> Connection<R, W>
where
    R: ProtocolPacket + Debug,
    W: ProtocolPacket + Debug,
{
    pub fn wrap_stream<S>(stream: S) -> Self
    where
        S: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    {
        let (read_stream, write_stream) = tokio::io::split(stream);

        Connection {
            reader: ReadConnection {
                raw: RawReadConnection {
                    read_stream: Box::new(read_stream),
                    buffer: Cursor::new(Vec::new()),
                    compression_threshold: None,
                    dec_cipher: None,
                },
                _reading: PhantomData,
            },
            writer: WriteConnection {
                raw: RawWriteConnection {
                    write_stream: Box::new(write_stream),
                    compression_threshold: None,
                    enc_cipher: None,
                },
                _writing: PhantomData,
            },
        }
    }

    pub async fn read(&mut self) -> Result<R, Box<ReadPacketError>> {
        self.reader.read().await
    }

    pub fn try_read(&mut self) -> Result<Option<R>, Box<ReadPacketError>> {
        self.reader.try_read()
    }

    pub async fn write(&mut self, packet: impl crate::packets::Packet<W>) -> io::Result<()> {
        let packet = packet.into_variant();
        self.writer.write(packet).await
    }

    pub async fn write_batch(&mut self, packets: &[W]) -> io::Result<()> {
        self.writer.write_batch(packets).await
    }

    #[must_use]
    pub fn into_split(self) -> (ReadConnection<R>, WriteConnection<W>) {
        (self.reader, self.writer)
    }

    #[must_use]
    pub fn into_split_raw(self) -> (RawReadConnection, RawWriteConnection) {
        (self.reader.raw, self.writer.raw)
    }
}

#[derive(Debug, Error)]
pub enum ConnectionError {
    #[error("{0}")]
    Io(#[from] io::Error),
}

#[cfg(not(target_arch = "wasm32"))]
use socks5_impl::protocol::UserKey;

#[cfg(target_arch = "wasm32")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UserKey {
    pub username: String,
    pub password: String,
}

#[cfg(target_arch = "wasm32")]
impl Display for UserKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.username, self.password)
    }
}

#[derive(Clone, Debug)]
pub struct Proxy {
    pub addr: SocketAddr,
    pub auth: Option<UserKey>,
}

impl Proxy {
    pub fn new(addr: SocketAddr, auth: Option<UserKey>) -> Self {
        Self { addr, auth }
    }
}
impl Display for Proxy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "socks5://")?;
        if let Some(auth) = &self.auth {
            write!(f, "{auth}@")?;
        }
        write!(f, "{}", self.addr)
    }
}

#[cfg(all(feature = "online-mode", not(target_arch = "wasm32")))]
impl From<Proxy> for reqwest::Proxy {
    fn from(proxy: Proxy) -> Self {
        reqwest::Proxy::all(proxy.to_string())
            .expect("azalea proxies should not fail to parse as reqwest proxies")
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn connect_timeout() -> Option<Duration> {
    use std::sync::OnceLock;
    static SECS: OnceLock<Option<Duration>> = OnceLock::new();
    *SECS.get_or_init(|| {
        let secs = std::env::var("MC_CONNECT_TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(10);
        (secs > 0).then(|| Duration::from_secs(secs))
    })
}

#[cfg(not(target_arch = "wasm32"))]
async fn connect_tcp(address: &SocketAddr) -> Result<TcpStream, ConnectionError> {
    let connect = TcpStream::connect(address);
    let stream = match connect_timeout() {
        Some(timeout) => {
            tokio::time::timeout(timeout, connect)
                .await
                .map_err(|_| {
                    io::Error::new(
                        io::ErrorKind::TimedOut,
                        format!("connect to {address} timed out after {timeout:?}"),
                    )
                })??
        }
        None => connect.await?,
    };
    Ok(stream)
}

impl Connection<ClientboundHandshakePacket, ServerboundHandshakePacket> {
    #[cfg(not(target_arch = "wasm32"))]
    pub async fn new(address: &SocketAddr) -> Result<Self, ConnectionError> {
        let stream = connect_tcp(address).await?;

        stream.set_nodelay(true)?;

        Self::new_from_stream(stream).await
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub async fn new_with_proxy(
        address: &SocketAddr,
        proxy: Proxy,
    ) -> Result<Self, ConnectionError> {
        let proxy_stream = connect_tcp(&proxy.addr).await?;
        let mut stream = BufStream::new(proxy_stream);

        let _ = socks5_impl::client::connect(&mut stream, address, proxy.auth)
            .await
            .map_err(io::Error::other)?;

        Self::new_from_stream(stream.into_inner()).await
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub async fn new_from_stream(stream: TcpStream) -> Result<Self, ConnectionError> {
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd as _;
            LAST_TCP_FD.store(stream.as_raw_fd(), std::sync::atomic::Ordering::SeqCst);
        }
        Ok(Self::wrap_stream(stream))
    }

    #[must_use]
    pub fn login(self) -> Connection<ClientboundLoginPacket, ServerboundLoginPacket> {
        Connection::from(self)
    }

    #[must_use]
    pub fn status(self) -> Connection<ClientboundStatusPacket, ServerboundStatusPacket> {
        Connection::from(self)
    }
}

impl Connection<ClientboundLoginPacket, ServerboundLoginPacket> {
    pub fn set_compression_threshold(&mut self, threshold: i32) {
        if threshold >= 0 {
            self.reader.raw.compression_threshold = Some(threshold as u32);
            self.writer.raw.compression_threshold = Some(threshold as u32);
        } else {
            self.reader.raw.compression_threshold = None;
            self.writer.raw.compression_threshold = None;
        }
    }

    pub fn set_encryption_key(&mut self, key: [u8; 16]) {
        let (enc_cipher, dec_cipher) = azalea_crypto::create_cipher(&key);
        self.reader.raw.dec_cipher = Some(dec_cipher);
        self.writer.raw.enc_cipher = Some(enc_cipher);
    }

    #[must_use]
    pub fn config(self) -> Connection<ClientboundConfigPacket, ServerboundConfigPacket> {
        Connection::from(self)
    }

    #[cfg(feature = "online-mode")]
    pub async fn authenticate(
        &self,
        access_token: &str,
        uuid: &Uuid,
        private_key: [u8; 16],
        packet: &ClientboundHello,
        sessionserver_proxy: Option<Proxy>,
    ) -> Result<(), ClientSessionServerError> {
        use azalea_auth::sessionserver::{self, SessionServerJoinOpts};

        sessionserver::join(SessionServerJoinOpts {
            access_token,
            public_key: &packet.public_key,
            private_key: &private_key,
            uuid,
            server_id: &packet.server_id,
            proxy: sessionserver_proxy.map(Proxy::into),
        })
        .await
    }
}

impl Connection<ServerboundHandshakePacket, ClientboundHandshakePacket> {
    #[must_use]
    pub fn login(self) -> Connection<ServerboundLoginPacket, ClientboundLoginPacket> {
        Connection::from(self)
    }

    #[must_use]
    pub fn status(self) -> Connection<ServerboundStatusPacket, ClientboundStatusPacket> {
        Connection::from(self)
    }
}

impl Connection<ServerboundLoginPacket, ClientboundLoginPacket> {
    pub fn set_compression_threshold(&mut self, threshold: i32) {
        if threshold >= 0 {
            self.reader.raw.compression_threshold = Some(threshold as u32);
            self.writer.raw.compression_threshold = Some(threshold as u32);
        } else {
            self.reader.raw.compression_threshold = None;
            self.writer.raw.compression_threshold = None;
        }
    }

    pub fn set_encryption_key(&mut self, key: [u8; 16]) {
        let (enc_cipher, dec_cipher) = azalea_crypto::create_cipher(&key);
        self.reader.raw.dec_cipher = Some(dec_cipher);
        self.writer.raw.enc_cipher = Some(enc_cipher);
    }

    #[must_use]
    pub fn game(self) -> Connection<ServerboundGamePacket, ClientboundGamePacket> {
        Connection::from(self)
    }

    #[cfg(feature = "online-mode")]
    pub async fn authenticate(
        &self,
        username: &str,
        public_key: &[u8],
        private_key: &[u8; 16],
        ip: Option<&str>,
    ) -> Result<GameProfile, ServerSessionServerError> {
        azalea_auth::sessionserver::serverside_auth(username, public_key, private_key, ip).await
    }

    #[must_use]
    pub fn config(self) -> Connection<ServerboundConfigPacket, ClientboundConfigPacket> {
        Connection::from(self)
    }
}

impl Connection<ServerboundConfigPacket, ClientboundConfigPacket> {
    #[must_use]
    pub fn game(self) -> Connection<ServerboundGamePacket, ClientboundGamePacket> {
        Connection::from(self)
    }
}

impl Connection<ClientboundConfigPacket, ServerboundConfigPacket> {
    #[must_use]
    pub fn game(self) -> Connection<ClientboundGamePacket, ServerboundGamePacket> {
        Connection::from(self)
    }
}

impl Connection<ClientboundGamePacket, ServerboundGamePacket> {
    #[must_use]
    pub fn config(self) -> Connection<ClientboundConfigPacket, ServerboundConfigPacket> {
        Connection::from(self)
    }
}
impl Connection<ServerboundGamePacket, ClientboundGamePacket> {
    #[must_use]
    pub fn config(self) -> Connection<ServerboundConfigPacket, ClientboundConfigPacket> {
        Connection::from(self)
    }
}

impl<R1, W1> Connection<R1, W1>
where
    R1: ProtocolPacket + Debug,
    W1: ProtocolPacket + Debug,
{
    #[must_use]
    pub fn from<R2, W2>(connection: Connection<R1, W1>) -> Connection<R2, W2>
    where
        R2: ProtocolPacket + Debug,
        W2: ProtocolPacket + Debug,
    {
        Connection {
            reader: ReadConnection {
                raw: connection.reader.raw,
                _reading: PhantomData,
            },
            writer: WriteConnection {
                raw: connection.writer.raw,
                _writing: PhantomData,
            },
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn wrap(stream: TcpStream) -> Connection<R1, W1> {
        Connection::wrap_stream(stream)
    }

    pub fn into_halves(self) -> (RawReadConnection, RawWriteConnection) {
        (self.reader.raw, self.writer.raw)
    }
}
