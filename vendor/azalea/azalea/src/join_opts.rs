use std::net::SocketAddr;

use azalea_protocol::{address::ServerAddr, connect::Proxy};

#[derive(Clone, Debug, Default)]
#[non_exhaustive]
pub struct JoinOpts {
    pub server_proxy: Option<Proxy>,
    pub sessionserver_proxy: Option<Proxy>,
    #[doc(alias = "custom_address")]
    pub custom_server_addr: Option<ServerAddr>,
    #[doc(alias = "custom_resolved_address")]
    pub custom_socket_addr: Option<SocketAddr>,
}

impl JoinOpts {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update(&mut self, other: &Self) {
        if let Some(proxy) = other.server_proxy.clone() {
            self.server_proxy = Some(proxy);
        }
        if let Some(proxy) = other.sessionserver_proxy.clone() {
            self.sessionserver_proxy = Some(proxy);
        }
        if let Some(custom_server_addr) = other.custom_server_addr.clone() {
            self.custom_server_addr = Some(custom_server_addr);
        }
        if let Some(custom_socket_addr) = other.custom_socket_addr {
            self.custom_socket_addr = Some(custom_socket_addr);
        }
    }

    #[must_use]
    pub fn proxy(self, proxy: Proxy) -> Self {
        self.server_proxy(proxy.clone()).sessionserver_proxy(proxy)
    }
    #[must_use]
    pub fn server_proxy(mut self, proxy: Proxy) -> Self {
        self.server_proxy = Some(proxy);
        self
    }
    #[must_use]
    pub fn sessionserver_proxy(mut self, proxy: Proxy) -> Self {
        self.sessionserver_proxy = Some(proxy);
        self
    }

    #[must_use]
    pub fn custom_server_addr(mut self, server_addr: ServerAddr) -> Self {
        self.custom_server_addr = Some(server_addr);
        self
    }
    #[must_use]
    pub fn custom_socket_addr(mut self, socket_addr: SocketAddr) -> Self {
        self.custom_socket_addr = Some(socket_addr);
        self
    }

    #[doc(hidden)]
    #[deprecated = "renamed to `custom_server_addr`."]
    pub fn custom_address(self, server_addr: ServerAddr) -> Self {
        self.custom_server_addr(server_addr)
    }
    #[doc(hidden)]
    #[deprecated = "renamed to `custom_socket_addr`."]
    pub fn custom_resolved_address(self, socket_addr: SocketAddr) -> Self {
        self.custom_socket_addr(socket_addr)
    }
}
