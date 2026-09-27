use std::net::{IpAddr, SocketAddr};
#[cfg(feature = "dns")]
use std::sync::LazyLock;

#[cfg(feature = "dns")]
pub use hickory_resolver::net::NetError as ResolveError;
#[cfg(feature = "dns")]
use hickory_resolver::{
    Resolver, TokioResolver,
    config::{GOOGLE, ResolverConfig},
    net::runtime::TokioRuntimeProvider,
    proto::rr::{Name, RData},
};
#[cfg(feature = "dns")]
use tracing::warn;

use crate::address::ServerAddr;

#[cfg(not(feature = "dns"))]
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct ResolveError(String);

#[cfg(not(feature = "dns"))]
impl From<&str> for ResolveError {
    fn from(message: &str) -> Self {
        Self(message.to_owned())
    }
}

#[cfg(not(feature = "dns"))]
pub async fn resolve_address(address: &ServerAddr) -> Result<SocketAddr, ResolveError> {
    match address.host.parse::<IpAddr>() {
        Ok(ip) => Ok(SocketAddr::new(ip, address.port)),
        Err(_) => Err(ResolveError::from(
            "this build has no DNS resolver, so the address must be a literal IP",
        )),
    }
}

#[doc(hidden)]
#[deprecated(note = "Renamed to ResolveError")]
pub type ResolverError = ResolveError;

#[cfg(feature = "dns")]
static RESOLVER: LazyLock<TokioResolver> = LazyLock::new(|| {
    Resolver::builder_tokio()
        .unwrap_or_else(|_| {
            warn!("System DNS resolver unavailable; falling back to Google DNS.");

            Resolver::builder_with_config(
                ResolverConfig::udp_and_tcp(&GOOGLE),
                TokioRuntimeProvider::new(),
            )
        })
        .build()
        .unwrap()
});

#[cfg(feature = "dns")]
pub async fn resolve_address(mut address: &ServerAddr) -> Result<SocketAddr, ResolveError> {
    let redirect = resolve_srv_redirect(address).await;
    if let Ok(redirect_target) = &redirect {
        address = redirect_target;
    }

    resolve_ip_without_redirects(address).await
}

#[cfg(feature = "dns")]
async fn resolve_ip_without_redirects(address: &ServerAddr) -> Result<SocketAddr, ResolveError> {
    if let Ok(ip) = address.host.parse::<IpAddr>() {
        return Ok(SocketAddr::new(ip, address.port));
    }

    let name = Name::from_ascii(&address.host)?;
    let lookup_ip = RESOLVER.lookup_ip(name).await?;

    let ip = lookup_ip
        .iter()
        .next()
        .ok_or(ResolveError::from("No A/AAAA record found"))?;

    Ok(SocketAddr::new(ip, address.port))
}

#[cfg(feature = "dns")]
async fn resolve_srv_redirect(address: &ServerAddr) -> Result<ServerAddr, ResolveError> {
    if address.port != 25565 {
        return Err(ResolveError::from("Port must be 25565 to do a SRV lookup"));
    }

    let query = format!("_minecraft._tcp.{}", address.host);
    let res = RESOLVER.srv_lookup(query).await?;

    let srv = res
        .answers()
        .first()
        .ok_or(ResolveError::from("No SRV record found"))?;
    let RData::SRV(srv) = &srv.data else {
        return Err(ResolveError::from(
            "Record returned from SRV lookup wasn't SRV",
        ));
    };

    Ok(ServerAddr {
        host: srv.target.to_ascii(),
        port: srv.port,
    })
}
