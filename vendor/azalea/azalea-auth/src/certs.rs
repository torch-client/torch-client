use base64::Engine;
use chrono::{DateTime, Utc};
use rsa::{RsaPrivateKey, pkcs8::DecodePrivateKey};
use serde::Deserialize;
use thiserror::Error;
use tracing::trace;

#[derive(Debug, Error)]
pub enum FetchCertificatesError {
    #[error("Http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Couldn't parse pkcs8 private key: {0}")]
    Pkcs8(#[from] rsa::pkcs8::Error),
}

pub async fn fetch_certificates(
    minecraft_access_token: &str,
) -> Result<Certificates, FetchCertificatesError> {
    let client = crate::http_client().unwrap_or_else(reqwest::Client::new);

    let res = client
        .post("https://api.minecraftservices.com/player/certificates")
        .header("Authorization", format!("Bearer {minecraft_access_token}"))
        .send()
        .await?
        .json::<CertificatesResponse>()
        .await?;
    trace!("{:?}", res);

    let private_key_pem_base64 = res
        .key_pair
        .private_key
        .lines()
        .skip(1)
        .take_while(|line| !line.starts_with('-'))
        .collect::<String>();
    let private_key_der = base64::engine::general_purpose::STANDARD
        .decode(private_key_pem_base64)
        .unwrap();

    let public_key_pem_base64 = res
        .key_pair
        .public_key
        .lines()
        .skip(1)
        .take_while(|line| !line.starts_with('-'))
        .collect::<String>();
    let public_key_der = base64::engine::general_purpose::STANDARD
        .decode(public_key_pem_base64)
        .unwrap();

    let private_key = RsaPrivateKey::from_pkcs8_der(&private_key_der).unwrap();

    let certificates = Certificates {
        private_key,
        public_key_der,

        signature_v1: base64::engine::general_purpose::STANDARD
            .decode(&res.public_key_signature)
            .unwrap(),
        signature_v2: base64::engine::general_purpose::STANDARD
            .decode(&res.public_key_signature_v2)
            .unwrap(),

        expires_at: res.expires_at,
        refresh_after: res.refreshed_after,
    };

    Ok(certificates)
}

#[derive(Clone, Debug)]
pub struct Certificates {
    pub private_key: RsaPrivateKey,
    pub public_key_der: Vec<u8>,

    pub signature_v1: Vec<u8>,
    pub signature_v2: Vec<u8>,

    pub expires_at: DateTime<Utc>,
    pub refresh_after: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CertificatesResponse {
    #[serde(rename = "keyPair")]
    pub key_pair: KeyPairResponse,

    #[serde(rename = "publicKeySignature")]
    pub public_key_signature: String,

    #[serde(rename = "publicKeySignatureV2")]
    pub public_key_signature_v2: String,

    #[serde(rename = "expiresAt")]
    pub expires_at: DateTime<Utc>,

    #[serde(rename = "refreshedAfter")]
    pub refreshed_after: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct KeyPairResponse {
    #[serde(rename = "privateKey")]
    pub private_key: String,

    #[serde(rename = "publicKey")]
    pub public_key: String,
}
