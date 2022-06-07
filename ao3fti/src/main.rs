mod verbose;

use clap::{Parser, Subcommand, IntoApp as _, FromArgMatches as _};
use tracing_error::ErrorLayer;
use tracing_subscriber::{prelude::*, EnvFilter, Registry};
use twelf::Layer;

use ao3fti_common::Conf;

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
#[clap(propagate_version = true)]
struct Cli {
    #[clap(flatten)]
    verbose: verbose::Verbosity,

    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// The URL to scrape and index
    Scrape { url: String },
    /// Start the built-in web server
    Serve,
}

#[tokio::main]
async fn main() -> Result<(), ao3fti_common::Report> {
    ao3fti_common::install()?;

    let matches = Cli::command().args(&Conf::clap_args()).get_matches();
    let cli = Cli::from_arg_matches(&matches)?;
    let conf = Conf::with_layers(&[
        Layer::Json("ao3fti.json".into()),
        Layer::Toml("ao3fti.toml".into()),
        Layer::Env(Some("AO3FTI_".to_string())),
        Layer::Clap(matches)
    ])?;

    let subscriber = Registry::default()
        .with(ErrorLayer::default())
        .with(tracing_subscriber::fmt::Layer::default())
        .with(EnvFilter::from_default_env().add_directive(cli.verbose.log_level_filter().into()));

    tracing::subscriber::set_global_default(subscriber)?;

    match cli.command {
        Commands::Scrape { url } => ao3fti_command_scrape::run(&conf, &url).await?,
        Commands::Serve => ao3fti_command_serve::run(&conf).await?,
    }

    Ok(())
}
