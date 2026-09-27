use uuid::Uuid;

use crate::account::{Account, AccountTrait};

#[derive(Debug)]
pub struct OfflineAccount {
    username: String,
}
impl AccountTrait for OfflineAccount {
    fn username(&self) -> &str {
        &self.username
    }
    fn uuid(&self) -> Uuid {
        azalea_crypto::offline::generate_uuid(&self.username)
    }
    fn access_token(&self) -> Option<String> {
        None
    }
}

impl Account {
    pub fn offline(username: &str) -> Self {
        OfflineAccount {
            username: username.to_owned(),
        }
        .into()
    }
}
