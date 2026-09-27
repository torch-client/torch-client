use std::path::PathBuf;

use azalea_auth::{
    AccessTokenResponse, AuthOpts,
    certs::Certificates,
    sessionserver::{self, ClientSessionServerError, SessionServerJoinOpts},
};
use parking_lot::Mutex;
use uuid::Uuid;

use crate::account::{Account, AccountTrait, BoxFuture};

fn default_cache_file() -> PathBuf {
    let minecraft_dir = minecraft_folder_path::minecraft_dir().unwrap_or_else(|| {
        panic!(
            "No {} environment variable found",
            minecraft_folder_path::home_env_var()
        )
    });
    minecraft_dir.join("azalea-auth.json")
}

#[derive(Clone, Debug, Default)]
pub struct MicrosoftAccountOpts {
    pub check_ownership: bool,
    pub cache_file: Option<PathBuf>,
    pub client_id: Option<String>,
    pub scope: Option<String>,
}

impl MicrosoftAccountOpts {
    fn to_auth_opts(&self) -> AuthOpts<'_> {
        let cache_file = self
            .cache_file
            .clone()
            .or_else(|| Some(default_cache_file()));

        AuthOpts {
            check_ownership: self.check_ownership,
            cache_file,
            client_id: self.client_id.as_deref(),
            scope: self.scope.as_deref(),
        }
    }
}

fn default_account_opts(client_id: Option<&str>, scope: Option<&str>) -> MicrosoftAccountOpts {
    MicrosoftAccountOpts {
        check_ownership: false,
        cache_file: Some(default_cache_file()),
        client_id: client_id.map(str::to_owned),
        scope: scope.map(str::to_owned),
    }
}

#[derive(Debug)]
pub struct MicrosoftAccount {
    cache_key: String,
    auth_opts: MicrosoftAccountOpts,

    username: String,
    uuid: Uuid,

    access_token: Mutex<String>,
    certs: Mutex<Option<Certificates>>,
}
impl MicrosoftAccount {
    async fn new(
        cache_key: &str,
        auth_opts: MicrosoftAccountOpts,
    ) -> Result<Self, azalea_auth::AuthError> {
        let auth_result = azalea_auth::auth(cache_key, auth_opts.to_auth_opts()).await?;

        Ok(Self {
            cache_key: cache_key.to_owned(),
            auth_opts,
            username: auth_result.profile.name,
            uuid: auth_result.profile.id,
            access_token: Mutex::new(auth_result.access_token),
            certs: Mutex::new(None),
        })
    }
}
impl AccountTrait for MicrosoftAccount {
    fn username(&self) -> &str {
        &self.username
    }
    fn uuid(&self) -> Uuid {
        self.uuid
    }
    fn access_token(&self) -> Option<String> {
        Some(self.access_token.lock().to_owned())
    }
    fn certs(&self) -> Option<azalea_auth::certs::Certificates> {
        self.certs.lock().as_ref().cloned()
    }
    fn set_certs(&self, certs: azalea_auth::certs::Certificates) {
        *self.certs.lock() = Some(certs);
    }
    fn refresh(&self) -> BoxFuture<'_, Result<(), azalea_auth::AuthError>> {
        Box::pin(async {
            let new_account =
                MicrosoftAccount::new(&self.cache_key, self.auth_opts.clone()).await?;
            let new_access_token = new_account.access_token().unwrap();
            *self.access_token.lock() = new_access_token;
            Ok(())
        })
    }
    fn join<'a>(
        &'a self,
        public_key: &'a [u8],
        private_key: &'a [u8; 16],
        server_id: &'a str,
        proxy: Option<reqwest::Proxy>,
    ) -> BoxFuture<'a, Result<(), ClientSessionServerError>> {
        Box::pin(async move {
            let access_token = self.access_token.lock().clone();
            sessionserver::join(SessionServerJoinOpts {
                access_token: &access_token,
                public_key,
                private_key,
                uuid: &self.uuid(),
                server_id,
                proxy,
            })
            .await
        })
    }
}

#[derive(Debug)]
pub struct MicrosoftWithAccessTokenAccount {
    msa: Mutex<azalea_auth::cache::ExpiringValue<AccessTokenResponse>>,

    username: String,
    uuid: Uuid,

    access_token: Mutex<String>,
    certs: Mutex<Option<Certificates>>,
}
impl MicrosoftWithAccessTokenAccount {
    async fn new(
        msa: azalea_auth::cache::ExpiringValue<AccessTokenResponse>,
        client_id: Option<&str>,
        scope: Option<&str>,
    ) -> Result<Self, azalea_auth::AuthError> {
        let client = reqwest::Client::new();

        let mut msa = msa.clone();

        if msa.is_expired() {
            use tracing::trace;

            trace!("refreshing Microsoft auth token");
            msa = azalea_auth::refresh_ms_auth_token(
                &client,
                &msa.data.refresh_token,
                client_id,
                scope,
            )
            .await?;
        }

        let msa_token = &msa.data.access_token;
        let res = azalea_auth::get_minecraft_token(&client, msa_token).await?;
        let profile = azalea_auth::get_profile(&client, &res.minecraft_access_token).await?;

        Ok(Self {
            username: profile.name,
            access_token: Mutex::new(res.minecraft_access_token),
            uuid: profile.id,
            msa: Mutex::new(msa),
            certs: Mutex::new(None),
        })
    }
}
impl AccountTrait for MicrosoftWithAccessTokenAccount {
    fn username(&self) -> &str {
        &self.username
    }
    fn uuid(&self) -> Uuid {
        self.uuid
    }
    fn access_token(&self) -> Option<String> {
        Some(self.access_token.lock().to_owned())
    }
    fn certs(&self) -> Option<azalea_auth::certs::Certificates> {
        self.certs.lock().as_ref().cloned()
    }
    fn set_certs(&self, certs: azalea_auth::certs::Certificates) {
        *self.certs.lock() = Some(certs);
    }
    fn refresh(&self) -> BoxFuture<'_, Result<(), azalea_auth::AuthError>> {
        Box::pin(async {
            let msa_value = self.msa.lock().clone();
            let new_account = MicrosoftWithAccessTokenAccount::new(msa_value, None, None).await?;

            let new_access_token = new_account.access_token().unwrap();

            *self.access_token.lock() = new_access_token;
            *self.msa.lock() = new_account.msa.lock().clone();

            Ok(())
        })
    }
    fn join<'a>(
        &'a self,
        public_key: &'a [u8],
        private_key: &'a [u8; 16],
        server_id: &'a str,
        proxy: Option<reqwest::Proxy>,
    ) -> BoxFuture<'a, Result<(), ClientSessionServerError>> {
        Box::pin(async move {
            let access_token = self.access_token.lock().clone();
            sessionserver::join(SessionServerJoinOpts {
                access_token: &access_token,
                public_key,
                private_key,
                uuid: &self.uuid(),
                server_id,
                proxy,
            })
            .await
        })
    }
}

impl Account {
    #[cfg(feature = "online-mode")]
    pub async fn microsoft(cache_key: &str) -> Result<Self, azalea_auth::AuthError> {
        MicrosoftAccount::new(cache_key, default_account_opts(None, None))
            .await
            .map(Account::from)
    }

    #[cfg(feature = "online-mode")]
    pub async fn microsoft_with_opts(
        cache_key: &str,
        auth_opts: MicrosoftAccountOpts,
    ) -> Result<Self, azalea_auth::AuthError> {
        MicrosoftAccount::new(cache_key, auth_opts)
            .await
            .map(Account::from)
    }

    #[cfg(feature = "online-mode")]
    #[deprecated(note = "Use `Account::microsoft_with_opts` instead.")]
    pub async fn microsoft_with_custom_client_id_and_scope(
        cache_key: &str,
        client_id: Option<&str>,
        scope: Option<&str>,
    ) -> Result<Self, azalea_auth::AuthError> {
        MicrosoftAccount::new(cache_key, default_account_opts(client_id, scope))
            .await
            .map(Account::from)
    }

    #[cfg(feature = "online-mode")]
    pub async fn with_microsoft_access_token(
        msa: azalea_auth::cache::ExpiringValue<AccessTokenResponse>,
    ) -> Result<Self, azalea_auth::AuthError> {
        Self::with_microsoft_access_token_and_custom_client_id_and_scope(msa, None, None).await
    }

    #[cfg(feature = "online-mode")]
    pub async fn with_microsoft_access_token_and_custom_client_id_and_scope(
        msa: azalea_auth::cache::ExpiringValue<AccessTokenResponse>,
        client_id: Option<&str>,
        scope: Option<&str>,
    ) -> Result<Self, azalea_auth::AuthError> {
        MicrosoftWithAccessTokenAccount::new(msa, client_id, scope)
            .await
            .map(Account::from)
    }
}
