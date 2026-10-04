use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::Arc;

pub(crate) fn dir() -> PathBuf {
    crate::client::assets::root().join("sounds")
}

fn pack_path(index_id: &str) -> PathBuf {
    dir().join(format!("{index_id}.bin"))
}

fn defs_path(index_id: &str) -> PathBuf {
    dir().join(format!("{index_id}.json"))
}

pub(crate) fn installed(index_id: &str) -> bool {
    pack_path(index_id).is_file() && defs_path(index_id).is_file()
}

pub(crate) fn save(index_id: &str, pack: &[u8], defs: &str) -> Result<(), String> {
    let write = |path: PathBuf, bytes: &[u8]| -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let part = path.with_extension("part");
        std::fs::write(&part, bytes).map_err(|e| e.to_string())?;
        std::fs::rename(&part, &path).map_err(|e| e.to_string())?;
        crate::log_info!("audio", "wrote {} ({} bytes)", path.display(), bytes.len());
        Ok(())
    };
    write(pack_path(index_id), pack)?;
    write(defs_path(index_id), defs.as_bytes())
}

pub(crate) fn load_defs(index_id: &str) -> Option<String> {
    std::fs::read_to_string(defs_path(index_id)).ok()
}

#[derive(Clone, Copy)]
struct Span {
    offset: u64,
    length: u32,
}

pub(crate) struct Pack {
    table: HashMap<String, Span>,
    file: File,
    cache: HashMap<String, Arc<[u8]>>,
}

const MAX_CACHED: usize = 512;

impl Pack {
    pub(crate) fn open(index_id: &str) -> Result<Pack, String> {
        let path = pack_path(index_id);
        let file = File::open(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut reader = std::io::BufReader::new(file);

        let (spans, consumed) = crate::platform::assets::read_table(&mut reader)
            .map_err(|e| format!("the sound pack: {e}"))?;

        let mut table = HashMap::with_capacity(spans.len());
        let mut offset = consumed;
        for (key, length) in spans {
            table.insert(key, Span { offset, length });
            offset += length as u64;
        }

        crate::log_info!("audio", "sound pack {index_id}: {} files", table.len());
        Ok(Pack {
            table,
            file: reader.into_inner(),
            cache: HashMap::new(),
        })
    }

    pub(crate) fn read(&mut self, key: &str) -> Option<Arc<[u8]>> {
        if let Some(hit) = self.cache.get(key) {
            return Some(hit.clone());
        }
        let span = *self.table.get(key)?;
        let bytes: Arc<[u8]> = {
            self.file.seek(SeekFrom::Start(span.offset)).ok()?;
            let mut buf = vec![0u8; span.length as usize];
            self.file.read_exact(&mut buf).ok()?;
            buf.into()
        };
        if self.cache.len() >= MAX_CACHED {
            self.cache.clear();
        }
        self.cache.insert(key.to_owned(), bytes.clone());
        Some(bytes)
    }

    pub(crate) fn len(&self) -> usize {
        self.table.len()
    }
}
