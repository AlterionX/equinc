struct Cfg {
    level: log::LevelFilter,
    bypass_stdio: bool,
}

impl Cfg {
    fn setup_logger(self) -> Result<(), fern::InitError> {
        let dispatch = fern::Dispatch::new()
            .format(|out, message, record| {
                let colors = fern::colors::ColoredLevelConfig::new()
                    .trace(fern::colors::Color::BrightBlack)
                    .debug(fern::colors::Color::White)
                    .info(fern::colors::Color::BrightWhite)
                    .warn(fern::colors::Color::Yellow)
                    .error(fern::colors::Color::Red);
                out.finish(format_args!(
                    "{}[{}][{}] {}",
                    chrono::Local::now().format("[%Y-%m-%d][%H:%M:%S]"),
                    record.target(),
                    colors.color(record.level()),
                    message
                ))
            })
            .level(self.level)
            .chain(fern::log_file("output.log")?);
        let dispatch = if self.bypass_stdio {
            dispatch
        } else {
            dispatch.chain(std::io::stdout())
        };
        dispatch.apply().map_err(Into::into)
    }
}

#[cfg(debug_assertions)]
pub fn setup() -> Result<(), fern::InitError> {
    Cfg {
        level: log::LevelFilter::Debug,
        bypass_stdio: false,
    }.setup_logger()
}

#[cfg(not(debug_assertions))]
pub fn setup() -> Result<(), fern::InitError> {
    Cfg {
        level: log::LevelFilter::Info,
        bypass_stdio: true,
    }.setup_logger()
}