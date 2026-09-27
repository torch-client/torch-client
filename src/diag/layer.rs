use super::stats::{Stat, bump};

pub fn custom_layers(_app: &mut bevy::app::App) -> Option<bevy::log::BoxedLayer> {
    #[cfg_attr(not(feature = "profiling"), expect(unused_mut))]
    let mut layers: Vec<bevy::log::BoxedLayer> = vec![Box::new(DropCounter)];
    #[cfg(feature = "profiling")]
    layers.extend(crate::diag::profiling::bridge_layer());
    Some(Box::new(layers))
}

struct DropCounter;

impl<S> bevy::log::tracing_subscriber::Layer<S> for DropCounter
where
    S: bevy::log::tracing::Subscriber,
{
    fn on_event(
        &self,
        event: &bevy::log::tracing::Event<'_>,
        _ctx: bevy::log::tracing_subscriber::layer::Context<'_, S>,
    ) {
        if event.metadata().target() == "azalea_world::chunk::partial"
            && *event.metadata().level() == bevy::log::tracing::Level::WARN
        {
            let mut first_line = false;
            event.record(&mut FieldPeek {
                hit: &mut first_line,
            });
            if first_line {
                bump(Stat::ChunksDropped);
            }
        }
    }
}

struct FieldPeek<'a> {
    hit: &'a mut bool,
}

impl bevy::log::tracing::field::Visit for FieldPeek<'_> {
    fn record_debug(
        &mut self,
        field: &bevy::log::tracing::field::Field,
        value: &dyn std::fmt::Debug,
    ) {
        if field.name() == "message" && format!("{value:?}").contains("not in the render distance")
        {
            *self.hit = true;
        }
    }
}
