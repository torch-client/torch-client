use std::{
    backtrace::Backtrace,
    env,
    fmt::Debug,
    io,
    io::{Cursor, Read},
    sync::LazyLock,
};

use azalea_buf::{AzBufVar, BufReadError};
use azalea_crypto::Aes128CfbDec;
use flate2::read::ZlibDecoder;
use futures::StreamExt;
use futures_lite::future;
use thiserror::Error;
use tokio::io::AsyncRead;
use tokio_util::{
    bytes::Buf,
    codec::{BytesCodec, FramedRead},
};
use tracing::trace;

use crate::packets::ProtocolPacket;

#[derive(Debug, Error)]
pub enum ReadPacketError {
    #[error("Error reading packet {packet_name} (id {packet_id}): {source}")]
    Parse {
        packet_id: u32,
        packet_name: String,
        backtrace: Box<Backtrace>,
        source: BufReadError,
    },
    #[error("Unknown packet id {id} in state {state_name}")]
    UnknownPacketId { state_name: String, id: u32 },
    #[error("Couldn't read packet id")]
    ReadPacketId { source: BufReadError },
    #[error(transparent)]
    Decompress {
        #[from]
        #[backtrace]
        source: DecompressionError,
    },
    #[error(transparent)]
    FrameSplitter {
        #[from]
        #[backtrace]
        source: FrameSplitterError,
    },
    #[error("Leftover data after reading packet {packet_name}: {data:?}")]
    LeftoverData { data: Vec<u8>, packet_name: String },
    #[error(transparent)]
    IoError {
        #[from]
        #[backtrace]
        source: io::Error,
    },
    #[error("Connection closed")]
    ConnectionClosed,
}

#[derive(Debug, Error)]
pub enum FrameSplitterError {
    #[error("Couldn't read VarInt length for packet. The previous packet may have been corrupted")]
    LengthRead {
        #[from]
        source: BufReadError,
    },
    #[error("Io error")]
    Io {
        #[from]
        #[backtrace]
        source: io::Error,
    },
    #[error("Packet is longer than {max} bytes (is {size})")]
    BadLength { max: usize, size: usize },
    #[error("Connection reset by peer")]
    ConnectionReset,
    #[error("Connection closed")]
    ConnectionClosed,
}

fn parse_frame(buffer: &mut Cursor<Vec<u8>>) -> Result<Box<[u8]>, FrameSplitterError> {
    let mut buffer_copy = Cursor::new(&buffer.get_ref()[buffer.position() as usize..]);
    let length = match u32::azalea_read_var(&mut buffer_copy) {
        Ok(length) => length as usize,
        Err(err) => match err {
            BufReadError::Io { source } => return Err(FrameSplitterError::Io { source }),
            _ => return Err(err.into()),
        },
    };

    if length > buffer_copy.remaining() {
        return Err(FrameSplitterError::BadLength {
            max: buffer_copy.remaining(),
            size: length,
        });
    }

    let varint_length = buffer.remaining() - buffer_copy.remaining();

    buffer.advance(varint_length);
    let data =
        buffer.get_ref()[buffer.position() as usize..buffer.position() as usize + length].to_vec();
    buffer.advance(length);

    if buffer.position() == buffer.get_ref().len() as u64 {
        buffer.get_mut().clear();

        buffer.get_mut().shrink_to(1024 * 64);

        buffer.set_position(0);
    }

    Ok(data.into_boxed_slice())
}

fn frame_splitter(buffer: &mut Cursor<Vec<u8>>) -> Result<Option<Box<[u8]>>, FrameSplitterError> {
    let read_frame = parse_frame(buffer);
    match read_frame {
        Ok(frame) => return Ok(Some(frame)),
        Err(err) => match err {
            FrameSplitterError::BadLength { .. } | FrameSplitterError::Io { .. } => {
            }
            _ => return Err(err),
        },
    }

    Ok(None)
}

pub fn deserialize_packet<P: ProtocolPacket + Debug>(
    stream: &mut Cursor<&[u8]>,
) -> Result<P, Box<ReadPacketError>> {
    let packet_id =
        u32::azalea_read_var(stream).map_err(|e| ReadPacketError::ReadPacketId { source: e })?;
    P::read(packet_id, stream)
}

static VALIDATE_DECOMPRESSED: bool = true;

pub static MAXIMUM_UNCOMPRESSED_LENGTH: u32 = 8_388_608;

#[derive(Debug, Error)]
pub enum DecompressionError {
    #[error("Couldn't read VarInt length for data")]
    LengthReadError {
        #[from]
        source: BufReadError,
    },
    #[error("Io error")]
    Io {
        #[from]
        #[backtrace]
        source: io::Error,
    },
    #[error("Badly compressed packet - size of {size} is below server threshold of {threshold}")]
    BelowCompressionThreshold { size: u32, threshold: u32 },
    #[error(
        "Badly compressed packet - size of {size} is larger than protocol maximum of {maximum}"
    )]
    AboveCompressionThreshold { size: u32, maximum: u32 },
}

pub fn compression_decoder(
    stream: &mut Cursor<&[u8]>,
    compression_threshold: u32,
) -> Result<Box<[u8]>, DecompressionError> {
    let n = u32::azalea_read_var(stream)?;
    if n == 0 {
        let buf = stream.get_ref()[stream.position() as usize..]
            .to_vec()
            .into_boxed_slice();
        stream.set_position(stream.get_ref().len() as u64);
        return Ok(buf);
    }

    if VALIDATE_DECOMPRESSED {
        if n < compression_threshold {
            return Err(DecompressionError::BelowCompressionThreshold {
                size: n,
                threshold: compression_threshold,
            });
        }
        if n > MAXIMUM_UNCOMPRESSED_LENGTH {
            return Err(DecompressionError::AboveCompressionThreshold {
                size: n,
                maximum: MAXIMUM_UNCOMPRESSED_LENGTH,
            });
        }
    }

    let mut decoded_buf = Vec::with_capacity(n as usize);

    let mut decoder = ZlibDecoder::new(stream);
    decoder.read_to_end(&mut decoded_buf)?;

    Ok(decoded_buf.into_boxed_slice())
}

pub async fn read_packet<P: ProtocolPacket + Debug, R>(
    stream: &mut R,
    buffer: &mut Cursor<Vec<u8>>,
    compression_threshold: Option<u32>,
    cipher: &mut Option<Aes128CfbDec>,
) -> Result<P, Box<ReadPacketError>>
where
    R: AsyncRead + Unpin + Send + Sync,
{
    let raw_packet = read_raw_packet(stream, buffer, compression_threshold, cipher).await?;
    let packet = deserialize_packet(&mut Cursor::new(&raw_packet))?;
    Ok(packet)
}

pub fn try_read_packet<P: ProtocolPacket + Debug, R>(
    stream: &mut R,
    buffer: &mut Cursor<Vec<u8>>,
    compression_threshold: Option<u32>,
    cipher: &mut Option<Aes128CfbDec>,
) -> Result<Option<P>, Box<ReadPacketError>>
where
    R: AsyncRead + Unpin + Send + Sync,
{
    let Some(raw_packet) = try_read_raw_packet(stream, buffer, compression_threshold, cipher)?
    else {
        return Ok(None);
    };
    let packet = deserialize_packet(&mut Cursor::new(&raw_packet))?;
    Ok(Some(packet))
}

pub async fn read_raw_packet<R>(
    stream: &mut R,
    buffer: &mut Cursor<Vec<u8>>,
    compression_threshold: Option<u32>,
    cipher: &mut Option<Aes128CfbDec>,
) -> Result<Box<[u8]>, Box<ReadPacketError>>
where
    R: AsyncRead + Unpin + Send + Sync,
{
    loop {
        if let Some(buf) = read_raw_packet_from_buffer::<R>(buffer, compression_threshold)? {
            return Ok(buf);
        };

        let bytes = read_and_decrypt_frame(stream, cipher).await?;
        buffer.get_mut().extend_from_slice(&bytes);
    }
}
pub fn try_read_raw_packet<R>(
    stream: &mut R,
    buffer: &mut Cursor<Vec<u8>>,
    compression_threshold: Option<u32>,
    cipher: &mut Option<Aes128CfbDec>,
) -> Result<Option<Box<[u8]>>, Box<ReadPacketError>>
where
    R: AsyncRead + Unpin + Send + Sync,
{
    loop {
        if let Some(buf) = read_raw_packet_from_buffer::<R>(buffer, compression_threshold)? {
            return Ok(Some(buf));
        };
        let Some(bytes) = try_read_and_decrypt_frame(stream, cipher)? else {
            return Ok(None);
        };
        buffer.get_mut().extend_from_slice(&bytes);
    }
}

async fn read_and_decrypt_frame<R>(
    stream: &mut R,
    cipher: &mut Option<Aes128CfbDec>,
) -> Result<Box<[u8]>, Box<ReadPacketError>>
where
    R: AsyncRead + Unpin + Send + Sync,
{
    let mut framed = FramedRead::new(stream, BytesCodec::new());

    let Some(message) = framed.next().await else {
        return Err(Box::new(ReadPacketError::ConnectionClosed));
    };
    let bytes = message.map_err(ReadPacketError::from)?;
    crate::traffic::note_rx(bytes.len());

    let mut bytes = bytes.to_vec().into_boxed_slice();

    if let Some(cipher) = cipher {
        azalea_crypto::decrypt_packet(cipher, &mut bytes);
    }

    Ok(bytes)
}
fn try_read_and_decrypt_frame<R>(
    stream: &mut R,
    cipher: &mut Option<Aes128CfbDec>,
) -> Result<Option<Box<[u8]>>, Box<ReadPacketError>>
where
    R: AsyncRead + Unpin + Send + Sync,
{
    let mut framed = FramedRead::new(stream, BytesCodec::new());

    let Some(message) = future::block_on(future::poll_once(framed.next())) else {
        return Ok(None);
    };
    let Some(message) = message else {
        return Err(Box::new(ReadPacketError::ConnectionClosed));
    };
    let bytes = message.map_err(ReadPacketError::from)?;
    crate::traffic::note_rx(bytes.len());
    let mut bytes = bytes.to_vec().into_boxed_slice();

    if let Some(cipher) = cipher {
        azalea_crypto::decrypt_packet(cipher, &mut bytes);
    }

    Ok(Some(bytes))
}

pub fn read_raw_packet_from_buffer<R>(
    buffer: &mut Cursor<Vec<u8>>,
    compression_threshold: Option<u32>,
) -> Result<Option<Box<[u8]>>, Box<ReadPacketError>>
where
    R: AsyncRead + Unpin + Send + Sync,
{
    let Some(mut buf) = frame_splitter(buffer).map_err(ReadPacketError::from)? else {
        return Ok(None);
    };

    if let Some(compression_threshold) = compression_threshold {
        buf = compression_decoder(&mut Cursor::new(&buf[..]), compression_threshold)
            .map_err(ReadPacketError::from)?;
    }

    if tracing::enabled!(tracing::Level::TRACE) {
        static DO_NOT_CUT_OFF_PACKET_LOGS: LazyLock<bool> = LazyLock::new(|| {
            env::var("AZALEA_DO_NOT_CUT_OFF_PACKET_LOGS")
                .map(|s| s == "1" || s == "true")
                .unwrap_or(false)
        });

        let buf_string: String = {
            if !*DO_NOT_CUT_OFF_PACKET_LOGS && buf.len() > 500 {
                let cut_off_buf = &buf[..500];
                format!("{cut_off_buf:?}...")
            } else {
                format!("{buf:?}")
            }
        };
        trace!("Reading packet with bytes: {buf_string}");
    };

    Ok(Some(buf))
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use azalea_buf::AzBuf as _;

    use crate::{packets::game::ClientboundGamePacket, read::deserialize_packet};

    #[test]
    fn fuzzed_1() {
        let _ = deserialize_packet::<ClientboundGamePacket>(&mut Cursor::new(
            [132, 1, 255, 255, 255, 255, 255].as_slice(),
        ));
    }
    #[test]
    fn fuzzed_2() {
        let _ = deserialize_packet::<ClientboundGamePacket>(&mut Cursor::new(
            [132, 1, 75, 0, 255, 255, 255, 255, 24, 0].as_slice(),
        ));
    }
    #[test]
    fn fuzzed_3() {
        let _ = deserialize_packet::<ClientboundGamePacket>(&mut Cursor::new(
            [
                94, 44, 157, 38, 61, 37, 37, 37, 37, 37, 37, 65, 128, 128, 1, 1, 255, 252, 128,
                128, 128, 128, 128, 128, 128, 40, 0,
            ]
            .as_slice(),
        ));
    }
    #[test]
    fn fuzzed_4() {
        let _ = deserialize_packet::<ClientboundGamePacket>(&mut Cursor::new(
            [94, 94, 70, 52, 0, 6, 0].as_slice(),
        ));
    }
    #[test]
    fn fuzzed_5() {
        let _ = deserialize_packet::<ClientboundGamePacket>(&mut Cursor::new(
            [94, 94, 70, 52, 0, 6, 0, 6, 0].as_slice(),
        ));
    }
    #[test]
    fn fuzzed_6() {
        let _ = simdnbt::owned::Nbt::azalea_read(&mut Cursor::new([10, 10, 0, 0, 0].as_slice()));
    }
}
