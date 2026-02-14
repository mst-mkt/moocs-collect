use clap::Parser;
use color_eyre::Result;
use std::path::PathBuf;

mod app;
mod components;
mod config;
mod download;
mod error;
mod service;
mod state;
mod ui;

use app::App;

#[derive(Parser)]
#[command(
    name = "collect-tui",
    version,
    about = "INIAD MOOCs スライドダウンローダー"
)]
struct Args {
    #[arg(long)]
    path: Option<PathBuf>,

    #[arg(long)]
    year: Option<u32>,

    #[arg(long, short = 'j')]
    concurrency: Option<usize>,
}

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;

    let args = Args::parse();
    let mut app = App::new(args.path, args.year, args.concurrency);
    app.run().await
}
