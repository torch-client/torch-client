pub(crate) fn open(folder: &str) -> Result<(), String> {
    let _ = folder;
    Err(crate::lang::tr!("singleplayer.no_backend").to_owned())
}
