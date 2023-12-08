use tracing::subscriber::set_global_default;
use tracing_subscriber::{layer::SubscriberExt, EnvFilter, Registry};

pub fn configure() {
    let Ok(env_filter) = EnvFilter::try_from_env("LOG") else {
        return;
    };

    let subscriber = Registry::default()
        .with(env_filter)
        .with(tracing_subscriber::fmt::layer());

    set_global_default(subscriber).expect("Setting subscriber");
}
