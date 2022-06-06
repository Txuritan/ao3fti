mod verbose;

use clap::{Parser, Subcommand};
use tracing_error::ErrorLayer;
use tracing_subscriber::{prelude::*, EnvFilter, Registry};

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
    let cli = Cli::parse();

    let subscriber = Registry::default()
        .with(ErrorLayer::default())
        .with(tracing_subscriber::fmt::Layer::default())
        .with(EnvFilter::from_default_env().add_directive(cli.verbose.log_level_filter().into()));

    tracing::subscriber::set_global_default(subscriber)?;

    ao3fti_common::install()?;

    match cli.command {
        Commands::Scrape { url } => ao3fti_command_scrape::run(&url).await?,
        Commands::Serve => ao3fti_command_serve::run().await?,
    }

    Ok(())
}
