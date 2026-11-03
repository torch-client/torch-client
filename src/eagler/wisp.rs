use azalea_protocol::address::ServerAddr;

use super::socket::{Frame, Socket};

const CONNECT: u8 = 0x01;
const DATA: u8 = 0x02;
const CONTINUE: u8 = 0x03;
const CLOSE: u8 = 0x04;

const TCP: u8 = 0x01;
const STREAM: u32 = 1;
const HEADER: usize = 5;
const REASON_UNKNOWN: u8 = 0x01;

pub(super) struct Stream {
    credit: u32,
}

pub(super) enum Inbound {
    Data(Vec<u8>),
    Closed(String),
    Nothing,
}

pub(super) async fn open(url: &str, server: &ServerAddr) -> eyre::Result<(Socket, Stream)> {
    let mut socket = Socket::connect(url).await?;
    let credit = loop {
        match socket.recv().await? {
            Some(Frame::Binary(frame)) => {
                if let Some(credit) = opening_credit(&frame) {
                    break credit;
                }
            }
            Some(Frame::Close(Some(reason))) => {
                eyre::bail!("the proxy closed the connection: {reason}")
            }
            Some(Frame::Close(None)) | None => {
                eyre::bail!("the proxy closed the connection without saying why")
            }
            Some(Frame::Text(_) | Frame::Other) => {}
        }
    };
    socket.send_binary(connect(server)).await?;
    Ok((socket, Stream { credit }))
}

fn opening_credit(frame: &[u8]) -> Option<u32> {
    match split(frame)? {
        (CONTINUE, 0, payload) => read_u32(payload),
        _ => None,
    }
}

fn header(kind: u8, capacity: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(HEADER + capacity);
    out.push(kind);
    out.extend_from_slice(&STREAM.to_le_bytes());
    out
}

fn connect(server: &ServerAddr) -> Vec<u8> {
    let mut out = header(CONNECT, 3 + server.host.len());
    out.push(TCP);
    out.extend_from_slice(&server.port.to_le_bytes());
    out.extend_from_slice(server.host.as_bytes());
    out
}

fn split(frame: &[u8]) -> Option<(u8, u32, &[u8])> {
    let (&kind, rest) = frame.split_first()?;
    let (id, payload) = rest.split_first_chunk::<4>()?;
    Some((kind, u32::from_le_bytes(*id), payload))
}

fn read_u32(payload: &[u8]) -> Option<u32> {
    payload.first_chunk::<4>().copied().map(u32::from_le_bytes)
}

fn describe(reason: u8) -> String {
    let text = match reason {
        0x02 => "the connection was closed",
        0x03 => "a network error ended the connection",
        0x41 => "the proxy rejected the address",
        0x42 => "the proxy could not reach that host",
        0x43 | 0x47 => "the connection through the proxy timed out",
        0x44 => "the server refused the connection",
        0x48 => "the proxy blocks that address",
        0x49 => "the proxy is throttling this connection",
        _ => return format!("the proxy closed the connection (reason 0x{reason:02x})"),
    };
    text.to_string()
}

impl Stream {
    pub(super) fn can_send(&self) -> bool {
        self.credit > 0
    }

    pub(super) fn data(&mut self, body: &[u8]) -> Vec<u8> {
        self.credit = self.credit.saturating_sub(1);
        let mut out = header(DATA, body.len());
        out.extend_from_slice(body);
        out
    }

    pub(super) fn receive(&mut self, frame: &[u8]) -> Inbound {
        match split(frame) {
            Some((DATA, STREAM, payload)) => Inbound::Data(payload.to_vec()),
            Some((CONTINUE, STREAM, payload)) => {
                if let Some(credit) = read_u32(payload) {
                    self.credit = credit;
                }
                Inbound::Nothing
            }
            Some((CLOSE, STREAM, payload)) => {
                Inbound::Closed(describe(payload.first().copied().unwrap_or(REASON_UNKNOWN)))
            }
            _ => Inbound::Nothing,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Inbound, STREAM, Stream, connect, opening_credit};
    use azalea_protocol::address::ServerAddr;

    fn message(kind: u8, stream: u32, payload: &[u8]) -> Vec<u8> {
        let mut out = vec![kind];
        out.extend_from_slice(&stream.to_le_bytes());
        out.extend_from_slice(payload);
        out
    }

    #[test]
    fn connect_is_little_endian_with_an_unterminated_host() {
        let server = ServerAddr {
            host: "mc.example".to_string(),
            port: 25565,
        };
        let mut expected = vec![0x01, 1, 0, 0, 0, 0x01, 0xDD, 0x63];
        expected.extend_from_slice(b"mc.example");
        assert_eq!(connect(&server), expected);
    }

    #[test]
    fn the_opening_continue_is_read_from_stream_zero() {
        assert_eq!(
            opening_credit(&[0x03, 0, 0, 0, 0, 0x80, 0, 0, 0]),
            Some(128)
        );
        assert_eq!(opening_credit(&message(0x03, STREAM, &[1, 0, 0, 0])), None);
        assert_eq!(opening_credit(&[0x03, 0]), None);
    }

    #[test]
    fn sending_stops_at_zero_credit_until_a_continue_replaces_it() {
        let mut stream = Stream { credit: 1 };
        assert!(stream.can_send());
        assert_eq!(stream.data(&[9]), message(0x02, STREAM, &[9]));
        assert!(!stream.can_send());
        stream.receive(&message(0x03, STREAM, &[5, 0, 0, 0]));
        assert_eq!(stream.credit, 5);
    }

    #[test]
    fn data_comes_out_without_its_header() {
        let mut stream = Stream { credit: 1 };
        match stream.receive(&message(0x02, STREAM, &[1, 2, 3])) {
            Inbound::Data(bytes) => assert_eq!(bytes, [1, 2, 3]),
            _ => panic!("expected data"),
        }
    }

    #[test]
    fn a_close_names_its_reason() {
        let mut stream = Stream { credit: 1 };
        match stream.receive(&message(0x04, STREAM, &[0x42])) {
            Inbound::Closed(reason) => assert!(reason.contains("could not reach")),
            _ => panic!("expected a close"),
        }
    }

    #[test]
    fn another_streams_traffic_and_a_v2_info_are_ignored() {
        let mut stream = Stream { credit: 1 };
        assert!(matches!(
            stream.receive(&message(0x02, 9, &[1])),
            Inbound::Nothing
        ));
        assert!(matches!(
            stream.receive(&message(0x05, 0, &[2, 0])),
            Inbound::Nothing
        ));
        assert!(matches!(stream.receive(&[0x02]), Inbound::Nothing));
    }
}
