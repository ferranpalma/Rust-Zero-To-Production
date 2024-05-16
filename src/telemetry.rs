use tracing::{subscriber, Subscriber};
use tracing_bunyan_formatter::{BunyanFormattingLayer, JsonStorageLayer};
use tracing_log::LogTracer;
use tracing_subscriber::{fmt::MakeWriter, layer::SubscriberExt, EnvFilter, Registry};

pub struct Telemetry {}

impl Telemetry {
    // See https://doc.rust-lang.org/nomicon/hrtb.html for details about sink
    pub fn create<Sink>(name: String, filter_level: String, sink: Sink)
    where
        Sink: for<'a> MakeWriter<'a> + Send + Sync + 'static,
    {
        let subscriber = Self::configure_subscriber(name, filter_level, sink);
        Self::initialise_subscriber(subscriber);
    }

    fn configure_subscriber<Sink>(
        name: String,
        filter_level: String,
        sink: Sink,
    ) -> impl Subscriber + Send + Sync
    where
        Sink: for<'a> MakeWriter<'a> + Send + Sync + 'static,
    {
        let env_filter =
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(filter_level));
        let formatting_layer = BunyanFormattingLayer::new(name, sink);

        Registry::default()
            .with(env_filter)
            .with(JsonStorageLayer)
            .with(formatting_layer)
    }

    fn initialise_subscriber(subscriber: impl Subscriber + Send + Sync) {
        LogTracer::init().expect("Failed to set LogTracer");
        subscriber::set_global_default(subscriber).expect("Failed to set subscriber");
    }
}
