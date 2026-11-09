use std::time::SystemTime;

/// Outbound port for reading the current time.
pub trait Clock: Clone + Send + Sync + 'static {
    fn now(&self) -> SystemTime;
}
