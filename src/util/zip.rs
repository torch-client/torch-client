use std::io::Read;

const EOCD_SIG: u32 = 0x0605_4b50;
const CENTRAL_SIG: u32 = 0x0201_4b50;
const LOCAL_SIG: u32 = 0x0403_4b50;

const CENTRAL_FIXED: usize = 46;
const LOCAL_FIXED: usize = 30;

const MAX_COMMENT: u64 = 0xFFFF;

pub struct Entry {
    pub name: String,
    offset: u64,
    compressed: u64,
    method: u16,
}

fn u16_at(b: &[u8], i: usize) -> u16 {
    u16::from_le_bytes([b[i], b[i + 1]])
}

fn u32_at(b: &[u8], i: usize) -> u32 {
    u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]])
}

fn at(jar: &[u8], from: usize, len: usize) -> Result<&[u8], String> {
    jar.get(from..from + len)
        .ok_or_else(|| "the archive ends mid-record".to_owned())
}

pub fn entries(jar: &[u8]) -> Result<Vec<Entry>, String> {
    let size = jar.len() as u64;
    let tail_len = (MAX_COMMENT + 22).min(size) as usize;
    let tail = &jar[jar.len() - tail_len..];

    let eocd = (0..=tail.len().saturating_sub(22))
        .rev()
        .find(|&i| u32_at(tail, i) == EOCD_SIG)
        .ok_or("no end of central directory record")?;
    let rec = &tail[eocd..];
    let count = u16_at(rec, 10);
    let cd_size = u32_at(rec, 12);
    let cd_offset = u32_at(rec, 16);
    if count == 0xFFFF || cd_size == 0xFFFF_FFFF || cd_offset == 0xFFFF_FFFF {
        return Err("zip64 archives are not supported".into());
    }

    let cd = at(jar, cd_offset as usize, cd_size as usize)?;

    let mut out = Vec::with_capacity(count as usize);
    let mut pos = 0usize;
    for _ in 0..count {
        if pos + CENTRAL_FIXED > cd.len() || u32_at(cd, pos) != CENTRAL_SIG {
            return Err("the central directory is malformed".into());
        }
        let name_len = u16_at(cd, pos + 28) as usize;
        let extra_len = u16_at(cd, pos + 30) as usize;
        let comment_len = u16_at(cd, pos + 32) as usize;
        let name_at = pos + CENTRAL_FIXED;
        let name = String::from_utf8_lossy(at(cd, name_at, name_len)?).into_owned();
        out.push(Entry {
            name,
            offset: u32_at(cd, pos + 42) as u64,
            compressed: u32_at(cd, pos + 20) as u64,
            method: u16_at(cd, pos + 10),
        });
        pos = name_at + name_len + extra_len + comment_len;
    }
    Ok(out)
}

pub fn read(jar: &[u8], entry: &Entry) -> Result<Vec<u8>, String> {
    let head_at = entry.offset as usize;
    let head = at(jar, head_at, LOCAL_FIXED)?;
    if u32_at(head, 0) != LOCAL_SIG {
        return Err("an entry has no local header".into());
    }
    let skip = u16_at(head, 26) as usize + u16_at(head, 28) as usize;
    let raw = at(jar, head_at + LOCAL_FIXED + skip, entry.compressed as usize)?;
    match entry.method {
        0 => Ok(raw.to_vec()),
        8 => {
            let mut out = Vec::new();
            flate2::read::DeflateDecoder::new(raw)
                .read_to_end(&mut out)
                .map_err(|e| e.to_string())?;
            Ok(out)
        }
        m => Err(format!("compression method {m} is not supported")),
    }
}
