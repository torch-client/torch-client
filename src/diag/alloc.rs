pub const SITES: [&str; Site::COUNT] = [
    "render", "upload", "compute", "entities", "bot", "net", "mesh", "light", "pool", "other",
];

#[derive(Clone, Copy)]
pub enum Site {
    Render,
    Upload,
    Compute,
    Entities,
    Bot,
    Net,
    Mesh,
    Light,
    Pool,
    Other,
}

impl Site {
    pub const COUNT: usize = 10;
}

pub struct SiteGuard(#[cfg(feature = "alloc_diag")] usize);

#[cfg(feature = "alloc_diag")]
mod counting {
    use super::{Site, SiteGuard};
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;
    use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering::Relaxed};

    static LIVE: AtomicUsize = AtomicUsize::new(0);
    static COUNT: AtomicUsize = AtomicUsize::new(0);
    static TOTAL: AtomicU64 = AtomicU64::new(0);

    pub fn init_task_pools() {
        use bevy::app::{TaskPoolOptions, TaskPoolThreadAssignmentPolicy};

        let hook = |site: Site| -> Option<std::sync::Arc<dyn Fn() + Send + Sync + 'static>> {
            Some(std::sync::Arc::new(move || label_thread(site)))
        };
        let TaskPoolOptions {
            compute,
            io,
            async_compute,
            ..
        } = TaskPoolOptions::default();
        TaskPoolOptions {
            compute: TaskPoolThreadAssignmentPolicy {
                on_thread_spawn: hook(Site::Compute),
                ..compute
            },
            io: TaskPoolThreadAssignmentPolicy {
                on_thread_spawn: hook(Site::Pool),
                ..io
            },
            async_compute: TaskPoolThreadAssignmentPolicy {
                on_thread_spawn: hook(Site::Pool),
                ..async_compute
            },
            ..TaskPoolOptions::default()
        }
        .create_default_pools();
    }

    static CHURN: [AtomicU64; Site::COUNT] = [const { AtomicU64::new(0) }; Site::COUNT];

    thread_local! {
        static SITE: Cell<usize> = const { Cell::new(Site::Other as usize) };
    }

    pub fn label_thread(site: Site) {
        SITE.set(site as usize);
    }

    pub fn scope(site: Site) -> SiteGuard {
        SiteGuard(SITE.replace(site as usize))
    }

    impl Drop for SiteGuard {
        fn drop(&mut self) {
            SITE.set(self.0);
        }
    }

    pub struct Counting;

    unsafe impl GlobalAlloc for Counting {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            let p = unsafe { System.alloc(layout) };
            if !p.is_null() {
                note_alloc(layout.size());
            }
            p
        }

        unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
            let p = unsafe { System.alloc_zeroed(layout) };
            if !p.is_null() {
                note_alloc(layout.size());
            }
            p
        }

        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            LIVE.fetch_sub(layout.size(), Relaxed);
            COUNT.fetch_sub(1, Relaxed);
            unsafe { System.dealloc(ptr, layout) }
        }

        unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
            let p = unsafe { System.realloc(ptr, layout, new_size) };
            if !p.is_null() {
                if new_size >= layout.size() {
                    let grew = new_size - layout.size();
                    LIVE.fetch_add(grew, Relaxed);
                    note_churn(grew);
                } else {
                    LIVE.fetch_sub(layout.size() - new_size, Relaxed);
                }
            }
            p
        }
    }

    fn note_alloc(size: usize) {
        LIVE.fetch_add(size, Relaxed);
        COUNT.fetch_add(1, Relaxed);
        note_churn(size);
    }

    fn note_churn(size: usize) {
        TOTAL.fetch_add(size as u64, Relaxed);
        let site = SITE.try_with(Cell::get).unwrap_or(Site::Other as usize);
        CHURN[site].fetch_add(size as u64, Relaxed);
    }

    pub fn live() -> (u64, usize) {
        (LIVE.load(Relaxed) as u64, COUNT.load(Relaxed))
    }

    pub fn total_allocated() -> u64 {
        TOTAL.load(Relaxed)
    }

    pub fn churn_by_site() -> [u64; Site::COUNT] {
        std::array::from_fn(|i| CHURN[i].load(Relaxed))
    }

    #[cfg(all(unix, target_env = "gnu"))]
    unsafe extern "C" {
        fn malloc_trim(pad: usize) -> i32;
    }

    pub fn trim() {
        #[cfg(all(unix, target_env = "gnu"))]
        unsafe {
            malloc_trim(0);
        }
    }
}

#[cfg(feature = "alloc_diag")]
pub use counting::*;

#[cfg(not(feature = "alloc_diag"))]
mod stub {
    use super::{Site, SiteGuard};

    pub fn init_task_pools() {}

    pub fn label_thread(_site: Site) {}

    pub fn scope(_site: Site) -> SiteGuard {
        SiteGuard()
    }

    impl Drop for SiteGuard {
        fn drop(&mut self) {}
    }

    pub fn live() -> (u64, usize) {
        (0, 0)
    }

    pub fn total_allocated() -> u64 {
        0
    }

    pub fn churn_by_site() -> [u64; Site::COUNT] {
        [0; Site::COUNT]
    }

    pub fn trim() {}
}

#[cfg(not(feature = "alloc_diag"))]
pub use stub::*;
