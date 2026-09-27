#[cfg(feature = "online-mode")]
pub mod microsoft;
pub mod offline;

use std::{fmt::Debug, ops::Deref, pin::Pin, sync::Arc};

#[cfg(feature = "online-mode")]
use azalea_auth::sessionserver::ClientSessionServerError;
use bevy_ecs::component::Component;
use uuid::Uuid;

#[derive(Clone, Component, Debug)]
pub struct Account(Arc<dyn AccountTrait>);

impl Account {
    #[deprecated = "moved to `uuid()`."]
    pub fn uuid_or_offline(&self) -> Uuid {
        self.uuid()
    }
}

pub(crate) type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub trait AccountTrait: Send + Sync + Debug {
    fn username(&self) -> &str;
    fn uuid(&self) -> Uuid;

    fn access_token(&self) -> Option<String>;

    #[cfg(feature = "online-mode")]
    fn refresh(&self) -> BoxFuture<'_, Result<(), azalea_auth::AuthError>> {
        Box::pin(async { Ok(()) })
    }
    #[cfg(not(feature = "online-mode"))]
    fn refresh(&self) -> BoxFuture<'_, Result<(), ()>> {
        Box::pin(async { Ok(()) })
    }

    #[cfg(feature = "online-mode")]
    fn certs(&self) -> Option<azalea_auth::certs::Certificates> {
        None
    }
    #[cfg(feature = "online-mode")]
    fn set_certs(&self, certs: azalea_auth::certs::Certificates) {
        let _ = certs;
    }

    #[cfg(feature = "online-mode")]
    fn join<'a>(
        &'a self,
        public_key: &'a [u8],
        private_key: &'a [u8; 16],
        server_id: &'a str,
        proxy: Option<reqwest::Proxy>,
    ) -> BoxFuture<'a, Result<(), ClientSessionServerError>> {
        let _ = (public_key, private_key, server_id, proxy);
        Box::pin(async { Ok(()) })
    }
    #[cfg(not(feature = "online-mode"))]
    fn join(
        &self,
        public_key: &[u8],
        private_key: &[u8; 16],
        server_id: &str,
        proxy: Option<()>,
    ) -> BoxFuture<'_, Result<(), ()>> {
        let _ = (public_key, private_key, server_id, proxy);
        Box::pin(async { Ok(()) })
    }
}
impl<T: AccountTrait + 'static> From<T> for Account {
    fn from(value: T) -> Self {
        Account(Arc::new(value))
    }
}
impl Deref for Account {
    type Target = dyn AccountTrait;

    fn deref(&self) -> &Self::Target {
        &*self.0
    }
}
