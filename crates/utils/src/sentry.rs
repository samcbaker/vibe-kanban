// Telemetry removed - this module is now a no-op stub

use tracing_subscriber::Layer;

#[derive(Clone, Copy, Debug)]
pub enum SentrySource {
    Backend,
    Mcp,
}

/// No-op: Sentry has been removed
pub fn init_once(_source: SentrySource) {
    // No-op - telemetry disabled
}

/// No-op: Sentry has been removed
pub fn configure_user_scope(_user_id: &str, _username: Option<&str>, _email: Option<&str>) {
    // No-op - telemetry disabled
}

/// Returns a no-op layer that does nothing
pub fn sentry_layer<S>() -> impl Layer<S>
where
    S: tracing::Subscriber,
    S: for<'a> tracing_subscriber::registry::LookupSpan<'a>,
{
    tracing_subscriber::layer::Identity::new()
}
