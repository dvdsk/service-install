#[derive(Debug, Clone)]
pub enum Schedule {
    /// Local time
    Daily(time::Time),
    /// Run once very this duration, 
    /// note the service runs with second accuracy
    Every(std::time::Duration),
}
