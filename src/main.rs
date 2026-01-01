//! YandexMusicMod - Rust implementation
//!
//! A fast patcher for Yandex Music desktop application that enables premium features.
//! This is a Rust rewrite of the original TypeScript YandexMusicBetaMod project.

mod api;
mod error;
mod patcher;
mod patches;

use anyhow::Result;
use clap::{Parser, Subcommand};
use indicatif::{ProgressBar, ProgressStyle};
use std::io::{self, Write};
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

/// Wait for user to press Enter before exiting.
/// This is useful when the program is run by double-clicking the exe on Windows,
/// so users can see the output before the console window closes.
fn wait_for_enter() {
    print!("\nPress Enter to exit...");
    let _ = io::stdout().flush();
    let _ = io::stdin().read_line(&mut String::new());
}

/// Check if we're running in an interactive console that will stay open,
/// or if we were likely launched by double-clicking the exe.
fn should_wait_before_exit() -> bool {
    // On Windows, check if we have a console attached that we own
    #[cfg(target_os = "windows")]
    {
        // If there's no TERM, SHELL, or other terminal indicators,
        // and we're on Windows, we're likely double-clicked
        std::env::var("TERM").is_err()
            && std::env::var("SHELL").is_err()
            && std::env::var("WT_SESSION").is_err()
            && std::env::var("PROMPT").is_err()
    }

    #[cfg(not(target_os = "windows"))]
    {
        false
    }
}

#[derive(Parser)]
#[command(name = "yandex-music-mod")]
#[command(author = "Jhon-Crow")]
#[command(version = "0.1.0")]
#[command(about = "A fast Rust patcher for Yandex Music desktop app", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Enable verbose logging
    #[arg(short, long, global = true)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Patch the latest stable Yandex Music build
    Patch {
        /// Output directory for the patched build
        #[arg(short, long, default_value = ".versions")]
        output: String,

        /// Enable auto-open devtools on startup
        #[arg(long)]
        auto_devtools: bool,
    },

    /// Download the latest Yandex Music build without patching
    Download {
        /// Output directory for the downloaded build
        #[arg(short, long, default_value = ".versions")]
        output: String,
    },

    /// Show information about the latest available build
    Info,
}

#[tokio::main]
async fn main() {
    let wait_on_exit = should_wait_before_exit();

    match run().await {
        Ok(_) => {
            if wait_on_exit {
                println!("\nOperation completed successfully!");
                wait_for_enter();
            }
        }
        Err(e) => {
            eprintln!("\nError: {:#}", e);
            if wait_on_exit {
                wait_for_enter();
            }
            std::process::exit(1);
        }
    }
}

async fn run() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    let level = if cli.verbose {
        Level::DEBUG
    } else {
        Level::INFO
    };

    let subscriber = FmtSubscriber::builder().with_max_level(level).finish();
    tracing::subscriber::set_global_default(subscriber)?;

    // Default to Patch command if no subcommand is provided
    let command = cli.command.unwrap_or_else(|| {
        println!("No command specified, defaulting to 'patch' command...\n");
        Commands::Patch {
            output: ".versions".to_string(),
            auto_devtools: false,
        }
    });

    match command {
        Commands::Patch {
            output,
            auto_devtools,
        } => {
            info!("Fetching latest stable build information...");

            let builds = api::get_stable_build().await?;

            if builds.is_empty() {
                anyhow::bail!("No builds found");
            }

            let build = &builds[0];
            info!("Found build: {} (version {})", build.path, build.version);

            let pb = ProgressBar::new(100);
            pb.set_style(
                ProgressStyle::default_bar()
                    .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta}) {msg}")?
                    .progress_chars("#>-"),
            );

            patcher::process_build(build, &output, auto_devtools, Some(&pb)).await?;

            pb.finish_with_message("Patching complete!");
            info!("Successfully patched Yandex Music v{}", build.version);
        }

        Commands::Download { output } => {
            info!("Fetching latest stable build information...");

            let builds = api::get_stable_build().await?;

            if builds.is_empty() {
                anyhow::bail!("No builds found");
            }

            let build = &builds[0];
            info!("Found build: {} (version {})", build.path, build.version);

            let output_path = format!("{}/{}.exe", output, build.version);
            std::fs::create_dir_all(&output)?;

            info!("Downloading to {}...", output_path);
            api::download_build(build, &output_path).await?;

            info!("Download complete: {}", output_path);
        }

        Commands::Info => {
            info!("Fetching latest stable build information...");

            let builds = api::get_stable_build().await?;

            if builds.is_empty() {
                println!("No builds found");
                return Ok(());
            }

            println!("\nAvailable builds:");
            println!("{}", "=".repeat(60));

            for build in builds {
                println!("Version:      {}", build.version);
                println!("File:         {}", build.path);
                println!("Size:         {} bytes", build.size);
                println!("SHA-512:      {}...", &build.hash[..32]);
                if let Some(date) = &build.release_date {
                    println!("Release Date: {}", date);
                }
                println!("{}", "-".repeat(60));
            }
        }
    }

    Ok(())
}
