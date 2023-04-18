use tracing::{Metadata, Level};
use tracing_subscriber::layer::{Context, Filter};

#[derive(Default, Debug)]
pub struct TraceFilter {}

impl TraceFilter {
    fn is_enabled(&self, metadata: &Metadata<'_>) -> bool {
        let roll = rand::random::<f32>();
        let level = metadata.level().as_str();
        if level == Level::TRACE.as_str() {
            roll < 0.00000005
        } else if level == Level::DEBUG.as_str() {
            roll < 0.000001
        } else {
            true
        }
    }
}

impl<S> Filter<S> for TraceFilter {
    fn enabled(&self, meta: &Metadata<'_>, _: &Context<'_, S>) -> bool {
        self.is_enabled(meta)
    }
}
