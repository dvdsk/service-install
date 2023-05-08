#[derive(Debug, Clone)]
pub enum Schedule {
    Daily(time::Time),
}
