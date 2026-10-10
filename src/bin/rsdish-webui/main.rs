use clap::Parser;
use rsdish::web::{WebOptions, logs::WebLogLayer, serve};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

#[derive(Debug, Parser)]
#[command(name = "rsdish-webui")]
#[command(about = "A web UI for rsdish domestic storage sync.")]
#[command(version)]
struct WebUiCli {
    #[arg(long, default_value = "127.0.0.1", help = "Address to bind.")]
    host: String,

    #[arg(short, long, default_value_t = 8787, help = "Port to bind.")]
    port: u16,

    #[arg(long, help = "Open the web UI in the default browser.")]
    open: bool,
}

#[tokio::main]
async fn main() {
    let fmt_layer = tracing_subscriber::fmt::layer();

    tracing_subscriber::registry()
        .with(tracing_subscriber::filter::LevelFilter::INFO)
        .with(fmt_layer)
        .with(WebLogLayer::new())
        .init();

    let cli = WebUiCli::parse();

    let options = WebOptions {
        host: cli.host,
        port: cli.port,
        open: cli.open,
    };

    if let Err(e) = serve(options).await {
        eprintln!("error: {:#}", e);
        std::process::exit(1);
    }
}
