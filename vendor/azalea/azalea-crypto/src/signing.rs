use std::time::{SystemTime, UNIX_EPOCH};

use azalea_buf::AzBuf;
use rsa::{
    RsaPrivateKey,
    signature::{RandomizedSigner, SignatureEncoding},
};
use sha2::Sha256;
use uuid::Uuid;

#[derive(AzBuf, Clone, Debug)]
pub struct SaltSignaturePair {
    pub salt: u64,
    pub signature: Vec<u8>,
}

#[derive(AzBuf, Clone, Debug, PartialEq)]
pub struct MessageSignature {
    pub bytes: [u8; 256],
}

#[derive(AzBuf, Clone, Debug, PartialEq)]
pub struct SignedMessageHeader {
    pub previous_signature: Option<MessageSignature>,
    pub sender: Uuid,
}

pub fn make_salt() -> u64 {
    rand::random()
}

pub struct SignChatMessageOptions {
    pub account_uuid: Uuid,
    pub chat_session_uuid: Uuid,

    pub message_index: u32,

    pub salt: u64,

    pub timestamp: SystemTime,

    pub message: String,

    pub private_key: RsaPrivateKey,
}

pub fn sign_chat_message(opts: &SignChatMessageOptions) -> MessageSignature {
    let mut data_to_sign = Vec::new();
    1i32.azalea_write(&mut data_to_sign).unwrap();
    opts.account_uuid.azalea_write(&mut data_to_sign).unwrap();
    opts.chat_session_uuid
        .azalea_write(&mut data_to_sign)
        .unwrap();
    opts.message_index.azalea_write(&mut data_to_sign).unwrap();
    opts.salt.azalea_write(&mut data_to_sign).unwrap();

    let seconds_since_epoch = opts
        .timestamp
        .duration_since(UNIX_EPOCH)
        .expect("timestamp must be after epoch")
        .as_secs();
    seconds_since_epoch.azalea_write(&mut data_to_sign).unwrap();

    let message_len: u32 = opts.message.len().try_into().unwrap();
    message_len.azalea_write(&mut data_to_sign).unwrap();
    data_to_sign.extend_from_slice(opts.message.as_bytes());

    0i32.azalea_write(&mut data_to_sign).unwrap();

    let signing_key = rsa::pkcs1v15::SigningKey::<Sha256>::new(opts.private_key.clone());
    let mut rng = rand::rng();
    let signature = signing_key
        .sign_with_rng(&mut rng, &data_to_sign)
        .to_bytes();

    MessageSignature {
        bytes: signature
            .as_ref()
            .try_into()
            .expect("signature must be 256 bytes"),
    }
}
