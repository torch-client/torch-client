use azalea::Client;
use azalea_protocol::packets::game::s_chat::{LastSeenMessagesUpdate, ServerboundChat};

pub(crate) fn ready(bot: &Client) -> bool {
    #[cfg(feature = "online_mode")]
    {
        use azalea::chat_signing::ChatSigningSession;

        capable(bot)
            && bot
                .ecs
                .read()
                .get::<ChatSigningSession>(bot.entity)
                .is_some()
    }
    #[cfg(not(feature = "online_mode"))]
    {
        let _ = bot;
        false
    }
}

pub(crate) fn capable(bot: &Client) -> bool {
    #[cfg(feature = "online_mode")]
    {
        use azalea::account::Account;

        bot.ecs
            .read()
            .get::<Account>(bot.entity)
            .is_some_and(|account| account.certs().is_some())
    }
    #[cfg(not(feature = "online_mode"))]
    {
        let _ = bot;
        false
    }
}

pub(crate) fn chat_packet(bot: &Client, content: String, signing: bool) -> ServerboundChat {
    let timestamp = crate::platform::time::SystemTime::now();
    let millis = timestamp
        .duration_since(crate::platform::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);

    #[cfg(feature = "online_mode")]
    let (salt, signature) = if signing {
        use azalea::account::Account;
        use azalea::chat_signing::ChatSigningSession;
        use azalea::client_chat::handler::create_signature;

        let salt = salt();
        let mut ecs = bot.ecs.write();
        let account = ecs.get::<Account>(bot.entity).cloned();
        let signature = if let Some(account) = account
            && account.certs().is_some()
            && let Some(mut session) = ecs.get_mut::<ChatSigningSession>(bot.entity)
        {
            Some(create_signature(
                &account,
                &mut session,
                salt,
                timestamp,
                &content,
            ))
        } else {
            None
        };
        (salt, signature)
    } else {
        (0, None)
    };
    #[cfg(not(feature = "online_mode"))]
    let (salt, signature) = {
        let _ = (bot, signing);
        (0u64, None)
    };

    ServerboundChat {
        message: content,
        timestamp: millis,
        salt,
        signature,
        last_seen_messages: LastSeenMessagesUpdate::default(),
    }
}

#[cfg(feature = "online_mode")]
fn salt() -> u64 {
    use crate::platform::time::{SystemTime, UNIX_EPOCH};
    const DRAW: u32 = 1 << 30;

    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let mut rng = crate::util::javarandom::JavaRandom::new(nanos as i64);
    ((rng.next_int(DRAW) as u64) << 32) | rng.next_int(DRAW) as u64
}

pub(crate) struct ChatSigningPlugin;

impl azalea::app::Plugin for ChatSigningPlugin {
    #[cfg_attr(
        not(feature = "online_mode"),
        expect(unused_variables, reason = "no system to add without the feature")
    )]
    fn build(&self, app: &mut azalea::app::App) {
        #[cfg(feature = "online_mode")]
        {
            use azalea::ecs::prelude::IntoScheduleConfigs as _;
            app.add_systems(
                azalea::app::Update,
                (
                    forget_session_on_login,
                    request_certs,
                    azalea::chat_signing::poll_request_certs_task,
                    send_chat_session,
                )
                    .chain()
                    .before(azalea::client_chat::handler::handle_send_chat_kind_event),
            );
        }
    }
}

#[cfg(feature = "online_mode")]
fn signing_wanted() -> bool {
    crate::SHARED.get().is_some_and(|shared| {
        let s = shared.lock().unwrap();
        s.chat_signing_allowed || s.session.chat_signing_session
    })
}

#[cfg(feature = "online_mode")]
fn request_certs(
    mut commands: azalea::ecs::prelude::Commands,
    query: azalea::ecs::prelude::Query<
        (
            azalea::ecs::prelude::Entity,
            &azalea::account::Account,
            Option<&azalea::chat_signing::OnlyRefreshCertsAfter>,
        ),
        (
            azalea::ecs::prelude::Without<azalea::chat_signing::RequestCertsTask>,
            azalea::ecs::prelude::With<azalea::InGameState>,
            azalea::ecs::prelude::With<azalea::login::IsAuthenticated>,
        ),
    >,
) {
    use azalea::chat_signing::{OnlyRefreshCertsAfter, RequestCertsTask};

    for (entity, account, backoff) in &query {
        if let Some(backoff) = backoff
            && backoff.refresh_at > crate::platform::time::Instant::now()
        {
            continue;
        }
        if account
            .certs()
            .is_some_and(|certs| certs.expires_at.timestamp_millis() > now_millis())
        {
            continue;
        }
        let Some(access_token) = account.access_token() else {
            continue;
        };
        let task = azalea::bevy_tasks::IoTaskPool::get().spawn(azalea::compat(async move {
            azalea_auth::certs::fetch_certificates(&access_token).await
        }));
        commands
            .entity(entity)
            .insert(RequestCertsTask(task))
            .remove::<OnlyRefreshCertsAfter>();
    }
}

#[cfg(feature = "online_mode")]
fn send_chat_session(
    mut commands: azalea::ecs::prelude::Commands,
    query: azalea::ecs::prelude::Query<
        (
            azalea::ecs::prelude::Entity,
            &azalea::chat_signing::QueuedCertsToSend,
        ),
        azalea::ecs::prelude::With<azalea::login::IsAuthenticated>,
    >,
) {
    use azalea::chat_signing::{ChatSigningSession, QueuedCertsToSend};
    use azalea_protocol::packets::game::ServerboundChatSessionUpdate;
    use azalea_protocol::packets::game::s_chat_session_update::{
        ProfilePublicKeyData, RemoteChatSessionData,
    };

    if query.is_empty() || !signing_wanted() {
        return;
    }
    for (entity, queued) in &query {
        let certs = &queued.certs;
        let session_id = uuid::Uuid::new_v4();
        commands.trigger(azalea::packet::game::SendGamePacketEvent::new(
            entity,
            ServerboundChatSessionUpdate {
                chat_session: RemoteChatSessionData {
                    session_id,
                    profile_public_key: ProfilePublicKeyData {
                        expires_at: certs.expires_at.timestamp_millis() as u64,
                        key: certs.public_key_der.clone(),
                        key_signature: certs.signature_v2.clone(),
                    },
                },
            },
        ));
        commands
            .entity(entity)
            .remove::<QueuedCertsToSend>()
            .insert(ChatSigningSession {
                session_id,
                messages_sent: 0,
            });
    }
}

#[cfg(feature = "online_mode")]
fn forget_session_on_login(
    mut commands: azalea::ecs::prelude::Commands,
    mut events: azalea::ecs::prelude::MessageReader<azalea::packet::game::ReceiveGamePacketEvent>,
    query: azalea::ecs::prelude::Query<&azalea::account::Account>,
) {
    use azalea::chat_signing::{ChatSigningSession, QueuedCertsToSend};
    use azalea_protocol::packets::game::ClientboundGamePacket;

    for event in events.read() {
        if !matches!(event.packet.as_ref(), ClientboundGamePacket::Login(_)) {
            continue;
        }
        let Ok(account) = query.get(event.entity) else {
            continue;
        };
        commands
            .entity(event.entity)
            .remove::<ChatSigningSession>()
            .remove::<QueuedCertsToSend>();
        if let Some(certs) = account.certs() {
            commands
                .entity(event.entity)
                .insert(QueuedCertsToSend { certs });
        }
    }
}

#[cfg(feature = "online_mode")]
fn now_millis() -> i64 {
    use crate::platform::time::{SystemTime, UNIX_EPOCH};

    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
