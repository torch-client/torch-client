#[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
fn main() -> eyre::Result<()> {
    torch_client::start_from_cli()
}

#[cfg(any(target_arch = "wasm32", target_os = "android"))]
fn main() {}
