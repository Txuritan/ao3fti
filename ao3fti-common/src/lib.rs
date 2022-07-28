pub mod models;
pub mod timer;
pub mod utils;

pub use color_eyre::{
    eyre::{bail, eyre as err, Context, Report},
    install,
};
pub use crossbeam_channel as channel;
pub use http::Uri;

#[twelf::config]
pub struct Conf {
    /// SQLite database path
    #[serde(default = "default_database")]
    pub database: String,
}

fn default_database() -> String {
    "ao3fti.db".to_string()
}
