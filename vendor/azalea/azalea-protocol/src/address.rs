use std::{
    fmt::{self, Debug, Display},
    net::SocketAddr,
    str::FromStr,
};

use crate::{resolve::resolve_address, resolver::ResolveError};

pub trait ResolvableAddr: Debug + Clone {
    fn server_addr(self) -> Result<ServerAddr, ResolveError>;
    fn resolve(self) -> impl Future<Output = Result<ResolvedAddr, ResolveError>> + Send;
}
impl<T: TryInto<ServerAddr, Error = ServerAddrParseError> + Debug + Send + Clone> ResolvableAddr
    for T
{
    fn server_addr(self) -> Result<ServerAddr, ResolveError> {
        self.try_into()
            .map_err(|_| "failed to parse address".into())
    }

    async fn resolve(self) -> Result<ResolvedAddr, ResolveError> {
        ResolvedAddr::new(self.server_addr()?).await
    }
}

impl ResolvableAddr for &ResolvedAddr {
    fn server_addr(self) -> Result<ServerAddr, ResolveError> {
        Ok(self.server.clone())
    }

    async fn resolve(self) -> Result<ResolvedAddr, ResolveError> {
        Ok(self.clone())
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ServerAddr {
    pub host: String,
    pub port: u16,
}

#[derive(Debug)]
pub struct ServerAddrParseError;

impl TryFrom<&str> for ServerAddr {
    type Error = ServerAddrParseError;

    fn try_from(string: &str) -> Result<Self, Self::Error> {
        if string.is_empty() {
            return Err(ServerAddrParseError);
        }
        let mut parts = string.split(':');
        let host = parts.next().ok_or(ServerAddrParseError)?.to_owned();
        let port = parts.next().unwrap_or("25565");
        let port = u16::from_str(port).ok().ok_or(ServerAddrParseError)?;
        Ok(ServerAddr { host, port })
    }
}
impl TryFrom<String> for ServerAddr {
    type Error = ServerAddrParseError;

    fn try_from(string: String) -> Result<Self, Self::Error> {
        ServerAddr::try_from(string.as_str())
    }
}

impl From<SocketAddr> for ServerAddr {
    fn from(addr: SocketAddr) -> Self {
        ServerAddr {
            host: addr.ip().to_string(),
            port: addr.port(),
        }
    }
}

impl Display for ServerAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.host, self.port)
    }
}

impl<'de> serde::Deserialize<'de> for ServerAddr {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let string = String::deserialize(deserializer)?;
        ServerAddr::try_from(string.as_str())
            .map_err(|_| serde::de::Error::custom("failed to parse address"))
    }
}

impl serde::Serialize for ServerAddr {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

#[derive(Clone, Debug)]
pub struct ResolvedAddr {
    pub server: ServerAddr,
    pub socket: SocketAddr,
}

impl ResolvedAddr {
    pub async fn new(server: impl Into<ServerAddr>) -> Result<Self, ResolveError> {
        let server = server.into();
        let socket = resolve_address(&server).await?;
        Ok(Self { server, socket })
    }
}
