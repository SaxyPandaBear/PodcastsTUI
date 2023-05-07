use tracing::{Metadata, Level};
use tracing_subscriber::layer::{Context, Filter};

#[derive(Default, Debug)]
pub struct TraceFilter {}

impl<S> Filter<S> for TraceFilter {
    fn enabled(&self, meta: &Metadata<'_>, _: &Context<'_, S>) -> bool {
        let roll = rand::random::<f32>();
        let level = *meta.level();
        if level == Level::TRACE {
            roll < 0.00000005
        } else if level == Level::DEBUG {
            roll < 0.000001
        } else {
            true
        }
    }
}
