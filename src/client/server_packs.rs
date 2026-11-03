use std::sync::atomic::{AtomicBool, Ordering};

use azalea::ecs::prelude::*;
use azalea_protocol::packets::config::ClientboundConfigPacket;
use azalea_protocol::packets::game::ClientboundGamePacket;
use uuid::Uuid;

use crate::SHARED;
use crate::gui::serverlist::PackPolicy;

#[cfg(resource_packs)]
const MAX_PACK_SIZE: u64 = 262_144_000;

static BUSY: AtomicBool = AtomicBool::new(false);

#[cfg(resource_packs)]
static GENERATION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[cfg(resource_packs)]
pub(crate) fn generation() -> u64 {
    GENERATION.load(Ordering::Relaxed)
}

#[cfg(resource_packs)]
type Loaded = Result<
    (
        std::sync::Arc<crate::platform::assets::archive::OwnedLayer>,
        std::sync::Arc<crate::resourcepacks::ServerPack>,
    ),
    String,
>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(not(resource_packs), allow(dead_code))]
enum Reply {
    Loaded,
    Declined,
    FailedDownload,
    Accepted,
    Downloaded,
    InvalidUrl,
    FailedReload,
    Discarded,
}

#[derive(Clone)]
pub(crate) struct Prompt {
    pub(crate) required: bool,
    pub(crate) message: Option<String>,
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum PromptStatus {
    #[default]
    Pending,
    Allowed,
    Declined,
}

#[derive(Clone, Copy, PartialEq, Eq)]
#[cfg_attr(not(resource_packs), allow(dead_code))]
enum Removal {
    DownloadFailed,
    ActivationFailed,
    Declined,
    Discarded,
    ServerRemoved,
    ServerReplaced,
}

impl Removal {
    fn reply(self) -> Option<Reply> {
        match self {
            Removal::DownloadFailed => Some(Reply::FailedDownload),
            Removal::ActivationFailed => Some(Reply::FailedReload),
            Removal::Declined => Some(Reply::Declined),
            Removal::Discarded => Some(Reply::Discarded),
            Removal::ServerRemoved | Removal::ServerReplaced => None,
        }
    }
}

struct Request {
    id: Uuid,
    url: String,
    hash: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Download {
    Requested,
    #[cfg_attr(not(resource_packs), allow(dead_code))]
    Pending,
    #[cfg_attr(not(resource_packs), allow(dead_code))]
    Done,
}

struct Pack {
    #[cfg_attr(not(resource_packs), allow(dead_code))]
    serial: u64,
    id: Uuid,
    #[cfg_attr(not(resource_packs), allow(dead_code))]
    url: String,
    #[cfg_attr(not(resource_packs), allow(dead_code))]
    hash: Option<String>,
    removal: Option<Removal>,
    download: Download,
    active: bool,
    accepted: bool,
    #[cfg(resource_packs)]
    layer: Option<Loaded>,
}

impl Pack {
    fn remove(&mut self, reason: Removal) {
        self.removal.get_or_insert(reason);
    }

    fn should_be_active(&self) -> bool {
        self.accepted && self.download == Download::Done && self.removal.is_none()
    }
}

#[cfg(resource_packs)]
enum Fetched {
    Failed,
    Done(Loaded),
}

#[cfg(resource_packs)]
type Batch = std::sync::mpsc::Receiver<Vec<(u64, Fetched)>>;

#[derive(Default)]
pub struct ServerPacks {
    pub(crate) prompt: Option<Prompt>,
    pub(crate) open_prompt: bool,
    pub(crate) answer: Option<bool>,
    policy: PackPolicy,
    status: PromptStatus,
    waiting: Vec<Request>,
    packs: Vec<Pack>,
    next_serial: u64,
    replies: Vec<(Uuid, Reply)>,
    disconnect: bool,
    #[cfg(resource_packs)]
    batches: Vec<Batch>,
    #[cfg(resource_packs)]
    loading: Vec<(u64, Uuid)>,
    #[cfg(resource_packs)]
    mounted: bool,
}

#[cfg(resource_packs)]
impl Drop for ServerPacks {
    fn drop(&mut self) {
        if self.mounted {
            crate::platform::assets::archive::set_server(Vec::new());
            crate::renderer::reload::request();
            GENERATION.fetch_add(1, Ordering::Relaxed);
        }
    }
}

impl ServerPacks {
    pub(crate) fn begin(&mut self, policy: PackPolicy) {
        self.policy = policy;
        self.status = match policy {
            PackPolicy::Enabled => PromptStatus::Allowed,
            PackPolicy::Disabled => PromptStatus::Declined,
            PackPolicy::Prompt => PromptStatus::Pending,
        };
    }

    fn push(
        &mut self,
        id: Uuid,
        url: &str,
        hash: &str,
        required: bool,
        prompt: Option<&azalea::FormattedText>,
    ) {
        if !http_url(url) {
            self.replies.push((id, Reply::InvalidUrl));
            return;
        }
        let request = Request {
            id,
            url: url.to_owned(),
            hash: sha1_hash(hash),
        };
        let ask =
            self.policy == PackPolicy::Prompt || (required && self.policy == PackPolicy::Disabled);
        if !ask {
            self.push_pack(request);
            return;
        }
        let message = prompt.map(|text| {
            crate::client::chat_text::to_spans(text)
                .iter()
                .map(|span| span.text.as_str())
                .collect::<String>()
        });
        self.waiting.push(request);
        self.prompt = Some(Prompt { required, message });
        self.open_prompt = true;
    }

    fn take_answer(&mut self) {
        let Some(accepted) = self.answer.take() else {
            return;
        };
        let Some(prompt) = self.prompt.take() else {
            return;
        };
        if accepted {
            self.allow();
        } else {
            self.reject();
            self.disconnect |= prompt.required;
        }
        for request in std::mem::take(&mut self.waiting) {
            self.push_pack(request);
        }
    }

    fn push_pack(&mut self, request: Request) {
        if self.status == PromptStatus::Declined {
            self.replies.push((request.id, Reply::Declined));
            return;
        }
        for pack in &mut self.packs {
            if pack.id == request.id {
                pack.remove(Removal::ServerReplaced);
            }
        }
        self.next_serial += 1;
        let mut pack = Pack {
            serial: self.next_serial,
            id: request.id,
            url: request.url,
            hash: request.hash,
            removal: None,
            download: Download::Requested,
            active: false,
            accepted: false,
            #[cfg(resource_packs)]
            layer: None,
        };
        if self.status == PromptStatus::Allowed {
            pack.accepted = true;
            self.replies.push((pack.id, Reply::Accepted));
        }
        self.packs.push(pack);
    }

    fn allow(&mut self) {
        self.status = PromptStatus::Allowed;
        for pack in &mut self.packs {
            if !pack.accepted && pack.removal.is_none() {
                pack.accepted = true;
                self.replies.push((pack.id, Reply::Accepted));
            }
        }
    }

    fn reject(&mut self) {
        self.status = PromptStatus::Declined;
        for pack in &mut self.packs {
            if !pack.accepted {
                pack.remove(Removal::Declined);
            }
        }
    }

    fn pop(&mut self, id: Option<Uuid>) {
        match id {
            Some(id) => {
                if let Some(pack) = self
                    .packs
                    .iter_mut()
                    .find(|p| p.removal.is_none() && p.id == id)
                {
                    pack.remove(Removal::ServerRemoved);
                }
            }
            None => self
                .packs
                .iter_mut()
                .for_each(|p| p.remove(Removal::ServerRemoved)),
        }
    }

    fn tick(&mut self) {
        if !self.update_downloads() {
            self.reload_if_needed();
        }
        #[cfg(resource_packs)]
        self.answer_loaded();
        self.cleanup();
    }

    #[cfg(resource_packs)]
    fn answer_loaded(&mut self) {
        let (packs, replies) = (&self.packs, &mut self.replies);
        self.loading.retain(|&(ticket, id)| {
            let live = crate::renderer::reload::covers(ticket);
            if live && packs.iter().any(|p| p.id == id && p.active) {
                replies.push((id, Reply::Loaded));
            }
            !live
        });
    }

    fn busy(&self) -> bool {
        #[cfg(resource_packs)]
        if !self.batches.is_empty() || !self.loading.is_empty() {
            return true;
        }
        self.prompt.is_some()
            || self
                .packs
                .iter()
                .any(|p| p.removal.is_some() || p.active != p.should_be_active())
    }

    #[cfg(resource_packs)]
    fn update_downloads(&mut self) -> bool {
        use std::sync::mpsc::TryRecvError;
        let mut finished = Vec::new();
        self.batches.retain(|batch| match batch.try_recv() {
            Ok(results) => {
                finished.push(results);
                false
            }
            Err(TryRecvError::Empty) => true,
            Err(TryRecvError::Disconnected) => {
                finished.push(Vec::new());
                false
            }
        });
        for results in finished {
            self.on_download(results);
        }

        let mut in_progress = false;
        let mut requests = Vec::new();
        for pack in &mut self.packs {
            if pack.removal.is_some() || !pack.accepted {
                continue;
            }
            in_progress |= pack.download != Download::Done;
            if pack.download == Download::Requested {
                pack.download = Download::Pending;
                requests.push((pack.serial, pack.url.clone(), pack.hash.clone()));
            }
        }
        if !requests.is_empty() {
            self.batches.push(download(requests));
        }
        in_progress
    }

    #[cfg(not(resource_packs))]
    fn update_downloads(&mut self) -> bool {
        for pack in &mut self.packs {
            if pack.accepted && pack.removal.is_none() {
                pack.remove(Removal::ActivationFailed);
            }
        }
        false
    }

    #[cfg(resource_packs)]
    fn on_download(&mut self, results: Vec<(u64, Fetched)>) {
        let failed: Vec<u64> = results
            .iter()
            .filter(|(_, fetched)| matches!(fetched, Fetched::Failed))
            .map(|(serial, _)| *serial)
            .collect();
        if !failed.is_empty() || results.is_empty() {
            for pack in self.packs.iter_mut().filter(|p| !p.active) {
                let pending = pack.download == Download::Pending;
                if failed.contains(&pack.serial) || (results.is_empty() && pending) {
                    pack.remove(Removal::DownloadFailed);
                } else {
                    pack.remove(Removal::Discarded);
                }
            }
        }
        for (serial, fetched) in results {
            let Fetched::Done(layer) = fetched else {
                continue;
            };
            let Some(pack) = self.packs.iter_mut().find(|p| p.serial == serial) else {
                continue;
            };
            pack.download = Download::Done;
            pack.layer = Some(layer);
            if pack.removal.is_none() {
                self.replies.push((pack.id, Reply::Downloaded));
            }
        }
    }

    #[cfg(resource_packs)]
    fn reload_if_needed(&mut self) {
        if !self.packs.iter().any(|p| p.active != p.should_be_active()) {
            return;
        }
        let broken = self
            .packs
            .iter()
            .any(|p| p.should_be_active() && !p.active && !matches!(p.layer, Some(Ok(_))));
        if broken {
            for pack in &mut self.packs {
                if !pack.active {
                    let loading = pack.should_be_active();
                    pack.remove(if loading {
                        Removal::ActivationFailed
                    } else {
                        Removal::Discarded
                    });
                }
            }
        }
        let mut loaded = Vec::new();
        for pack in &mut self.packs {
            let should = pack.should_be_active();
            if should && !pack.active {
                loaded.push(pack.id);
            }
            pack.active = should;
        }
        let layers: Vec<_> = self
            .packs
            .iter()
            .rev()
            .filter(|p| p.active)
            .filter_map(|p| Some(p.layer.as_ref()?.as_ref().ok()?.0.clone()))
            .collect();
        crate::log_info!("resourcepacks", "{} server pack(s) mounted", layers.len());
        crate::platform::assets::archive::set_server(layers);
        self.mounted = true;
        GENERATION.fetch_add(1, Ordering::Relaxed);
        let ticket = crate::renderer::reload::request();
        self.loading
            .extend(loaded.into_iter().map(|id| (ticket, id)));
    }

    #[cfg(not(resource_packs))]
    fn reload_if_needed(&mut self) {}

    #[cfg(resource_packs)]
    pub(crate) fn shown(&self) -> Vec<std::sync::Arc<crate::resourcepacks::ServerPack>> {
        self.packs
            .iter()
            .rev()
            .filter(|p| p.active)
            .filter_map(|p| Some(p.layer.as_ref()?.as_ref().ok()?.1.clone()))
            .collect()
    }

    fn cleanup(&mut self) {
        let replies = &mut self.replies;
        self.packs.retain(|pack| match pack.removal {
            Some(reason) if !pack.active => {
                replies.extend(reason.reply().map(|reply| (pack.id, reply)));
                false
            }
            _ => true,
        });
    }
}

fn http_url(url: &str) -> bool {
    url.split_once("://").is_some_and(|(scheme, rest)| {
        !rest.is_empty()
            && (scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https"))
    })
}

fn sha1_hash(hash: &str) -> Option<String> {
    (hash.len() == 40 && hash.bytes().all(|b| b.is_ascii_hexdigit()))
        .then(|| hash.to_ascii_lowercase())
}

#[cfg(resource_packs)]
fn download(requests: Vec<(u64, String, Option<String>)>) -> Batch {
    let (tx, rx) = std::sync::mpsc::channel();
    #[cfg(target_arch = "wasm32")]
    crate::platform::executor::spawn_detached(fetch_all(requests, tx));
    #[cfg(not(target_arch = "wasm32"))]
    if let Err(e) = std::thread::Builder::new()
        .name("server-pack".into())
        .spawn(move || crate::platform::executor::block_on(fetch_all(requests, tx)))
    {
        crate::log_warn!("resourcepacks", "could not start a download thread: {e}");
    }
    rx
}

#[cfg(resource_packs)]
async fn fetch_all(
    requests: Vec<(u64, String, Option<String>)>,
    tx: std::sync::mpsc::Sender<Vec<(u64, Fetched)>>,
) {
    let mut results = Vec::with_capacity(requests.len());
    for (serial, url, hash) in requests {
        results.push((serial, fetch(&url, hash.as_deref()).await));
    }
    let _ = tx.send(results);
}

#[cfg(resource_packs)]
async fn fetch(url: &str, hash: Option<&str>) -> Fetched {
    use crate::platform::http;
    let mut under_cap = |got: u64, _total: u64| got <= MAX_PACK_SIZE;
    let bytes = match http::get_bytes(url, 1, &mut under_cap).await {
        Ok(http::Fetched::Body(bytes)) => bytes,
        Ok(http::Fetched::Cancelled) => {
            crate::log_warn!(
                "resourcepacks",
                "server pack {url} is over {MAX_PACK_SIZE} bytes"
            );
            return Fetched::Failed;
        }
        Err(e) => {
            crate::log_warn!("resourcepacks", "server pack {url} did not download: {e}");
            return Fetched::Failed;
        }
    };
    let actual = crate::util::hash::sha1_hex(&bytes);
    if let Some(expected) = hash
        && actual != expected
    {
        crate::log_warn!(
            "resourcepacks",
            "server pack {url} has hash {actual}, expected {expected}"
        );
        return Fetched::Failed;
    }
    crate::log_info!("resourcepacks", "server pack {url}: {} bytes", bytes.len());
    let loaded = crate::resourcepacks::server_pack(bytes, url, &actual)
        .map(|(layer, shown)| (std::sync::Arc::new(layer), std::sync::Arc::new(shown)));
    Fetched::Done(loaded.inspect_err(|e| {
        crate::log_warn!("resourcepacks", "server pack {url} is not mounted: {e}");
    }))
}

pub(crate) struct ServerPackPlugin;

impl azalea::app::Plugin for ServerPackPlugin {
    fn build(&self, app: &mut azalea::app::App) {
        app.add_systems(azalea::app::Update, update);
    }
}

fn update(
    mut pushes: MessageReader<azalea::packet::game::ResourcePackEvent>,
    mut game: MessageReader<azalea::packet::game::ReceiveGamePacketEvent>,
    mut config: MessageReader<azalea::packet::config::ReceiveConfigPacketEvent>,
    mut commands: Commands,
    mut disconnect: MessageWriter<azalea::disconnect::DisconnectEvent>,
    query: Query<(Entity, Option<&azalea::InConfigState>), With<azalea::local_player::WorldHolder>>,
) {
    let Some(shared) = SHARED.get() else {
        return;
    };
    let pops: Vec<Option<Uuid>> = game
        .read()
        .filter_map(|event| match event.packet.as_ref() {
            ClientboundGamePacket::ResourcePackPop(p) => Some(p.id),
            _ => None,
        })
        .chain(
            config
                .read()
                .filter_map(|event| match event.packet.as_ref() {
                    ClientboundConfigPacket::ResourcePackPop(p) => Some(p.id),
                    _ => None,
                }),
        )
        .collect();
    let pushes: Vec<_> = pushes.read().collect();
    if pushes.is_empty() && pops.is_empty() && !BUSY.load(Ordering::Relaxed) {
        return;
    }

    let (replies, leave) = {
        let mut s = shared.lock().unwrap();
        let packs = &mut s.session.server_packs;
        for push in pushes {
            packs.push(
                push.id,
                &push.url,
                &push.hash,
                push.required,
                push.prompt.as_ref(),
            );
        }
        for id in pops {
            packs.pop(id);
        }
        packs.take_answer();
        packs.tick();
        BUSY.store(packs.busy(), Ordering::Relaxed);
        (
            std::mem::take(&mut packs.replies),
            std::mem::take(&mut packs.disconnect),
        )
    };

    let Ok((entity, in_config)) = query.single() else {
        return;
    };
    let in_config = in_config.is_some();
    for (id, reply) in replies {
        crate::log_info!("resourcepacks", "server pack {id}: {reply:?}");
        if in_config {
            use azalea_protocol::packets::config::s_resource_pack::{
                Action, ServerboundResourcePack,
            };
            let action = match reply {
                Reply::Loaded => Action::SuccessfullyLoaded,
                Reply::Declined => Action::Declined,
                Reply::FailedDownload => Action::FailedDownload,
                Reply::Accepted => Action::Accepted,
                Reply::Downloaded => Action::Downloaded,
                Reply::InvalidUrl => Action::InvalidUrl,
                Reply::FailedReload => Action::FailedReload,
                Reply::Discarded => Action::Discarded,
            };
            commands.trigger(azalea::packet::config::SendConfigPacketEvent::new(
                entity,
                ServerboundResourcePack { id, action },
            ));
        } else {
            use azalea_protocol::packets::game::s_resource_pack::{
                Action, ServerboundResourcePack,
            };
            let action = match reply {
                Reply::Loaded => Action::SuccessfullyLoaded,
                Reply::Declined => Action::Declined,
                Reply::FailedDownload => Action::FailedDownload,
                Reply::Accepted => Action::Accepted,
                Reply::Downloaded => Action::Downloaded,
                Reply::InvalidUrl => Action::InvalidUrl,
                Reply::FailedReload => Action::FailedReload,
                Reply::Discarded => Action::Discarded,
            };
            commands.trigger(azalea::packet::game::SendGamePacketEvent::new(
                entity,
                ServerboundResourcePack { id, action },
            ));
        }
    }
    if leave {
        let reason =
            crate::gui::tooltip::translate("multiplayer.requiredTexturePrompt.disconnect", &[]);
        disconnect.write(azalea::disconnect::DisconnectEvent {
            entity,
            reason: Some(reason.into()),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(n: u128) -> Uuid {
        Uuid::from_u128(n)
    }

    fn with_policy(policy: PackPolicy) -> ServerPacks {
        let mut packs = ServerPacks::default();
        packs.begin(policy);
        packs
    }

    fn answer(packs: &mut ServerPacks, yes: bool) -> Vec<(Uuid, Reply)> {
        packs.answer = Some(yes);
        packs.take_answer();
        packs.cleanup();
        std::mem::take(&mut packs.replies)
    }

    #[test]
    fn only_http_and_https_are_urls() {
        assert!(http_url("https://example.com/pack.zip"));
        assert!(http_url("HTTP://example.com/pack.zip"));
        assert!(!http_url("ftp://example.com/pack.zip"));
        assert!(!http_url("file:///tmp/pack.zip"));
        assert!(!http_url("example.com/pack.zip"));
    }

    #[test]
    fn a_hash_that_is_not_sha1_is_no_hash() {
        assert_eq!(sha1_hash(&"AB".repeat(20)), Some("ab".repeat(20)));
        assert_eq!(sha1_hash("abc"), None);
        assert_eq!(sha1_hash(&"zz".repeat(20)), None);
    }

    #[test]
    fn a_bad_url_is_answered_without_asking() {
        let mut packs = ServerPacks::default();
        packs.push(id(1), "ftp://example.com/p.zip", "", true, None);
        assert!(packs.prompt.is_none());
        assert_eq!(packs.replies, [(id(1), Reply::InvalidUrl)]);
    }

    #[test]
    fn an_enabled_server_is_accepted_without_asking() {
        let mut packs = with_policy(PackPolicy::Enabled);
        packs.push(id(1), "https://example.com/a.zip", "", false, None);
        assert!(packs.prompt.is_none());
        assert_eq!(packs.replies, [(id(1), Reply::Accepted)]);
    }

    #[test]
    fn a_disabled_server_is_declined_unless_its_pack_is_required() {
        let mut packs = with_policy(PackPolicy::Disabled);
        packs.push(id(1), "https://example.com/a.zip", "", false, None);
        assert!(packs.prompt.is_none());
        assert_eq!(packs.replies, [(id(1), Reply::Declined)]);
        packs.push(id(2), "https://example.com/b.zip", "", true, None);
        assert!(packs.prompt.is_some(), "a required pack still asks");
    }

    #[test]
    fn declining_answers_declined_and_a_required_pack_disconnects() {
        let mut packs = with_policy(PackPolicy::Prompt);
        packs.push(id(1), "https://example.com/a.zip", "", true, None);
        packs.push(id(2), "https://example.com/b.zip", "", true, None);
        assert_eq!(
            answer(&mut packs, false),
            [(id(1), Reply::Declined), (id(2), Reply::Declined)]
        );
        assert!(packs.disconnect);
        assert!(packs.packs.is_empty());
    }

    #[test]
    fn accepting_answers_accepted_first() {
        let mut packs = with_policy(PackPolicy::Prompt);
        packs.push(id(1), "https://example.com/a.zip", "", false, None);
        assert_eq!(answer(&mut packs, true), [(id(1), Reply::Accepted)]);
        assert!(!packs.disconnect);
    }

    #[cfg(not(resource_packs))]
    #[test]
    fn with_nowhere_to_mount_an_accepted_pack_fails_to_reload() {
        let mut packs = with_policy(PackPolicy::Prompt);
        packs.push(id(1), "https://example.com/a.zip", "", false, None);
        assert_eq!(answer(&mut packs, true), [(id(1), Reply::Accepted)]);
        packs.tick();
        assert_eq!(packs.replies, [(id(1), Reply::FailedReload)]);
    }

    #[test]
    fn a_pop_before_the_answer_leaves_the_request() {
        let mut packs = with_policy(PackPolicy::Prompt);
        packs.push(id(1), "https://example.com/a.zip", "", false, None);
        packs.pop(None);
        assert_eq!(answer(&mut packs, true), [(id(1), Reply::Accepted)]);
    }
}
