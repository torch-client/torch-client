use std::sync::Arc;

use base64::Engine as _;
use parking_lot::Mutex;

use super::assets::Failure;
use crate::platform::storage;
use crate::util::hash::sha1_hex;
use crate::{log_info, log_warn};

const TAG: &str = "bedrock_art";

pub(crate) const REVISION: &str = "46ba6ea985fb5a92d79a9419198f10dda14c199d";

const BASE_URL: &str = "https://raw.githubusercontent.com/Mojang/bedrock-samples";

const MAX_BYTES: usize = 64 * 1024;

pub(crate) const FILES: &[(&str, &str)] = &[
    ("ui/jump", "c680defa5618a55cc9319008b6839bc310f4c048"),
    (
        "ui/jump_pressed",
        "80491cfcefca3d10a4c55716131f4819c920b960",
    ),
    ("ui/sneak", "10a83b2ab415da2f12dd156452563069e0d21f79"),
    (
        "ui/sneak_pressed",
        "7ccf29444d99a144382f038e3463f744633f732b",
    ),
    ("ui/sprint", "feac835b4cfefcae57cc7f40810bdda039a5b14c"),
    (
        "ui/sprint_pressed",
        "811d438b7e8c433bfff9bf3c9e20b21450d8dd21",
    ),
    ("ui/attack", "851e5e076366e6f328fc452d94269fcd39b14b76"),
    (
        "ui/attack_pressed",
        "6e0b72f47dc819ee3e34735d4f9f10519a0acd5d",
    ),
    ("ui/interact", "75d677e3346003c489d9aeb45e5b612cef157e95"),
    (
        "ui/interact_pressed",
        "c1253f5994d9b834c5c9c2cd8bb4d2242d4e4ffc",
    ),
    (
        "ui/joystick_frame",
        "79a291966875ffe9dbe726cef6cdb460b4cb6475",
    ),
    (
        "ui/joystick_knob",
        "d6a6e22c6317bf2a6212309a9bb3d5d2bfcb4dd1",
    ),
    (
        "gui/controls/up",
        "c9af5691a4dfb7c619b64357b3e532480ddc5d54",
    ),
    (
        "gui/controls/up_pressed",
        "ffccffb0b0b6600c29b9c09b9d19f6345d12e41c",
    ),
    (
        "gui/controls/down",
        "c505ec42adf8af62bbd5365013fd247c43e26a48",
    ),
    (
        "gui/controls/down_pressed",
        "8e0de1b0bb6da51cb8bb810c7f4809993fd0a6e1",
    ),
    (
        "gui/controls/left",
        "95f60e6921581522c8af579aad392e29a451d6cd",
    ),
    (
        "gui/controls/left_pressed",
        "43fbd103b15fec82929d20831f76635ab701fcc0",
    ),
    (
        "gui/controls/right",
        "7e483e8f73d9dcf00eea3055c28bd130d8ea1c9b",
    ),
    (
        "gui/controls/right_pressed",
        "db24e6ddd4415080bc53ddbfbb69547df47990f2",
    ),
    (
        "gui/controls/up_left",
        "db73c3b3d25b1708899bcd8a4f0446ac8cf61243",
    ),
    (
        "gui/controls/up_left_pressed",
        "33754ea218888ee04dd0bf3e6defd04050dc1e82",
    ),
    (
        "gui/controls/up_right",
        "051f8285d4a560c5b5125df3c2246a9e0a34de25",
    ),
    (
        "gui/controls/up_right_pressed",
        "1e9a226697bb0519aaba85b7ccf1236ad4c59c70",
    ),
    (
        "gui/controls/down_left",
        "e3f934b03e550739e8118ade7a9047146b8a2a98",
    ),
    (
        "gui/controls/down_left_pressed",
        "4e69ac5ecbd8e7bae0222daa90c86a4e2c3cb769",
    ),
    (
        "gui/controls/down_right",
        "97b6182ec934da1d06d93f3f88fd29fb8f6ade7e",
    ),
    (
        "gui/controls/down_right_pressed",
        "c91183321927cac2ed08b6ffa92e04de5b30db94",
    ),
    (
        "gui/controls/jump_dpad",
        "e47714fa62d35de4c97a5ac64d1363c9a888eb51",
    ),
    (
        "gui/controls/jump_dpad_pressed",
        "01491ffa2f5b44eda1d05a49ff32183514e7e89a",
    ),
    (
        "gui/controls/sneak_dpad",
        "1b2457438707bfa8a8e1a41aec10372eead0fc65",
    ),
    (
        "gui/controls/sneak_dpad_pressed",
        "43a47534be17b99090a1fa3070b2d346240db1e5",
    ),
    (
        "gui/controls/flyingascend",
        "eaaed7140f0757ca16e8ff6e801be771af89fb33",
    ),
    (
        "gui/controls/flyingascend_pressed",
        "2ae71ea2309423ad6d0ca049bdcd7fe6250425c3",
    ),
    (
        "gui/controls/flyingdescend",
        "b2b82e2f2c10be7e38cc84dbe15577f666fb12f8",
    ),
    (
        "gui/controls/flyingdescend_pressed",
        "69a60b16e8b5313132957ade97d1a22128232cfe",
    ),
];

const TOTAL: u16 = FILES.len() as u16;

const NO_ANSWER: &str = "GitHub did not answer";
const DAMAGED: &str = "A button texture arrived damaged";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Status {
    Idle,
    Fetching { done: u16, total: u16 },
    Failed(&'static str),
    Ready,
}

pub(crate) type Files = Vec<(&'static str, Vec<u8>)>;

static STATUS: Mutex<Status> = Mutex::new(Status::Idle);
static READY: Mutex<Option<Arc<Files>>> = Mutex::new(None);

pub(crate) fn status() -> Status {
    *STATUS.lock()
}

pub(crate) fn files() -> Option<Arc<Files>> {
    READY.lock().clone()
}

pub(crate) fn request() {
    {
        let mut status = STATUS.lock();
        if *status != Status::Idle {
            return;
        }
        *status = Status::Fetching {
            done: 0,
            total: TOTAL,
        };
    }
    if let Some(files) = stored() {
        log_info!(TAG, "{} button textures from storage", files.len());
        finish(files);
        return;
    }
    log_info!(
        TAG,
        "fetching {} button textures at {REVISION}",
        FILES.len()
    );
    super::language::spawn(TAG, || async {
        match fetch_all().await {
            Ok(files) => {
                storage::BEDROCK_TOUCH.store(&write_doc(&files));
                log_info!(TAG, "{} button textures fetched and stored", files.len());
                finish(files);
            }
            Err(e) => {
                log_warn!(TAG, "{}", e.detail);
                *STATUS.lock() = Status::Failed(e.short);
            }
        }
    });
}

pub(crate) fn retry() {
    {
        let mut status = STATUS.lock();
        if !matches!(*status, Status::Failed(_)) {
            return;
        }
        *status = Status::Idle;
    }
    request();
}

fn finish(files: Files) {
    *READY.lock() = Some(Arc::new(files));
    *STATUS.lock() = Status::Ready;
}

fn blob_sha1(bytes: &[u8]) -> String {
    let mut data = format!("blob {}\0", bytes.len()).into_bytes();
    data.extend_from_slice(bytes);
    sha1_hex(&data)
}

fn verify(name: &str, blob: &str, bytes: &[u8]) -> Result<(), String> {
    if bytes.len() > MAX_BYTES {
        return Err(format!(
            "{name}: {} bytes, more than {MAX_BYTES}",
            bytes.len()
        ));
    }
    let got = blob_sha1(bytes);
    if got != blob {
        return Err(format!("{name}: blob {got}, expected {blob}"));
    }
    Ok(())
}

async fn fetch_all() -> Result<Files, Failure> {
    let mut out = Vec::with_capacity(FILES.len());
    for (i, &(name, blob)) in FILES.iter().enumerate() {
        out.push((name, fetch_one(name, blob).await?));
        *STATUS.lock() = Status::Fetching {
            done: i as u16 + 1,
            total: TOTAL,
        };
    }
    Ok(out)
}

async fn fetch_one(name: &str, blob: &str) -> Result<Vec<u8>, Failure> {
    let url = format!("{BASE_URL}/{REVISION}/resource_pack/textures/{name}.png");
    let body = crate::platform::http::get_bytes(&url, 0, &mut |got, _| got <= MAX_BYTES as u64)
        .await
        .map_err(|e| {
            let mut failure = Failure::http(NO_ANSWER, e);
            failure.detail = format!("{url}: {}", failure.detail);
            failure
        })?;
    let bytes = match body {
        crate::platform::http::Fetched::Body(bytes) => bytes,
        crate::platform::http::Fetched::Cancelled => {
            return Err(Failure::new(
                DAMAGED,
                format!("{name}: more than {MAX_BYTES} bytes"),
            ));
        }
    };
    verify(name, blob, &bytes).map_err(|detail| Failure::new(DAMAGED, detail))?;
    Ok(bytes)
}

fn stored() -> Option<Files> {
    let doc = storage::BEDROCK_TOUCH.load()?;
    let Some(files) = read_doc(&doc) else {
        log_info!(
            TAG,
            "the stored button textures are incomplete or from another revision"
        );
        return None;
    };
    for ((name, bytes), &(_, blob)) in files.iter().zip(FILES) {
        if let Err(detail) = verify(name, blob, bytes) {
            log_warn!(TAG, "stored copy is damaged: {detail}");
            return None;
        }
    }
    Some(files)
}

fn write_doc(files: &[(&'static str, Vec<u8>)]) -> String {
    let engine = base64::engine::general_purpose::STANDARD;
    let mut out = String::from(REVISION);
    for (name, bytes) in files {
        out.push('\n');
        out.push_str(name);
        out.push(' ');
        out.push_str(&engine.encode(bytes));
    }
    out
}

fn read_doc(doc: &str) -> Option<Files> {
    let engine = base64::engine::general_purpose::STANDARD;
    let mut lines = doc.lines();
    if lines.next()?.trim() != REVISION {
        return None;
    }
    let mut found: Vec<Option<Vec<u8>>> = vec![None; FILES.len()];
    for line in lines.filter(|l| !l.trim().is_empty()) {
        let (name, data) = line.split_once(' ')?;
        let i = FILES.iter().position(|(n, _)| *n == name)?;
        found[i] = Some(engine.decode(data.trim()).ok()?);
    }
    FILES
        .iter()
        .zip(found)
        .map(|(&(name, _), bytes)| Some((name, bytes?)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_blob_is_named_as_git_names_it() {
        assert_eq!(
            blob_sha1(b"hello\n"),
            "ce013625030ba8dba906f756967f9e9ca394464a"
        );
    }

    #[test]
    fn the_manifest_names_36_files_by_full_hashes() {
        assert_eq!(FILES.len(), 36);
        let mut names: Vec<&str> = FILES.iter().map(|(n, _)| *n).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), FILES.len(), "a name is listed twice");
        for (name, blob) in FILES {
            assert_eq!(blob.len(), 40, "{name}");
            assert!(
                blob.bytes()
                    .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()),
                "{name}"
            );
        }
    }

    #[test]
    fn the_stored_document_round_trips() {
        let files: Files = FILES
            .iter()
            .enumerate()
            .map(|(i, &(name, _))| (name, vec![i as u8; i + 1]))
            .collect();
        let doc = write_doc(&files);
        assert_eq!(read_doc(&doc), Some(files.clone()));

        assert_eq!(read_doc(&doc.replacen(REVISION, "0000", 1)), None);
        let short: Vec<&str> = doc.lines().take(FILES.len()).collect();
        assert_eq!(read_doc(&short.join("\n")), None);
        assert_eq!(read_doc(&format!("{doc}\nui/jump !!!")), None);
        assert_eq!(read_doc(&format!("{doc}\nui/unknown AAAA")), None);
    }

    #[test]
    fn a_file_must_match_its_blob() {
        let blob = "ce013625030ba8dba906f756967f9e9ca394464a";
        assert!(verify("x", blob, b"hello\n").is_ok());
        assert!(verify("x", blob, b"hello").is_err());
        assert!(verify("x", blob, &vec![0; MAX_BYTES + 1]).is_err());
    }
}
