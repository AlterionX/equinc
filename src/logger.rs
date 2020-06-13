use chrono::Local;
use fern::{
    colors::{Color, ColoredLevelConfig},
    log_file, Dispatch, InitError,
};
use log::LevelFilter;

struct Cfg {
    level: LevelFilter,
    bypass_stdio: bool,
}

impl Cfg {
    fn setup_logger(self) -> Result<(), InitError> {
        let dispatch = Dispatch::new()
            .format(|out, message, record| {
                let colors = ColoredLevelConfig::new()
                    .trace(Color::BrightBlack)
                    .debug(Color::White)
                    .info(Color::BrightWhite)
                    .warn(Color::Yellow)
                    .error(Color::Red);
                out.finish(format_args!(
                    "{}[{}][{}] {}",
                    Local::now().format("[%Y-%m-%d][%H:%M:%S]"),
                    record.target(),
                    colors.color(record.level()),
                    message
                ))
            })
            .level(self.level)
            .chain(log_file("output.log")?);
        let dispatch = if self.bypass_stdio {
            dispatch
        } else {
            dispatch.chain(std::io::stdout())
        };
        dispatch.apply().map_err(Into::into)
    }
}

#[cfg(debug_assertions)]
pub fn setup() -> Result<(), InitError> {
    Cfg {
        level: LevelFilter::Debug,
        bypass_stdio: false,
    }
    .setup_logger()
}

#[cfg(not(debug_assertions))]
pub fn setup() -> Result<(), InitError> {
    Cfg {
        level: LevelFilter::Info,
        bypass_stdio: true,
    }
    .setup_logger()
}
