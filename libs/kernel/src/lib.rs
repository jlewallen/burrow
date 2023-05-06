pub mod english;
pub mod hooks;
pub mod model;
pub mod plugins;
pub mod scopes;
pub mod session;
pub mod surround;

pub use english::*;
pub use hooks::*;
pub use model::*;
pub use plugins::*;
pub use scopes::*;
pub use session::*;
pub use surround::*;

pub struct LogTimeFromNow {
    started: std::time::Instant,
    prefix: String,
}

impl LogTimeFromNow {
    pub fn new(prefix: &str) -> Self {
        Self {
            started: std::time::Instant::now(),
            prefix: prefix.to_owned(),
        }
    }
}

impl Drop for LogTimeFromNow {
    fn drop(&mut self) {
        let elapsed = std::time::Instant::now() - self.started;
        info!("{} {:?}", self.prefix, elapsed);
    }
}
