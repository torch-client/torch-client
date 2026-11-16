pub fn custom_layers(_app: &mut bevy::app::App) -> Option<bevy::log::BoxedLayer> {
    #[cfg(feature = "profiling")]
    return crate::diag::profiling::bridge_layer();
    #[cfg(not(feature = "profiling"))]
    None
}
