use std::{
    collections::HashMap,
    path::PathBuf,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;
use tokio::time::sleep;
use tracing::{error, trace};
use uuid::Uuid;

use crate::cache::{self, CachedAccount, ExpiringValue};

#[derive(Default)]
pub struct AuthOpts<'a> {
    pub check_ownership: bool,
    pub cache_file: Option<PathBuf>,
    pub client_id: Option<&'a str>,
    pub scope: Option<&'a str>,
}

#[derive(Debug, Error)]
pub enum AuthError {
    #[error(
        "The Minecraft API is indicating that you don't own the game. \
        If you're using Xbox Game Pass, set `check_ownership` to false in the auth options."
    )]
    DoesNotOwnGame,
    #[error("Error getting Microsoft auth token: {0}")]
    GetMicrosoftAuthToken(#[from] GetMicrosoftAuthTokenError),
    #[error("Error refreshing Microsoft auth token: {0}")]
    RefreshMicrosoftAuthToken(#[from] RefreshMicrosoftAuthTokenError),
    #[error("Error getting Xbox Live auth token: {0}")]
    GetXboxLiveAuthToken(#[from] MinecraftXstsAuthError),
    #[error("Error getting Minecraft profile: {0}")]
    GetMinecraftProfile(#[from] GetProfileError),
    #[error("Error checking ownership: {0}")]
    CheckOwnership(#[from] CheckOwnershipError),
    #[error("Error getting Minecraft auth token: {0}")]
    GetMinecraftAuthToken(#[from] MinecraftAuthError),
    #[error("Error authenticating with Xbox Live: {0}")]
    GetXboxLiveAuth(#[from] XboxLiveAuthError),
}

pub async fn auth(cache_key: &str, opts: AuthOpts<'_>) -> Result<AuthResult, AuthError> {
    let cached_account = if let Some(cache_file) = &opts.cache_file {
        cache::get_account_in_cache(cache_file, cache_key).await
    } else {
        None
    };

    if let Some(account) = &cached_account
        && !account.mca.is_expired()
    {

        Ok(AuthResult {
            access_token: account.mca.data.access_token.clone(),
            profile: account.profile.clone(),
        })
    } else {
        let client_id = opts.client_id.unwrap_or(CLIENT_ID);
        let scope = opts.scope.unwrap_or(SCOPE);

        let client = reqwest::Client::new();
        let mut msa = if let Some(account) = cached_account {
            account.msa
        } else {
            interactive_get_ms_auth_token(&client, cache_key, Some(client_id), Some(scope)).await?
        };
        if msa.is_expired() {
            trace!("refreshing Microsoft auth token");
            match refresh_ms_auth_token(
                &client,
                &msa.data.refresh_token,
                opts.client_id,
                opts.scope,
            )
            .await
            {
                Ok(new_msa) => msa = new_msa,
                Err(e) => {
                    error!("Error refreshing Microsoft auth token: {}", e);
                    msa = interactive_get_ms_auth_token(
                        &client,
                        cache_key,
                        Some(client_id),
                        Some(scope),
                    )
                    .await?;
                }
            }
        }

        let msa_token = &msa.data.access_token;
        trace!("Got access token: {msa_token}");

        let res = get_minecraft_token(&client, msa_token).await?;

        if opts.check_ownership {
            let has_game = check_ownership(&client, &res.minecraft_access_token).await?;
            if !has_game {
                return Err(AuthError::DoesNotOwnGame);
            }
        }

        let profile: ProfileResponse = get_profile(&client, &res.minecraft_access_token).await?;

        if let Some(cache_file) = opts.cache_file
            && let Err(e) = cache::set_account_in_cache(
                &cache_file,
                cache_key,
                CachedAccount {
                    cache_key: cache_key.to_owned(),
                    mca: res.mca,
                    msa,
                    xbl: res.xbl,
                    profile: profile.clone(),
                },
            )
            .await
        {
            error!("{}", e);
        }

        Ok(AuthResult {
            access_token: res.minecraft_access_token,
            profile,
        })
    }
}

pub async fn get_minecraft_token(
    client: &reqwest::Client,
    msa: &str,
) -> Result<MinecraftTokenResponse, AuthError> {
    let xbl_auth = auth_with_xbox_live(client, msa).await?;

    let xsts_token = obtain_xsts_for_minecraft(
        client,
        &xbl_auth
            .get()
            .expect("Xbox Live auth token shouldn't have expired yet")
            .token,
    )
    .await?;

    let mca = auth_with_minecraft(client, &xbl_auth.data.user_hash, &xsts_token).await?;

    let minecraft_access_token: String = mca
        .get()
        .expect("Minecraft auth shouldn't have expired yet")
        .access_token
        .to_string();

    Ok(MinecraftTokenResponse {
        mca,
        xbl: xbl_auth,
        minecraft_access_token,
    })
}

#[derive(Debug)]
pub struct MinecraftTokenResponse {
    pub mca: ExpiringValue<MinecraftAuthResponse>,
    pub xbl: ExpiringValue<XboxLiveAuth>,
    pub minecraft_access_token: String,
}

#[derive(Debug)]
pub struct AuthResult {
    pub access_token: String,
    pub profile: ProfileResponse,
}

#[derive(Debug, Deserialize)]
pub struct DeviceCodeResponse {
    pub user_code: String,
    pub device_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AccessTokenResponse {
    pub token_type: String,
    pub expires_in: u64,
    pub scope: String,
    pub access_token: String,
    pub refresh_token: String,
    pub user_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct XboxLiveAuthResponse {
    pub issue_instant: String,
    pub not_after: String,
    pub token: String,
    pub display_claims: HashMap<String, Vec<HashMap<String, String>>>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct XboxLiveAuth {
    pub token: String,
    pub user_hash: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct MinecraftAuthResponse {
    pub username: String,
    pub roles: Vec<String>,
    pub access_token: String,
    pub token_type: String,
    pub expires_in: u64,
}

#[derive(Debug, Deserialize)]
pub struct GameOwnershipResponse {
    pub items: Vec<GameOwnershipItem>,
    pub signature: String,
    #[serde(rename = "keyId")]
    pub key_id: String,
}

#[derive(Debug, Deserialize)]
pub struct GameOwnershipItem {
    pub name: String,
    pub signature: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProfileResponse {
    pub id: Uuid,
    pub name: String,
    pub skins: Vec<serde_json::Value>,
    pub capes: Vec<serde_json::Value>,
}

const CLIENT_ID: &str = "00000000441cc96b";
const SCOPE: &str = "service::user.auth.xboxlive.com::MBI_SSL";

#[derive(Debug, Error)]
pub enum GetMicrosoftAuthTokenError {
    #[error("Http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Authentication timed out")]
    Timeout,
}

pub async fn get_ms_link_code(
    client: &reqwest::Client,
    client_id: Option<&str>,
    scope: Option<&str>,
) -> Result<DeviceCodeResponse, GetMicrosoftAuthTokenError> {
    let client_id = if let Some(c) = client_id {
        c
    } else {
        CLIENT_ID
    };

    let scope = if let Some(c) = scope { c } else { SCOPE };

    Ok(client
        .post("https://login.live.com/oauth20_connect.srf")
        .form(&[
            ("scope", scope),
            ("client_id", client_id),
            ("response_type", "device_code"),
        ])
        .send()
        .await?
        .json::<DeviceCodeResponse>()
        .await?)
}

pub async fn get_ms_auth_token(
    client: &reqwest::Client,
    res: DeviceCodeResponse,
    client_id: Option<&str>,
) -> Result<ExpiringValue<AccessTokenResponse>, GetMicrosoftAuthTokenError> {
    let client_id = if let Some(c) = client_id {
        c
    } else {
        CLIENT_ID
    };

    let login_expires_at = Instant::now() + Duration::from_secs(res.expires_in);

    while Instant::now() < login_expires_at {
        sleep(Duration::from_secs(res.interval)).await;

        trace!("Polling to check if user has logged in...");
        let res = client
            .post(format!(
                "https://login.live.com/oauth20_token.srf?client_id={client_id}"
            ))
            .form(&[
                ("client_id", client_id),
                ("device_code", &res.device_code),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ])
            .send()
            .await?
            .json::<AccessTokenResponse>()
            .await;
        if let Ok(access_token_response) = res {
            trace!("access_token_response: {:?}", access_token_response);
            let expires_at =
                SystemTime::now() + Duration::from_secs(access_token_response.expires_in);
            return Ok(ExpiringValue {
                data: access_token_response,
                expires_at: expires_at
                    .duration_since(UNIX_EPOCH)
                    .expect("Time went backwards")
                    .as_secs(),
            });
        }
    }

    Err(GetMicrosoftAuthTokenError::Timeout)
}

pub async fn interactive_get_ms_auth_token(
    client: &reqwest::Client,
    cache_key: &str,
    client_id: Option<&str>,
    scope: Option<&str>,
) -> Result<ExpiringValue<AccessTokenResponse>, GetMicrosoftAuthTokenError> {
    let res = get_ms_link_code(client, client_id, scope).await?;
    trace!("Device code response: {:?}", res);
    let verification_uri = &res.verification_uri;
    let user_code = &res.user_code;
    println!(
        "Go to \x1b[1m{verification_uri}?otc={user_code}\x1b[m and log in for \x1b[1m{cache_key}\x1b[m",
    );

    get_ms_auth_token(client, res, client_id).await
}

#[derive(Debug, Error)]
pub enum RefreshMicrosoftAuthTokenError {
    #[error("Http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Error parsing JSON: {0}")]
    Json(#[from] serde_json::Error),
}

pub async fn refresh_ms_auth_token(
    client: &reqwest::Client,
    refresh_token: &str,
    client_id: Option<&str>,
    scope: Option<&str>,
) -> Result<ExpiringValue<AccessTokenResponse>, RefreshMicrosoftAuthTokenError> {
    let client_id = client_id.unwrap_or(CLIENT_ID);
    let scope = scope.unwrap_or(SCOPE);

    let access_token_response_text = client
        .post("https://login.live.com/oauth20_token.srf")
        .form(&[
            ("scope", scope),
            ("client_id", client_id),
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
        ])
        .send()
        .await?
        .text()
        .await?;
    let access_token_response: AccessTokenResponse =
        serde_json::from_str(&access_token_response_text)?;

    let expires_at = SystemTime::now() + Duration::from_secs(access_token_response.expires_in);
    Ok(ExpiringValue {
        data: access_token_response,
        expires_at: expires_at
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs(),
    })
}

#[derive(Debug, Error)]
pub enum XboxLiveAuthError {
    #[error("Http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Invalid expiry date: {0}")]
    InvalidExpiryDate(String),
}

async fn auth_with_xbox_live(
    client: &reqwest::Client,
    access_token: &str,
) -> Result<ExpiringValue<XboxLiveAuth>, XboxLiveAuthError> {
    let auth_json = json!({
        "Properties": {
            "AuthMethod": "RPS",
            "SiteName": "user.auth.xboxlive.com",
            "RpsTicket": access_token
        },
        "RelyingParty": "http://auth.xboxlive.com",
        "TokenType": "JWT"
    });
    let payload = auth_json.to_string();
    trace!("auth_json: {:#?}", auth_json);
    let res = client
        .post("https://user.auth.xboxlive.com/user/authenticate")
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .header("x-xbl-contract-version", "1")
        .body(payload)
        .send()
        .await?
        .json::<XboxLiveAuthResponse>()
        .await?;
    trace!("Xbox Live auth response: {:?}", res);

    let expires_at = DateTime::parse_from_rfc3339(&res.not_after)
        .map_err(|e| XboxLiveAuthError::InvalidExpiryDate(format!("{}: {e}", res.not_after)))?
        .with_timezone(&Utc)
        .timestamp() as u64;
    Ok(ExpiringValue {
        data: XboxLiveAuth {
            token: res.token,
            user_hash: res.display_claims["xui"].first().unwrap()["uhs"].clone(),
        },
        expires_at,
    })
}

#[derive(Debug, Error)]
pub enum MinecraftXstsAuthError {
    #[error("Http error: {0}")]
    Http(#[from] reqwest::Error),
}

async fn obtain_xsts_for_minecraft(
    client: &reqwest::Client,
    xbl_auth_token: &str,
) -> Result<String, MinecraftXstsAuthError> {
    let res = client
        .post("https://xsts.auth.xboxlive.com/xsts/authorize")
        .header("Accept", "application/json")
        .json(&json!({
            "Properties": {
                "SandboxId": "RETAIL",
                "UserTokens": [xbl_auth_token.to_owned()]
            },
            "RelyingParty": "rp://api.minecraftservices.com/",
            "TokenType": "JWT"
        }))
        .send()
        .await?
        .json::<XboxLiveAuthResponse>()
        .await?;
    trace!("Xbox Live auth response (for XSTS): {:?}", res);

    Ok(res.token)
}

#[derive(Debug, Error)]
pub enum MinecraftAuthError {
    #[error("Http error: {0}")]
    Http(#[from] reqwest::Error),
}

async fn auth_with_minecraft(
    client: &reqwest::Client,
    user_hash: &str,
    xsts_token: &str,
) -> Result<ExpiringValue<MinecraftAuthResponse>, MinecraftAuthError> {
    let res = client
        .post("https://api.minecraftservices.com/authentication/login_with_xbox")
        .header("Accept", "application/json")
        .json(&json!({
            "identityToken": format!("XBL3.0 x={user_hash};{xsts_token}")
        }))
        .send()
        .await?
        .json::<MinecraftAuthResponse>()
        .await?;
    trace!("{:?}", res);

    let expires_at = SystemTime::now() + Duration::from_secs(res.expires_in);
    Ok(ExpiringValue {
        data: res,
        expires_at: expires_at.duration_since(UNIX_EPOCH).unwrap().as_secs(),
    })
}

#[derive(Debug, Error)]
pub enum CheckOwnershipError {
    #[error("Http error: {0}")]
    Http(#[from] reqwest::Error),
}

pub async fn check_ownership(
    client: &reqwest::Client,
    minecraft_access_token: &str,
) -> Result<bool, CheckOwnershipError> {
    let res = client
        .get("https://api.minecraftservices.com/entitlements/mcstore")
        .header("Authorization", format!("Bearer {minecraft_access_token}"))
        .send()
        .await?
        .json::<GameOwnershipResponse>()
        .await?;
    trace!("{:?}", res);

    Ok(!res.items.is_empty())
}

#[derive(Debug, Error)]
pub enum GetProfileError {
    #[error("Http error: {0}")]
    Http(#[from] reqwest::Error),
}

pub async fn get_profile(
    client: &reqwest::Client,
    minecraft_access_token: &str,
) -> Result<ProfileResponse, GetProfileError> {
    let res = client
        .get("https://api.minecraftservices.com/minecraft/profile")
        .header("Authorization", format!("Bearer {minecraft_access_token}"))
        .send()
        .await?
        .json::<ProfileResponse>()
        .await?;
    trace!("{:?}", res);

    Ok(res)
}
