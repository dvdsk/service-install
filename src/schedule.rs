#[derive(Debug, Clone)]
pub enum Schedule {
    /// Local time
    Daily(time::Time),
}
