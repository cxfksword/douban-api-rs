use clap::Parser;
use serde::Deserialize;

#[derive(Parser, Debug, Clone, Deserialize)]
#[clap(author, version, about, long_about = None)]
pub struct Opt {
    /// Listen host
    #[clap(long, default_value = "0.0.0.0")]
    pub host: String,
    /// Listen port
    #[clap(short, long, default_value = "8080")]
    pub port: u16,
    #[clap(short, long, default_value = "3", env = "DOUBAN_API_LIMIT_SIZE")]
    pub limit: usize,
    #[clap(long, default_value = "", env = "DOUBAN_COOKIE")]
    pub cookie: String,
    #[clap(short, long)]
    pub debug: bool,
}
