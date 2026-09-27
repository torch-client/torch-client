use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

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
    file: Mutex<File>,
    cache: Mutex<HashMap<String, Arc<[u8]>>>,
}

const MAX_CACHED: usize = 512;

impl Pack {
    pub(crate) fn open(index_id: &str) -> Result<Pack, String> {
        let path = pack_path(index_id);
        let file = File::open(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut reader = std::io::BufReader::new(file);

        let mut header = [0u8; 12];
        reader
            .read_exact(&mut header)
            .map_err(|_| "the sound pack ends mid-header".to_owned())?;
        if &header[..8] != b"MCASSET2" {
            return Err("the sound pack is not MCASSET2".to_owned());
        }
        let count = u32::from_le_bytes([header[8], header[9], header[10], header[11]]) as usize;

        let mut consumed = 12u64;
        let mut spans: Vec<(String, u32)> = Vec::with_capacity(count.min(1 << 16));
        let mut u32_at = |reader: &mut std::io::BufReader<File>| -> Result<u32, String> {
            let mut n = [0u8; 4];
            reader
                .read_exact(&mut n)
                .map_err(|_| "the sound pack ends mid-table".to_owned())?;
            Ok(u32::from_le_bytes(n))
        };
        for _ in 0..count {
            let key_len = u32_at(&mut reader)? as usize;
            let mut key = vec![0u8; key_len];
            reader
                .read_exact(&mut key)
                .map_err(|_| "the sound pack ends mid-key".to_owned())?;
            let key = String::from_utf8(key).map_err(|_| "a sound pack key is not UTF-8")?;
            let blob_len = u32_at(&mut reader)?;
            consumed += 8 + key_len as u64;
            spans.push((key, blob_len));
        }

        let mut table = HashMap::with_capacity(spans.len());
        let mut offset = consumed;
        for (key, length) in spans {
            table.insert(key, Span { offset, length });
            offset += length as u64;
        }

        crate::log_info!("audio", "sound pack {index_id}: {} files", table.len());
        Ok(Pack {
            table,
            file: Mutex::new(reader.into_inner()),
            cache: Mutex::new(HashMap::new()),
        })
    }

    pub(crate) fn read(&self, key: &str) -> Option<Arc<[u8]>> {
        if let Some(hit) = self.cache.lock().ok()?.get(key) {
            return Some(hit.clone());
        }
        let span = *self.table.get(key)?;
        let bytes: Arc<[u8]> = {
            let mut file = self.file.lock().ok()?;
            file.seek(SeekFrom::Start(span.offset)).ok()?;
            let mut buf = vec![0u8; span.length as usize];
            file.read_exact(&mut buf).ok()?;
            buf.into()
        };
        if let Ok(mut cache) = self.cache.lock() {
            if cache.len() >= MAX_CACHED {
                cache.clear();
            }
            cache.insert(key.to_owned(), bytes.clone());
        }
        Some(bytes)
    }

    pub(crate) fn len(&self) -> usize {
        self.table.len()
    }
}
