use clap::{Arg, Command};
use color_eyre::Result;
use std::path::PathBuf;

mod app;
mod components;
mod download;
mod ui;

use app::App;

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;

    let matches = Command::new("collect-tui")
        .version("0.0.0")
        .about("INIAD MOOCs スライドダウンローダー")
        .arg(
            Arg::new("path")
                .long("path")
                .value_name("PATH")
                .help("Download directory path")
                .value_parser(clap::value_parser!(PathBuf)),
        )
        .arg(
            Arg::new("year")
                .long("year")
                .value_name("YEAR")
                .help("Target year")
                .value_parser(clap::value_parser!(u32)),
        )
        .arg(
            Arg::new("concurrency")
                .long("concurrency")
                .short('j')
                .value_name("NUM")
                .help("Number of concurrent downloads")
                .default_value("5")
                .value_parser(clap::value_parser!(usize)),
        )
        .get_matches();

    let download_path = matches.get_one::<PathBuf>("path").cloned();
    let year = matches.get_one::<u32>("year").copied();
    let concurrency = matches.get_one::<usize>("concurrency").copied().unwrap();

    let mut app = App::new(download_path, year, concurrency);
    app.run()
}
