pub use log::error;
pub use log::info;
pub use log::trace;
pub use log::warn;

use crate::arch;

static LOGGER: GlobalLogger = GlobalLogger::new();

struct GlobalLogger {}

impl GlobalLogger {
    const fn new() -> Self {
        Self {}
    }
}

impl log::Log for GlobalLogger {
    fn enabled(&self, _metadata: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        if matches!(record.level(), log::Level::Error | log::Level::Warn) {
            println!(
                "[{:10}] {:5} {}",
                arch::process_name(),
                record.level(),
                record.args()
            );
        } else {
            println!("[{:10}] {}", arch::process_name(), record.args());
        }
    }

    fn flush(&self) {}
}

pub fn init() {
    log::set_logger(&LOGGER).unwrap();
    log::set_max_level(log::LevelFilter::Trace);
}
