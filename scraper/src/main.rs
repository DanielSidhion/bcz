use std::{path::{PathBuf, Path}};

use bcz::{ScraperState, bc::BcScraper};
use clap::Parser;
use tokio_stream::StreamExt;
use tracing::{info, Instrument, Level, metadata::LevelFilter};
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
struct Args {
    #[clap(short, long, parse(from_os_str))]
    state_path: PathBuf,
    #[clap(short, long)]
    resume: bool,
}

fn read_or_create_state<P: AsRef<Path>>(state_path: P) -> Result<ScraperState, Box<dyn std::error::Error>> {
    let state_string = std::fs::read_to_string(state_path)?;
    Ok(serde_json::from_str(&state_string)?)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let filter = EnvFilter::new("debug,html5ever=error,hyper=error,reqwest=error,selectors=error");

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .init();
    info!("Starting a new state");
    // let state = read_or_create_state(args.state_path)?;
    let state = ScraperState::new();
    let scraper = BcScraper::with_state(state);

    let mut artists = scraper.artists();
    let mut total_artists = 1000;

    while let Some(artist_url) = artists.next().instrument(tracing::info_span!("artists_stream")).await {
        info!(artist_url = ?artist_url, "Got a new artist!");
        total_artists -= 1;

        if total_artists <= 0 {
            break;
        }
    }

    Ok(())
}
