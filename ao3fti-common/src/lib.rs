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
    /// PostgreSQL connection URI
    pub database: String,

    /// Your Archive Of Our Own username
    pub username: Option<String>,

    /// Your Archive Of Our Own password
    pub password: Option<String>,
}
