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
    uncompressed: u64,
    method: u16,
}

fn u16_at(b: &[u8], i: usize) -> u16 {
    u16::from_le_bytes([b[i], b[i + 1]])
}

fn u32_at(b: &[u8], i: usize) -> u32 {
    u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]])
}

const EOCD_FIXED: usize = 22;

fn at(jar: &[u8], from: usize, len: usize) -> Result<&[u8], String> {
    from.checked_add(len)
        .and_then(|end| jar.get(from..end))
        .ok_or_else(|| "the archive ends mid-record".to_owned())
}

pub fn entries(jar: &[u8]) -> Result<Vec<Entry>, String> {
    if jar.len() < EOCD_FIXED {
        return Err("the file is too short to be a zip archive".into());
    }
    let tail_len = (MAX_COMMENT as usize + EOCD_FIXED).min(jar.len());
    let tail = &jar[jar.len() - tail_len..];

    let eocd = (0..=tail.len() - EOCD_FIXED)
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
            uncompressed: u32_at(cd, pos + 24) as u64,
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
            let mut out = Vec::with_capacity(entry.uncompressed as usize);
            flate2::read::DeflateDecoder::new(raw)
                .take(entry.uncompressed + 1)
                .read_to_end(&mut out)
                .map_err(|e| e.to_string())?;
            if out.len() as u64 > entry.uncompressed {
                return Err(format!(
                    "{} inflates past the {} bytes its directory entry declares",
                    entry.name, entry.uncompressed
                ));
            }
            Ok(out)
        }
        m => Err(format!("compression method {m} is not supported")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_too_short_to_be_an_archive_is_an_error() {
        for bytes in [
            &[][..],
            &[0x50, 0x4b][..],
            &[0x50, 0x4b, 0x05, 0x06, 0, 0, 0, 0][..],
        ] {
            assert!(entries(bytes).is_err(), "{bytes:?}");
        }
    }

    #[test]
    fn an_empty_archive_has_no_entries() {
        let mut bytes = vec![0u8; EOCD_FIXED];
        bytes[..4].copy_from_slice(&EOCD_SIG.to_le_bytes());
        assert_eq!(entries(&bytes).map(|e| e.len()), Ok(0));
    }

    #[test]
    fn a_slice_past_the_end_or_overflowing_is_an_error() {
        assert!(at(&[1, 2, 3], 2, 2).is_err());
        assert!(at(&[1, 2, 3], usize::MAX, 2).is_err());
        assert_eq!(at(&[1, 2, 3], 1, 2), Ok(&[2, 3][..]));
    }
}
