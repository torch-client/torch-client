use crate::platform::time::Instant;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

pub const MEMORY_METRICS: bool = cfg!(any(
    target_os = "linux",
    target_os = "android",
    target_arch = "wasm32"
));

pub const CPU_METRICS: bool = cfg!(any(target_os = "linux", target_os = "android"));

#[derive(Clone, Copy, Default)]
pub struct Runtime {
    pub rss_bytes: u64,
    pub rss_anon_bytes: u64,
    pub heap_live_bytes: u64,
    pub heap_live_allocs: usize,
    pub heap_alloc_per_sec: f64,
    pub heap_churn_by_site: [f64; super::alloc::Site::COUNT],
    pub cpu_percent: f32,
    pub cores: usize,
    pub rx_per_sec: f64,
    pub tx_per_sec: f64,
    pub rx_total: u64,
    pub tx_total: u64,
}

struct RuntimeSampler {
    at: Instant,
    cpu_ticks: u64,
    rx: u64,
    tx: u64,
    allocated: u64,
    churn: [u64; super::alloc::Site::COUNT],
    last: Runtime,
}

static SAMPLER: Mutex<Option<RuntimeSampler>> = Mutex::new(None);

const SAMPLE_WINDOW: Duration = Duration::from_millis(500);

pub fn runtime() -> Runtime {
    let now = Instant::now();
    let rx = azalea_protocol::traffic::rx();
    let tx = azalea_protocol::traffic::tx();
    let mut guard = SAMPLER.lock().unwrap();

    let (heap_live_bytes, heap_live_allocs) = super::alloc::live();
    let allocated = super::alloc::total_allocated();
    let churn = super::alloc::churn_by_site();

    let Some(prev) = guard.as_mut() else {
        let sample = RuntimeSampler {
            at: now,
            cpu_ticks: cpu_ticks(),
            rx,
            tx,
            allocated,
            churn,
            last: Runtime {
                rss_bytes: rss_bytes(),
                rss_anon_bytes: rss_anon_bytes(),
                heap_live_bytes,
                heap_live_allocs,
                cores: core_count(),
                rx_total: rx,
                tx_total: tx,
                ..Runtime::default()
            },
        };
        let out = sample.last;
        *guard = Some(sample);
        return out;
    };

    let elapsed = now.duration_since(prev.at);
    if elapsed < SAMPLE_WINDOW {
        prev.last.rx_total = rx;
        prev.last.tx_total = tx;
        return prev.last;
    }

    let secs = elapsed.as_secs_f64();
    let ticks = cpu_ticks();
    let cpu_secs = ticks.saturating_sub(prev.cpu_ticks) as f64 / 100.0;

    prev.last = Runtime {
        rss_bytes: rss_bytes(),
        rss_anon_bytes: rss_anon_bytes(),
        heap_live_bytes,
        heap_live_allocs,
        heap_alloc_per_sec: allocated.saturating_sub(prev.allocated) as f64 / secs,
        heap_churn_by_site: std::array::from_fn(|i| {
            churn[i].saturating_sub(prev.churn[i]) as f64 / secs
        }),
        cpu_percent: (cpu_secs / secs * 100.0) as f32,
        cores: core_count(),
        rx_per_sec: rx.saturating_sub(prev.rx) as f64 / secs,
        tx_per_sec: tx.saturating_sub(prev.tx) as f64 / secs,
        rx_total: rx,
        tx_total: tx,
    };
    prev.at = now;
    prev.cpu_ticks = ticks;
    prev.rx = rx;
    prev.tx = tx;
    prev.allocated = allocated;
    prev.churn = churn;
    prev.last
}

#[cfg(target_arch = "wasm32")]
fn wasm_linear_memory_bytes() -> u64 {
    #[wasm_bindgen::prelude::wasm_bindgen(inline_js = "
        export function mc_memory_byte_length(memory) {
            return memory.buffer.byteLength;
        }
    ")]
    unsafe extern "C" {
        #[wasm_bindgen(catch)]
        fn mc_memory_byte_length(
            memory: &wasm_bindgen::JsValue,
        ) -> Result<f64, wasm_bindgen::JsValue>;
    }

    mc_memory_byte_length(&wasm_bindgen::memory()).unwrap_or(0.0) as u64
}

fn rss_bytes() -> u64 {
    #[cfg(target_arch = "wasm32")]
    return wasm_linear_memory_bytes();

    #[cfg(not(target_arch = "wasm32"))]
    {
        if !MEMORY_METRICS {
            return 0;
        }
        let Ok(status) = std::fs::read_to_string("/proc/self/status") else {
            return 0;
        };
        for line in status.lines() {
            if let Some(rest) = line.strip_prefix("VmRSS:") {
                let kb: u64 = rest
                    .split_whitespace()
                    .next()
                    .and_then(|n| n.parse().ok())
                    .unwrap_or(0);
                return kb * 1024;
            }
        }
        0
    }
}

fn rss_anon_bytes() -> u64 {
    #[cfg(target_arch = "wasm32")]
    return wasm_linear_memory_bytes();

    #[cfg(not(target_arch = "wasm32"))]
    {
        if !MEMORY_METRICS {
            return 0;
        }
        let Ok(rollup) = std::fs::read_to_string("/proc/self/smaps_rollup") else {
            return 0;
        };
        for line in rollup.lines() {
            if let Some(rest) = line.strip_prefix("Anonymous:") {
                let kb: u64 = rest
                    .split_whitespace()
                    .next()
                    .and_then(|n| n.parse().ok())
                    .unwrap_or(0);
                return kb * 1024;
            }
        }
        0
    }
}

fn cpu_ticks() -> u64 {
    #[cfg(target_arch = "wasm32")]
    return 0;

    #[cfg(not(target_arch = "wasm32"))]
    {
        if !CPU_METRICS {
            return 0;
        }
        let Ok(stat) = std::fs::read_to_string("/proc/self/stat") else {
            return 0;
        };
        let Some(after) = stat.rsplit_once(')').map(|(_, rest)| rest) else {
            return 0;
        };
        let mut fields = after.split_whitespace().skip(11);
        let utime: u64 = fields.next().and_then(|n| n.parse().ok()).unwrap_or(0);
        let stime: u64 = fields.next().and_then(|n| n.parse().ok()).unwrap_or(0);
        utime + stime
    }
}

#[cfg_attr(not(feature = "profiling"), allow(dead_code))]
pub struct ThreadCpu {
    pub name: String,
    pub percent: f32,
}

#[cfg_attr(not(feature = "profiling"), allow(dead_code))]
pub fn thread_cpu() -> Vec<ThreadCpu> {
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    return Vec::new();

    #[cfg(any(target_os = "linux", target_os = "android"))]
    return proc_thread_cpu();
}

#[cfg_attr(not(feature = "profiling"), allow(dead_code))]
#[cfg(any(target_os = "linux", target_os = "android"))]
fn proc_thread_cpu() -> Vec<ThreadCpu> {
    use std::collections::HashMap;

    static PREV: Mutex<Option<(Instant, HashMap<u32, (String, u64)>)>> = Mutex::new(None);

    let now = Instant::now();
    let mut sample: HashMap<u32, (String, u64)> = HashMap::new();
    let tasks = match std::fs::read_dir("/proc/self/task") {
        Ok(tasks) => tasks,
        Err(_) => return Vec::new(),
    };
    for entry in tasks.flatten() {
        let Ok(tid) = entry.file_name().to_string_lossy().parse::<u32>() else {
            continue;
        };
        let Ok(stat) = std::fs::read_to_string(entry.path().join("stat")) else {
            continue;
        };
        let (before, after) = match (stat.find('('), stat.rfind(')')) {
            (Some(a), Some(b)) if b > a => (&stat[a + 1..b], &stat[b + 1..]),
            _ => continue,
        };
        let mut fields = after.split_whitespace().skip(11);
        let utime: u64 = fields.next().and_then(|n| n.parse().ok()).unwrap_or(0);
        let stime: u64 = fields.next().and_then(|n| n.parse().ok()).unwrap_or(0);
        sample.insert(tid, (before.to_owned(), utime + stime));
    }

    let mut guard = PREV.lock().unwrap();
    let out = match guard.as_ref() {
        Some((at, prev)) => {
            let secs = now.duration_since(*at).as_secs_f64();
            let hz = clock_hz() as f64;
            let mut out: Vec<ThreadCpu> = Vec::new();
            if secs > 0.0 {
                for (tid, (name, ticks)) in &sample {
                    let before = prev.get(tid).map(|(_, t)| *t).unwrap_or(0);
                    let delta = ticks.saturating_sub(before) as f64;
                    let percent = (delta / hz / secs * 100.0) as f32;
                    if percent >= 0.5 {
                        out.push(ThreadCpu {
                            name: name.clone(),
                            percent,
                        });
                    }
                }
            }
            out.sort_by(|a, b| b.percent.total_cmp(&a.percent));
            out
        }
        None => Vec::new(),
    };
    *guard = Some((now, sample));
    out
}

#[cfg_attr(not(feature = "profiling"), allow(dead_code))]
#[cfg(any(target_os = "linux", target_os = "android"))]
fn clock_hz() -> u64 {
    static HZ: OnceLock<u64> = OnceLock::new();
    *HZ.get_or_init(|| {
        let hz = unsafe { libc_sysconf(2) };
        if hz > 0 { hz as u64 } else { 100 }
    })
}

#[cfg_attr(not(feature = "profiling"), allow(dead_code))]
#[cfg(any(target_os = "linux", target_os = "android"))]
unsafe extern "C" {
    #[link_name = "sysconf"]
    fn libc_sysconf(name: i32) -> i64;
}

fn core_count() -> usize {
    static CORES: OnceLock<usize> = OnceLock::new();
    *CORES.get_or_init(|| {
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1)
    })
}

pub(super) fn reset_sampler() {
    *SAMPLER.lock().unwrap() = None;
}
