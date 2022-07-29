pub mod models;
pub mod timer;
pub mod utils;

use std::path::{Path, PathBuf};

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
    /// Path to store indexed story data
    #[serde(default = "default_index")]
    pub index: PathBuf,
}

fn default_database() -> String {
    "ao3fti.db".to_string()
}

fn default_index() -> PathBuf {
    Path::new("./index").to_path_buf()
}
