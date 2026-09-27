use std::sync::{Mutex, OnceLock};

use crate::gui::accountlist::{self, AccountKind};
use crate::{log_error, log_info};

#[derive(Clone)]
pub enum Auth {
    Starting,
    Prompt {
        url: std::sync::Arc<str>,
        code: std::sync::Arc<str>,
    },
    Done {
        name: String,
        uuid: String,
        token: String,
        session: String,
        session_expires: u64,
    },
    Failed(std::sync::Arc<str>),
}

impl std::fmt::Debug for Auth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Starting => write!(f, "Starting"),
            Self::Prompt { url, code } => f
                .debug_struct("Prompt")
                .field("url", url)
                .field("code", code)
                .finish(),
            Self::Done {
                name,
                uuid,
                token: _,
                session: _,
                session_expires,
            } => f
                .debug_struct("Done")
                .field("name", name)
                .field("uuid", uuid)
                .field("token", &"<redacted>")
                .field("session", &"<redacted>")
                .field("session_expires", session_expires)
                .finish(),
            Self::Failed(reason) => f.debug_tuple("Failed").field(reason).finish(),
        }
    }
}

#[derive(Default)]
struct Slot {
    generation: u64,
    state: Option<Auth>,
}

fn slot() -> &'static Mutex<Slot> {
    static SLOT: OnceLock<Mutex<Slot>> = OnceLock::new();
    SLOT.get_or_init(Default::default)
}

pub fn begin() {
    let generation = {
        let mut slot = slot().lock().unwrap();
        slot.generation += 1;
        slot.state = Some(Auth::Starting);
        slot.generation
    };
    log_info!("auth", "signing in with Microsoft");
    crate::platform::executor::spawn_detached(async move { run(generation).await });
}

pub fn state() -> Option<Auth> {
    slot().lock().unwrap().state.clone()
}

pub fn cancel() {
    log_info!("auth", "sign-in cancelled");
    clear();
}

pub fn clear() {
    let mut slot = slot().lock().unwrap();
    slot.generation += 1;
    slot.state = None;
}

#[cfg(not(target_arch = "wasm32"))]
static CLI_SESSION: OnceLock<CliSession> = OnceLock::new();

#[cfg(not(target_arch = "wasm32"))]
struct CliSession {
    name: String,
    uuid: uuid::Uuid,
    token: String,
}

#[cfg(not(target_arch = "wasm32"))]
pub fn sign_in_with_token(token: String) -> Result<String, String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("Could not start a runtime to authenticate with: {e}"))?;
    let profile = runtime
        .block_on(azalea_auth::get_profile(client(), &token))
        .map_err(|e| {
            format!(
                "The access token was not accepted by api.minecraftservices.com ({e}). It has to \
                 be a Minecraft session token, the `access_token` of a `login_with_xbox` reply, \
                 rather than a Microsoft OAuth token, and it has to still be valid."
            )
        })?;
    let name = profile.name.clone();
    log_info!(
        "auth",
        "the access token belongs to {name} ({})",
        profile.id
    );
    let _ = CLI_SESSION.set(CliSession {
        name: name.clone(),
        uuid: profile.id,
        token,
    });
    Ok(name)
}

fn publish(generation: u64, state: Auth) {
    let mut slot = slot().lock().unwrap();
    if slot.generation == generation {
        slot.state = Some(state);
    }
}

async fn run(generation: u64) {
    let client = client();

    let code = match azalea_auth::get_ms_link_code(client, None, None).await {
        Ok(code) => code,
        Err(e) => return fail(generation, format!("Could not reach Microsoft: {e}")),
    };

    let url = format!("{}?otc={}", code.verification_uri, code.user_code);
    let user_code = code.user_code.clone();

    log_info!("auth", "open {url}");
    log_info!(
        "auth",
        "or go to {} and enter the code {user_code}",
        code.verification_uri
    );
    log_info!(
        "auth",
        "waiting for sign-in, this code expires in {} minutes",
        code.expires_in / 60
    );
    publish(
        generation,
        Auth::Prompt {
            url: url.into(),
            code: user_code.into(),
        },
    );

    let msa = match azalea_auth::get_ms_auth_token(client, code, None).await {
        Ok(msa) => msa,
        Err(e) => return fail(generation, format!("Sign-in did not complete: {e}")),
    };
    let token = msa.data.refresh_token.clone();

    let minecraft = match azalea_auth::get_minecraft_token(client, &msa.data.access_token).await {
        Ok(minecraft) => minecraft,
        Err(e) => return fail(generation, format!("Microsoft accepted the code, but: {e}")),
    };
    let profile = match azalea_auth::get_profile(client, &minecraft.minecraft_access_token).await {
        Ok(profile) => profile,
        Err(e) => {
            return fail(
                generation,
                format!("Signed in, but could not read the profile: {e}"),
            );
        }
    };

    let name = profile.name;
    let uuid = profile.id.to_string();
    log_info!("auth", "signed in as {name} ({uuid})");
    publish(
        generation,
        Auth::Done {
            name,
            uuid,
            token,
            session: minecraft.minecraft_access_token,
            session_expires: minecraft.mca.expires_at,
        },
    );
}

fn fail(generation: u64, message: String) {
    log_error!("auth", "{message}");
    publish(generation, Auth::Failed(message.into()));
}

const EXPIRY_MARGIN: u64 = 60;

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub async fn account() -> Result<azalea::account::Account, String> {
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(session) = CLI_SESSION.get() {
        log_info!(
            "auth",
            "joining as {} (session token from the command line)",
            session.name
        );
        return Ok(MicrosoftAccount::new(
            session.name.clone(),
            session.uuid,
            session.token.clone(),
            String::new(),
        )
        .into());
    }

    let accounts = accountlist::load();
    let entry = match accounts.active() {
        Some(entry) if entry.kind == AccountKind::Microsoft => entry.clone(),
        _ => {
            return Ok(azalea::account::Account::offline(
                &crate::client::bot::username(),
            ));
        }
    };
    let (Some(refresh), Some(uuid)) = (entry.token.as_deref(), entry.uuid.as_deref()) else {
        return Err(format!(
            "The account {} has no saved sign-in. Remove it and sign in again.",
            entry.name
        ));
    };
    let uuid = uuid::Uuid::parse_str(uuid).map_err(|e| {
        format!(
            "The account {} has a uuid this client cannot read ({e}). Remove it and sign in again.",
            entry.name
        )
    })?;
    let refresh = refresh.to_string();

    let usable = entry
        .session_expires
        .is_some_and(|expires| expires > now() + EXPIRY_MARGIN);
    if let Some(session) = entry.session.clone().filter(|_| usable) {
        log_info!(
            "auth",
            "joining as {} (Microsoft, cached session)",
            entry.name
        );
        return Ok(MicrosoftAccount::new(entry.name, uuid, session, refresh).into());
    }

    log_info!("auth", "renewing the session for {}", entry.name);
    let session = sign_in(&refresh, &entry.name).await?;
    accountlist::update_tokens(
        &uuid.to_string(),
        &session.refresh,
        &session.token,
        session.expires,
    );
    log_info!("auth", "joining as {} (Microsoft)", entry.name);
    Ok(MicrosoftAccount::new(entry.name, uuid, session.token, session.refresh).into())
}

struct Session {
    token: String,
    expires: u64,
    refresh: String,
}

async fn sign_in(refresh: &str, name: &str) -> Result<Session, String> {
    let client = client();
    let msa = azalea_auth::refresh_ms_auth_token(client, refresh, None, None)
        .await
        .map_err(|e| format!("Could not refresh the sign-in for {name}: {e}"))?;
    let minecraft = azalea_auth::get_minecraft_token(client, &msa.data.access_token)
        .await
        .map_err(|e| format!("Could not sign in as {name}: {e}"))?;
    Ok(Session {
        token: minecraft.minecraft_access_token,
        expires: minecraft.mca.expires_at,
        refresh: msa.data.refresh_token,
    })
}

struct MicrosoftAccount {
    username: String,
    uuid: uuid::Uuid,
    access_token: parking_lot::Mutex<String>,
    refresh_token: parking_lot::Mutex<String>,
    certs: parking_lot::Mutex<Option<azalea_auth::certs::Certificates>>,
}

impl std::fmt::Debug for MicrosoftAccount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MicrosoftAccount")
            .field("username", &self.username)
            .field("uuid", &self.uuid)
            .field("access_token", &"<redacted>")
            .field("refresh_token", &"<redacted>")
            .field("certs", &self.certs.lock().is_some())
            .finish()
    }
}

impl MicrosoftAccount {
    fn new(
        username: String,
        uuid: uuid::Uuid,
        access_token: String,
        refresh_token: String,
    ) -> MicrosoftAccount {
        MicrosoftAccount {
            username,
            uuid,
            access_token: parking_lot::Mutex::new(access_token),
            refresh_token: parking_lot::Mutex::new(refresh_token),
            certs: parking_lot::Mutex::new(None),
        }
    }
}

impl azalea::account::AccountTrait for MicrosoftAccount {
    fn username(&self) -> &str {
        &self.username
    }

    fn uuid(&self) -> uuid::Uuid {
        self.uuid
    }

    fn access_token(&self) -> Option<String> {
        Some(self.access_token.lock().clone())
    }

    fn certs(&self) -> Option<azalea_auth::certs::Certificates> {
        self.certs.lock().as_ref().cloned()
    }

    fn set_certs(&self, certs: azalea_auth::certs::Certificates) {
        *self.certs.lock() = Some(certs);
    }

    fn refresh(
        &self,
    ) -> std::pin::Pin<Box<dyn Future<Output = Result<(), azalea_auth::AuthError>> + Send + '_>>
    {
        Box::pin(async move {
            let refresh = self.refresh_token.lock().clone();
            if refresh.is_empty() {
                log_error!(
                    "auth",
                    "the session token given on the command line has expired, and there is no \
                     refresh token to replace it with: pass a newer --access-token"
                );
            }
            let client = client();
            let msa = azalea_auth::refresh_ms_auth_token(client, &refresh, None, None).await?;
            let minecraft =
                azalea_auth::get_minecraft_token(client, &msa.data.access_token).await?;

            *self.access_token.lock() = minecraft.minecraft_access_token.clone();
            *self.refresh_token.lock() = msa.data.refresh_token.clone();
            accountlist::update_tokens(
                &self.uuid.to_string(),
                &msa.data.refresh_token,
                &minecraft.minecraft_access_token,
                minecraft.mca.expires_at,
            );
            log_info!("auth", "renewed the session for {}", self.username);
            Ok(())
        })
    }

    fn join<'a>(
        &'a self,
        public_key: &'a [u8],
        private_key: &'a [u8; 16],
        server_id: &'a str,
        proxy: Option<reqwest::Proxy>,
    ) -> std::pin::Pin<
        Box<
            dyn Future<Output = Result<(), azalea_auth::sessionserver::ClientSessionServerError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            let access_token = self.access_token.lock().clone();
            azalea_auth::sessionserver::join(azalea_auth::sessionserver::SessionServerJoinOpts {
                access_token: &access_token,
                public_key,
                private_key,
                uuid: &self.uuid,
                server_id,
                proxy,
            })
            .await
        })
    }
}

fn client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        crate::install_crypto_provider();
        reqwest::Client::new()
    })
}
