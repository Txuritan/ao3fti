//! modified version of https://docs.rs/clap-verbosity-flag/1.0.0/clap_verbosity_flag/ for tracing

#[derive(clap::Args, Debug, Clone)]
pub struct Verbosity<L: LogLevel = ErrorLevel> {
    #[clap(
        long,
        short = 'v',
        parse(from_occurrences),
        global = true,
        help = L::verbose_help(),
        long_help = L::verbose_long_help(),
    )]
    verbose: i8,

    #[clap(
        long,
        short = 'q',
        parse(from_occurrences),
        global = true,
        help = L::quiet_help(),
        long_help = L::quiet_long_help(),
        conflicts_with = "verbose",
    )]
    quiet: i8,

    #[clap(skip)]
    phantom: std::marker::PhantomData<L>,
}

impl<L: LogLevel> Verbosity<L> {
    pub fn log_level_filter(&self) -> LevelFilter {
        level_enum(self.verbosity())
            .map(LevelFilter::from_level)
            .unwrap_or(LevelFilter::OFF)
    }

    fn verbosity(&self) -> i8 {
        level_value(L::default()) - self.quiet + self.verbose
    }
}

fn level_value(level: Option<Level>) -> i8 {
    match level {
        None => -1,
        Some(Level::ERROR) => 0,
        Some(Level::WARN) => 1,
        Some(Level::INFO) => 2,
        Some(Level::DEBUG) => 3,
        Some(Level::TRACE) => 4,
    }
}

fn level_enum(verbosity: i8) -> Option<Level> {
    match verbosity {
        std::i8::MIN..=-1 => None,
        0 => Some(Level::ERROR),
        1 => Some(Level::WARN),
        2 => Some(Level::INFO),
        3 => Some(Level::DEBUG),
        4..=std::i8::MAX => Some(Level::TRACE),
    }
}

use std::fmt;

use tracing::{level_filters::LevelFilter, Level};

impl<L: LogLevel> fmt::Display for Verbosity<L> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.verbosity())
    }
}

pub trait LogLevel {
    fn default() -> Option<Level>;

    fn verbose_help() -> Option<&'static str> {
        Some("More output per occurrence")
    }

    fn verbose_long_help() -> Option<&'static str> {
        None
    }

    fn quiet_help() -> Option<&'static str> {
        Some("Less output per occurrence")
    }

    fn quiet_long_help() -> Option<&'static str> {
        None
    }
}

#[derive(Copy, Clone, Debug, Default)]
pub struct ErrorLevel;

impl LogLevel for ErrorLevel {
    fn default() -> Option<Level> {
        Some(Level::ERROR)
    }
}

#[derive(Copy, Clone, Debug, Default)]
pub struct WarnLevel;

impl LogLevel for WarnLevel {
    fn default() -> Option<Level> {
        Some(Level::WARN)
    }
}

#[derive(Copy, Clone, Debug, Default)]
pub struct InfoLevel;

impl LogLevel for InfoLevel {
    fn default() -> Option<Level> {
        Some(Level::INFO)
    }
}
