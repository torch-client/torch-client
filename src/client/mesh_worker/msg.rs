use crate::lighting::level::{LightMap, SectionLight};
use crate::lighting::{ColumnPos, SectionPos, data_layer::DataLayer, data_layer::SIZE as LAYER};
use crate::renderer::{MeshBuf, PendingSection, visgraph::VisibilitySet};

pub struct Writer(Vec<u8>);

impl Writer {
    fn new(tag: u8) -> Self {
        Self(vec![tag])
    }

    fn u8(&mut self, v: u8) {
        self.0.push(v);
    }

    fn bool(&mut self, v: bool) {
        self.0.push(v as u8);
    }

    fn i32(&mut self, v: i32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }

    fn u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }

    fn u64(&mut self, v: u64) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }

    fn i16(&mut self, v: i16) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }

    fn bytes(&mut self, v: &[u8]) {
        self.u32(v.len() as u32);
        self.0.extend_from_slice(v);
    }

    fn finish(self) -> Vec<u8> {
        self.0
    }
}

pub struct Reader<'a> {
    buf: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let end = self.at.checked_add(n)?;
        let slice = self.buf.get(self.at..end)?;
        self.at = end;
        Some(slice)
    }

    fn u8(&mut self) -> Option<u8> {
        Some(self.take(1)?[0])
    }

    fn bool(&mut self) -> Option<bool> {
        Some(self.u8()? != 0)
    }

    fn i32(&mut self) -> Option<i32> {
        Some(i32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }

    fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }

    fn u64(&mut self) -> Option<u64> {
        Some(u64::from_le_bytes(self.take(8)?.try_into().ok()?))
    }

    fn i16(&mut self) -> Option<i16> {
        Some(i16::from_le_bytes(self.take(2)?.try_into().ok()?))
    }

    fn bytes(&mut self) -> Option<&'a [u8]> {
        let n = self.u32()? as usize;
        self.take(n)
    }

    fn count(&mut self) -> Option<usize> {
        let n = self.u32()? as usize;
        (n <= self.buf.len() - self.at).then_some(n)
    }
}

const TO_OPTIONS: u8 = 1;
const TO_LEVEL: u8 = 2;
const TO_BIOMES: u8 = 3;
const TO_CHUNK: u8 = 4;
const TO_LIGHT_UPDATE: u8 = 5;
const TO_BLOCK_CHANGE: u8 = 6;
const TO_UNLOAD: u8 = 7;
const TO_BACKLOG: u8 = 8;
const TO_RESET_LIGHT: u8 = 9;
const TO_RESET_LEVEL: u8 = 10;

pub enum ToWorker {
    Options {
        lighting: bool,
        smooth: bool,
    },
    Level {
        dim: u8,
        min_y: i32,
        height: u32,
    },
    BiomeTable(Vec<u8>),
    Chunk {
        cx: i32,
        cz: i32,
        mesh: Vec<(i32, i32)>,
        packet: Vec<u8>,
    },
    LightUpdate {
        cx: i32,
        cz: i32,
        data: Vec<u8>,
    },
    BlockChange {
        x: i32,
        y: i32,
        z: i32,
        state: u32,
    },
    Unload {
        cx: i32,
        cz: i32,
    },
    Backlog(u32),
    ResetLight,
    ResetLevel,
}

impl ToWorker {
    pub fn encode(&self) -> Vec<u8> {
        match self {
            ToWorker::Options { lighting, smooth } => {
                let mut w = Writer::new(TO_OPTIONS);
                w.bool(*lighting);
                w.bool(*smooth);
                w.finish()
            }
            ToWorker::Level { dim, min_y, height } => {
                let mut w = Writer::new(TO_LEVEL);
                w.u8(*dim);
                w.i32(*min_y);
                w.u32(*height);
                w.finish()
            }
            ToWorker::BiomeTable(table) => {
                let mut w = Writer::new(TO_BIOMES);
                w.bytes(table);
                w.finish()
            }
            ToWorker::Chunk {
                cx,
                cz,
                mesh,
                packet,
            } => {
                let mut w = Writer::new(TO_CHUNK);
                w.i32(*cx);
                w.i32(*cz);
                w.u32(mesh.len() as u32);
                for (x, z) in mesh {
                    w.i32(*x);
                    w.i32(*z);
                }
                w.bytes(packet);
                w.finish()
            }
            ToWorker::LightUpdate { cx, cz, data } => {
                let mut w = Writer::new(TO_LIGHT_UPDATE);
                w.i32(*cx);
                w.i32(*cz);
                w.bytes(data);
                w.finish()
            }
            ToWorker::BlockChange { x, y, z, state } => {
                let mut w = Writer::new(TO_BLOCK_CHANGE);
                w.i32(*x);
                w.i32(*y);
                w.i32(*z);
                w.u32(*state);
                w.finish()
            }
            ToWorker::Unload { cx, cz } => {
                let mut w = Writer::new(TO_UNLOAD);
                w.i32(*cx);
                w.i32(*cz);
                w.finish()
            }
            ToWorker::Backlog(n) => {
                let mut w = Writer::new(TO_BACKLOG);
                w.u32(*n);
                w.finish()
            }
            ToWorker::ResetLight => Writer::new(TO_RESET_LIGHT).finish(),
            ToWorker::ResetLevel => Writer::new(TO_RESET_LEVEL).finish(),
        }
    }

    pub fn decode(buf: &[u8]) -> Option<Self> {
        let mut r = Reader { buf, at: 1 };
        Some(match *buf.first()? {
            TO_OPTIONS => ToWorker::Options {
                lighting: r.bool()?,
                smooth: r.bool()?,
            },
            TO_LEVEL => ToWorker::Level {
                dim: r.u8()?,
                min_y: r.i32()?,
                height: r.u32()?,
            },
            TO_BIOMES => ToWorker::BiomeTable(r.bytes()?.to_vec()),
            TO_CHUNK => {
                let (cx, cz) = (r.i32()?, r.i32()?);
                let n = r.count()?;
                let mut mesh = Vec::with_capacity(n);
                for _ in 0..n {
                    mesh.push((r.i32()?, r.i32()?));
                }
                ToWorker::Chunk {
                    cx,
                    cz,
                    mesh,
                    packet: r.bytes()?.to_vec(),
                }
            }
            TO_LIGHT_UPDATE => ToWorker::LightUpdate {
                cx: r.i32()?,
                cz: r.i32()?,
                data: r.bytes()?.to_vec(),
            },
            TO_BLOCK_CHANGE => ToWorker::BlockChange {
                x: r.i32()?,
                y: r.i32()?,
                z: r.i32()?,
                state: r.u32()?,
            },
            TO_UNLOAD => ToWorker::Unload {
                cx: r.i32()?,
                cz: r.i32()?,
            },
            TO_BACKLOG => ToWorker::Backlog(r.u32()?),
            TO_RESET_LIGHT => ToWorker::ResetLight,
            TO_RESET_LEVEL => ToWorker::ResetLevel,
            _ => return None,
        })
    }
}

const FROM_READY: u8 = 1;
const FROM_FAILED: u8 = 2;
const FROM_SECTIONS: u8 = 3;
const FROM_LIGHT: u8 = 4;

pub struct LightDelta {
    pub lowest: i32,
    pub sections: Vec<(SectionPos, Option<SectionLight>)>,
    pub columns: Vec<(ColumnPos, bool, Option<i32>)>,
    pub removed: Vec<ColumnPos>,
}

pub enum FromWorker {
    Ready,
    Failed(String),
    Sections {
        chunks: Vec<PendingSection>,
        edits: Vec<PendingSection>,
    },
    Light(LightDelta),
}

pub fn ready() -> Vec<u8> {
    Writer::new(FROM_READY).finish()
}

pub fn failed(message: &str) -> Vec<u8> {
    let mut w = Writer::new(FROM_FAILED);
    w.bytes(message.as_bytes());
    w.finish()
}

pub fn sections(chunks: &[PendingSection], edits: &[PendingSection]) -> Vec<u8> {
    let mut w = Writer::new(FROM_SECTIONS);
    for list in [chunks, edits] {
        w.u32(list.len() as u32);
        for sec in list {
            w.i32(sec.chunk_x);
            w.i32(sec.chunk_z);
            w.i32(sec.sec_y);
            w.u64(sec.vis.bits());
            mesh_buf(&mut w, &sec.opaque);
            mesh_buf(&mut w, &sec.water);
        }
    }
    w.finish()
}

fn mesh_buf(w: &mut Writer, buf: &MeshBuf) {
    for v in buf.min {
        w.i16(v);
    }
    for v in buf.max {
        w.i16(v);
    }
    w.bytes(bytemuck::cast_slice(&buf.verts));
    w.bytes(bytemuck::cast_slice(&buf.idx));
    w.bytes(bytemuck::cast_slice(&buf.cutout_idx));
}

fn read_mesh_buf(r: &mut Reader) -> Option<MeshBuf> {
    let mut min = [0i16; 3];
    for v in &mut min {
        *v = r.i16()?;
    }
    let mut max = [0i16; 3];
    for v in &mut max {
        *v = r.i16()?;
    }
    let verts = read_pod(r.bytes()?)?;
    let idx = read_pod(r.bytes()?)?;
    let cutout_idx = read_pod(r.bytes()?)?;
    Some(MeshBuf {
        verts,
        idx,
        cutout_idx,
        min,
        max,
        #[cfg(feature = "shader_support")]
        pack: Vec::new(),
        #[cfg(feature = "shader_support")]
        pack_block: None,
    })
}

fn read_pod<T: bytemuck::Pod>(bytes: &[u8]) -> Option<Vec<T>> {
    let width = size_of::<T>();
    if bytes.len() % width != 0 {
        return None;
    }
    let mut out = vec![T::zeroed(); bytes.len() / width];
    bytemuck::cast_slice_mut::<T, u8>(&mut out).copy_from_slice(bytes);
    Some(out)
}

pub fn light(
    map: &LightMap,
    sections: &[SectionPos],
    columns: &[ColumnPos],
    removed: &[ColumnPos],
) -> Vec<u8> {
    let mut w = Writer::new(FROM_LIGHT);
    w.i32(map.lowest_section());
    w.u32(sections.len() as u32);
    for sec in sections {
        w.i32(sec.x);
        w.i32(sec.y);
        w.i32(sec.z);
        match map.section_layers(*sec) {
            Some(layers) => {
                w.bool(true);
                layer(&mut w, &layers.block);
                layer(&mut w, &layers.sky);
            }
            None => w.bool(false),
        }
    }
    w.u32(columns.len() as u32);
    for col in columns {
        let (lit, top) = map.column_state(*col);
        w.i32(col.x);
        w.i32(col.z);
        w.bool(lit);
        match top {
            Some(top) => {
                w.bool(true);
                w.i32(top);
            }
            None => w.bool(false),
        }
    }
    w.u32(removed.len() as u32);
    for col in removed {
        w.i32(col.x);
        w.i32(col.z);
    }
    w.finish()
}

fn layer(w: &mut Writer, l: &DataLayer) {
    match l.as_bytes() {
        Some(bytes) => {
            w.bool(true);
            w.0.extend_from_slice(bytes);
        }
        None => {
            w.bool(false);
            w.u8(l.fill_value());
        }
    }
}

fn read_layer(r: &mut Reader) -> Option<DataLayer> {
    if r.bool()? {
        let bytes: &[u8; LAYER] = r.take(LAYER)?.try_into().ok()?;
        Some(DataLayer::from_bytes(bytes))
    } else {
        Some(DataLayer::filled(r.u8()?))
    }
}

impl FromWorker {
    pub fn decode(buf: &[u8]) -> Option<Self> {
        let mut r = Reader { buf, at: 1 };
        Some(match *buf.first()? {
            FROM_READY => FromWorker::Ready,
            FROM_FAILED => FromWorker::Failed(String::from_utf8_lossy(r.bytes()?).into_owned()),
            FROM_SECTIONS => {
                let mut lists: [Vec<PendingSection>; 2] = [Vec::new(), Vec::new()];
                for list in &mut lists {
                    let n = r.count()?;
                    list.reserve(n);
                    for _ in 0..n {
                        list.push(PendingSection {
                            chunk_x: r.i32()?,
                            chunk_z: r.i32()?,
                            sec_y: r.i32()?,
                            vis: VisibilitySet::from_bits(r.u64()?),
                            opaque: read_mesh_buf(&mut r)?,
                            water: read_mesh_buf(&mut r)?,
                        });
                    }
                }
                let [chunks, edits] = lists;
                FromWorker::Sections { chunks, edits }
            }
            FROM_LIGHT => {
                let lowest = r.i32()?;
                let n = r.count()?;
                let mut sections = Vec::with_capacity(n);
                for _ in 0..n {
                    let sec = SectionPos {
                        x: r.i32()?,
                        y: r.i32()?,
                        z: r.i32()?,
                    };
                    let layers = if r.bool()? {
                        Some(SectionLight {
                            block: read_layer(&mut r)?,
                            sky: read_layer(&mut r)?,
                        })
                    } else {
                        None
                    };
                    sections.push((sec, layers));
                }
                let n = r.count()?;
                let mut columns = Vec::with_capacity(n);
                for _ in 0..n {
                    let col = ColumnPos {
                        x: r.i32()?,
                        z: r.i32()?,
                    };
                    let lit = r.bool()?;
                    let top = if r.bool()? { Some(r.i32()?) } else { None };
                    columns.push((col, lit, top));
                }
                let n = r.count()?;
                let mut removed = Vec::with_capacity(n);
                for _ in 0..n {
                    removed.push(ColumnPos {
                        x: r.i32()?,
                        z: r.i32()?,
                    });
                }
                FromWorker::Light(LightDelta {
                    lowest,
                    sections,
                    columns,
                    removed,
                })
            }
            _ => return None,
        })
    }
}
