//! Jentayu CLI entrypoint.

use anyhow::Result;
use clap::Parser;
use jentayu::alert::{evaluate, print_alerts, AlertThresholds};
use jentayu::cli::{Cli, Commands};
use jentayu::collector::{CollectMode, Collector};
use jentayu::display::{print_snapshot, run_tui};
use jentayu::export::{write_json, write_prometheus};
use jentayu::metrics::{snapshot_for_export, SortBy};
use std::time::Duration;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    init_tracing(cli.verbose);

    let (want_proc, want_ctr) = cli.collect_flags();
    let mode = CollectMode::from_flags(want_proc, want_ctr);
    let sort_by: SortBy = cli.sort.into();
    let thresholds = AlertThresholds {
        cpu_percent: cli.alert_cpu,
        memory_percent: cli.alert_mem,
    };

    let mut collector = Collector::new(mode).await;
    if !collector.docker_available() && matches!(mode, CollectMode::Containers) {
        eprintln!("warning: Docker unavailable — no container metrics will be shown");
    }

    let command = cli.command.unwrap_or(Commands::Once);

    match command {
        Commands::Once => {
            let snap = collector.snapshot().await;
            print_snapshot(&snap, cli.filter.as_deref(), sort_by, cli.top);
            let alerts = evaluate(&snap, &thresholds);
            print_alerts(&alerts);
        }

        Commands::Watch => {
            let interval = Duration::from_secs(cli.interval.max(1));
            loop {
                // Clear screen for a simple watch experience.
                print!("\x1B[2J\x1B[1;1H");
                let snap = collector.snapshot().await;
                print_snapshot(&snap, cli.filter.as_deref(), sort_by, cli.top);
                let alerts = evaluate(&snap, &thresholds);
                print_alerts(&alerts);
                tokio::time::sleep(interval).await;
            }
        }

        Commands::Tui => {
            let config = jentayu::display::tui::TuiConfig {
                interval: Duration::from_secs(cli.interval.max(1)),
                name_filter: cli.filter.clone(),
                sort_by,
                top: cli.top,
            };
            run_tui(collector, config).await?;
        }

        Commands::Prometheus { output } => {
            let snap = collector.snapshot().await;
            let export_snap = snapshot_for_export(&snap, cli.filter.as_deref(), sort_by, cli.top);
            let body = write_prometheus(&export_snap, output.as_deref())?;

            match output {
                None => print!("{body}"),
                Some(path) => eprintln!("wrote prometheus metrics to {}", path.display()),
            }

            // Alerts always evaluate the full collected snapshot.
            let alerts = evaluate(&snap, &thresholds);
            print_alerts(&alerts);
        }

        Commands::Json { output } => {
            let snap = collector.snapshot().await;
            let export_snap = snapshot_for_export(&snap, cli.filter.as_deref(), sort_by, cli.top);
            let body = write_json(&export_snap, output.as_deref())?;

            match output {
                None => println!("{body}"),
                Some(path) => eprintln!("wrote json snapshot to {}", path.display()),
            }

            // Alerts always evaluate the full collected snapshot.
            let alerts = evaluate(&snap, &thresholds);
            print_alerts(&alerts);
        }
    }

    Ok(())
}

fn init_tracing(verbose: u8) {
    let level = match verbose {
        0 => "warn",
        1 => "info",
        2 => "debug",
        _ => "trace",
    };
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(level));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .try_init();
}
