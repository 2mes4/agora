//! CLI binary for running the AGORA / A2A conformance test suite.

use agora_conformance::ConformanceRunner;
use clap::Parser;

#[derive(Parser)]
#[command(
    name = "agora-conformance",
    author,
    version,
    about = "A2A conformance test runner"
)]
struct Args {
    /// Target A2A endpoint URL (e.g. http://127.0.0.1:7100/a2a/echo)
    #[arg(short, long, default_value = "http://127.0.0.1:7100/a2a/echo")]
    url: String,

    /// Optional API key for auth
    #[arg(short, long, env = "AGORA_API_KEY")]
    api_key: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let args = Args::parse();

    println!("Running A2A conformance tests against: {}", args.url);
    let mut runner = ConformanceRunner::new(&args.url);
    if let Some(key) = args.api_key {
        runner = runner.with_api_key(key);
    }

    let report = runner.run_all().await;

    println!("\n--- Conformance Results ---");
    for res in &report.results {
        let status = if res.passed { "PASS" } else { "FAIL" };
        println!("[{}] {}", status, res.name);
        if let Some(d) = &res.details {
            println!("      Details: {}", d);
        }
    }
    println!("---------------------------");
    println!(
        "Total: {} | Passed: {} | Failed: {}",
        report.total, report.passed, report.failed
    );

    if report.failed > 0 {
        std::process::exit(1);
    }

    Ok(())
}
