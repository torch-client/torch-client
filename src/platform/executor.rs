pub(crate) fn spawn<F>(future: F)
where
    F: Future<Output = ()> + 'static,
{
    #[cfg(target_arch = "wasm32")]
    {
        wasm_bindgen_futures::spawn_local(future);
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("net")
            .on_thread_start(|| crate::diag::alloc::label_thread(crate::diag::alloc::Site::Net))
            .enable_all()
            .build()
            .expect("tokio runtime");
        runtime.block_on(future);
    }
}

#[cfg(all(not(target_arch = "wasm32"), feature = "asset_download"))]
pub(crate) fn block_on<F: Future>(future: F) -> F::Output {
    use std::task::{Context, Poll};
    let mut future = std::pin::pin!(future);
    let mut cx = Context::from_waker(std::task::Waker::noop());
    loop {
        match future.as_mut().poll(&mut cx) {
            Poll::Ready(value) => return value,
            Poll::Pending => std::thread::yield_now(),
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn spawn_detached<F>(future: F)
where
    F: Future<Output = ()> + 'static,
{
    wasm_bindgen_futures::spawn_local(future);
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn spawn_detached<F>(future: F)
where
    F: Future<Output = ()> + Send + 'static,
{
    native_pool().spawn(future);
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn spawn_task<F>(future: F)
where
    F: Future<Output = ()> + 'static,
{
    wasm_bindgen_futures::spawn_local(future);
}

#[cfg(all(not(target_arch = "wasm32"), feature = "eagler"))]
pub(crate) fn spawn_task<F>(future: F)
where
    F: Future<Output = ()> + Send + 'static,
{
    tokio::spawn(future);
}

#[cfg(not(target_arch = "wasm32"))]
fn native_pool() -> &'static tokio::runtime::Runtime {
    static POOL: std::sync::OnceLock<tokio::runtime::Runtime> = std::sync::OnceLock::new();
    POOL.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .thread_name("mc-async")
            .on_thread_start(|| crate::diag::alloc::label_thread(crate::diag::alloc::Site::Net))
            .build()
            .expect("async runtime")
    })
}
