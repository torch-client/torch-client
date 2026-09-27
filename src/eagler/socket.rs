pub(super) enum Frame {
    Binary(Vec<u8>),
    Text(String),
    Close(Option<String>),
    Other,
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) use native::{Rx, Socket, Tx};
#[cfg(target_arch = "wasm32")]
pub(super) use web::{Rx, Socket, Tx};

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use futures_util::{
        SinkExt, StreamExt,
        stream::{SplitSink, SplitStream},
    };
    use tokio_tungstenite::tungstenite::Message;

    use super::Frame;

    type Stream = tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >;

    pub(in crate::eagler) struct Socket(Stream);
    pub(in crate::eagler) struct Tx(SplitSink<Stream, Message>);
    pub(in crate::eagler) struct Rx(SplitStream<Stream>);

    impl Socket {
        pub(in crate::eagler) async fn connect(url: &str) -> eyre::Result<Self> {
            crate::install_crypto_provider();
            match tokio_tungstenite::connect_async(url).await {
                Ok((socket, _response)) => Ok(Self(socket)),
                Err(tokio_tungstenite::tungstenite::Error::Http(response)) => {
                    eyre::bail!(
                        "{url} answered with HTTP {} instead of upgrading to a websocket, so it \
                         is not an Eaglercraft endpoint. The endpoint is often on a subdomain or \
                         a path rather than the site root.",
                        response.status()
                    )
                }
                Err(e) => Err(e.into()),
            }
        }

        pub(in crate::eagler) async fn send_binary(
            &mut self,
            payload: Vec<u8>,
        ) -> eyre::Result<()> {
            self.0.send(Message::Binary(payload.into())).await?;
            Ok(())
        }

        pub(in crate::eagler) async fn send_text(&mut self, text: String) -> eyre::Result<()> {
            self.0.send(Message::Text(text.into())).await?;
            Ok(())
        }

        pub(in crate::eagler) async fn recv(&mut self) -> eyre::Result<Option<Frame>> {
            match self.0.next().await {
                Some(message) => Ok(Some(convert(message?))),
                None => Ok(None),
            }
        }

        pub(in crate::eagler) fn split(self) -> (Tx, Rx) {
            let (sink, stream) = self.0.split();
            (Tx(sink), Rx(stream))
        }
    }

    impl Tx {
        pub(in crate::eagler) async fn send_binary(
            &mut self,
            payload: Vec<u8>,
        ) -> eyre::Result<()> {
            self.0.send(Message::Binary(payload.into())).await?;
            Ok(())
        }
    }

    impl Rx {
        pub(in crate::eagler) async fn recv(&mut self) -> eyre::Result<Option<Frame>> {
            match self.0.next().await {
                Some(message) => Ok(Some(convert(message?))),
                None => Ok(None),
            }
        }
    }

    fn convert(message: Message) -> Frame {
        match message {
            Message::Binary(payload) => Frame::Binary(payload.to_vec()),
            Message::Text(text) => Frame::Text(text.to_string()),
            Message::Close(reason) => {
                Frame::Close(reason.map(|reason| format!("{} ({})", reason.reason, reason.code)))
            }
            _ => Frame::Other,
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod web {
    use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
    use wasm_bindgen::{JsCast, prelude::Closure};

    use super::Frame;

    enum Event {
        Open,
        Frame(Frame),
        Failed(String),
    }

    pub(in crate::eagler) struct Socket {
        tx: Tx,
        rx: Rx,
    }

    pub(in crate::eagler) struct Tx(web_sys::WebSocket);

    pub(in crate::eagler) struct Rx {
        events: UnboundedReceiver<Event>,
        done: bool,
        handlers: Handlers,
    }

    struct Handlers {
        socket: web_sys::WebSocket,
        _open: Closure<dyn FnMut()>,
        _message: Closure<dyn FnMut(web_sys::MessageEvent)>,
        _close: Closure<dyn FnMut(web_sys::CloseEvent)>,
        _error: Closure<dyn FnMut(web_sys::Event)>,
    }

    impl Drop for Handlers {
        fn drop(&mut self) {
            self.socket.set_onopen(None);
            self.socket.set_onmessage(None);
            self.socket.set_onclose(None);
            self.socket.set_onerror(None);
            let _ = self.socket.close();
        }
    }

    impl Socket {
        pub(in crate::eagler) async fn connect(url: &str) -> eyre::Result<Self> {
            let socket = web_sys::WebSocket::new(url).map_err(|e| {
                eyre::eyre!(
                    "{url} is not an address a websocket can be opened to: {}",
                    describe(&e)
                )
            })?;
            socket.set_binary_type(web_sys::BinaryType::Arraybuffer);

            let (sender, events) = unbounded_channel();
            let handlers = install(&socket, sender);
            let mut rx = Rx {
                events,
                done: false,
                handlers,
            };

            match rx.events.recv().await {
                Some(Event::Open) => Ok(Self { tx: Tx(socket), rx }),
                Some(Event::Failed(e)) => eyre::bail!("could not open a websocket to {url}: {e}"),
                Some(Event::Frame(Frame::Close(reason))) => match reason {
                    Some(reason) => {
                        eyre::bail!("{url} closed the websocket before it opened: {reason}")
                    }
                    None => eyre::bail!("{url} closed the websocket before it opened"),
                },
                _ => eyre::bail!("the websocket to {url} neither opened nor failed"),
            }
        }

        pub(in crate::eagler) async fn send_binary(
            &mut self,
            payload: Vec<u8>,
        ) -> eyre::Result<()> {
            self.tx.send_binary(payload).await
        }

        pub(in crate::eagler) async fn send_text(&mut self, text: String) -> eyre::Result<()> {
            self.tx
                .0
                .send_with_str(&text)
                .map_err(|e| eyre::eyre!("could not send on the websocket: {}", describe(&e)))
        }

        pub(in crate::eagler) async fn recv(&mut self) -> eyre::Result<Option<Frame>> {
            self.rx.recv().await
        }

        pub(in crate::eagler) fn split(self) -> (Tx, Rx) {
            (self.tx, self.rx)
        }
    }

    impl Tx {
        pub(in crate::eagler) async fn send_binary(
            &mut self,
            payload: Vec<u8>,
        ) -> eyre::Result<()> {
            self.0
                .send_with_u8_array(&payload)
                .map_err(|e| eyre::eyre!("could not send on the websocket: {}", describe(&e)))
        }
    }

    impl Rx {
        pub(in crate::eagler) async fn recv(&mut self) -> eyre::Result<Option<Frame>> {
            if self.done {
                return Ok(None);
            }
            match self.events.recv().await {
                Some(Event::Frame(Frame::Close(reason))) => {
                    self.done = true;
                    Ok(Some(Frame::Close(reason)))
                }
                Some(Event::Frame(frame)) => Ok(Some(frame)),
                Some(Event::Failed(e)) => {
                    self.done = true;
                    Err(eyre::eyre!("{e}"))
                }
                Some(Event::Open) => Ok(Some(Frame::Other)),
                None => {
                    self.done = true;
                    Ok(None)
                }
            }
        }
    }

    fn install(socket: &web_sys::WebSocket, sender: UnboundedSender<Event>) -> Handlers {
        let open = {
            let sender = sender.clone();
            Closure::<dyn FnMut()>::new(move || {
                let _ = sender.send(Event::Open);
            })
        };
        let message = {
            let sender = sender.clone();
            Closure::<dyn FnMut(web_sys::MessageEvent)>::new(move |e: web_sys::MessageEvent| {
                let data = e.data();
                let frame = if let Some(buffer) = data.dyn_ref::<js_sys::ArrayBuffer>() {
                    Frame::Binary(js_sys::Uint8Array::new(buffer).to_vec())
                } else if let Some(text) = data.as_string() {
                    Frame::Text(text)
                } else {
                    Frame::Other
                };
                let _ = sender.send(Event::Frame(frame));
            })
        };
        let close = {
            let sender = sender.clone();
            Closure::<dyn FnMut(web_sys::CloseEvent)>::new(move |e: web_sys::CloseEvent| {
                let reason = e.reason();
                let reason = if reason.is_empty() {
                    format!("code {}", e.code())
                } else {
                    format!("{reason} ({})", e.code())
                };
                let _ = sender.send(Event::Frame(Frame::Close(Some(reason))));
            })
        };
        let error = Closure::<dyn FnMut(web_sys::Event)>::new(move |_: web_sys::Event| {
            let _ = sender.send(Event::Failed(
                "the websocket failed. The browser does not say why, deliberately: a page is not \
                 allowed to learn whether a host exists, refused the connection or served a \
                 certificate it did not trust. The browser's own console names it."
                    .to_string(),
            ));
        });

        socket.set_onopen(Some(open.as_ref().unchecked_ref()));
        socket.set_onmessage(Some(message.as_ref().unchecked_ref()));
        socket.set_onclose(Some(close.as_ref().unchecked_ref()));
        socket.set_onerror(Some(error.as_ref().unchecked_ref()));

        Handlers {
            socket: socket.clone(),
            _open: open,
            _message: message,
            _close: close,
            _error: error,
        }
    }

    fn describe(value: &wasm_bindgen::JsValue) -> String {
        value
            .as_string()
            .or_else(|| {
                js_sys::Reflect::get(value, &"message".into())
                    .ok()?
                    .as_string()
            })
            .unwrap_or_else(|| format!("{value:?}"))
    }
}
