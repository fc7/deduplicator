mod fileinfo;
mod formatter;
mod interactive;
mod params;
mod processor;
mod scanner;
mod server;

use self::{formatter::Formatter, interactive::Interactive, server::Server};
use anyhow::Result;
use clap::Parser;
use colored::Colorize;
use params::Params;
use std::fs;
use std::sync::atomic::Ordering;

fn main() -> Result<()> {
    let app_args = Params::parse();
    let server = Server::new(app_args.clone());
    server.start()?;

    if app_args.comparison_mode {
        // Analyze the results for comparison between staging and target
        let comparison_result = processor::Processor::analyze_comparison(server.hw_duplicate_set.clone())?;

        // Print warnings
        if !comparison_result.warnings.is_empty() {
            println!("\n{}", "Warnings:".yellow().bold());
            for warning in &comparison_result.warnings {
                println!("{}", warning.yellow());
            }
        }

        // Delete files from staging
        if !comparison_result.files_to_delete.is_empty() {
            println!("\n{}", "Files to be removed from staging:".red().bold());
            for file in &comparison_result.files_to_delete {
                println!("  - {}", file.path.display());
            }

            if app_args.interactive {
                match Interactive::scan_group_confirmation()? {
                    true => {
                        for file in &comparison_result.files_to_delete {
                            match fs::remove_file(&file.path) {
                                Ok(_) => println!("{}: {}", "DELETED".green(), file.path.display()),
                                Err(e) => println!("{}: {} - {}", "FAILED".red(), file.path.display(), e),
                            }
                        }
                    }
                    false => println!("{}", "\nCancelled Delete Operation.".red()),
                }
            } else {
                // Non-interactive mode: delete files directly
                for file in &comparison_result.files_to_delete {
                    match fs::remove_file(&file.path) {
                        Ok(_) => println!("{}: {}", "DELETED".green(), file.path.display()),
                        Err(e) => println!("{}: {} - {}", "FAILED".red(), file.path.display(), e),
                    }
                }
            }
        } else {
            println!("\n{}", "No duplicates found between staging and target folders.".green());
        }
    } else {
        match app_args.interactive {
            false => {
                Formatter::print(
                    server.hw_duplicate_set,
                    server.max_file_path_len.load(Ordering::Acquire),
                    &app_args,
                );
            }
            true => {
                Interactive::init(server.hw_duplicate_set, &app_args)?;
            }
        }
    }

    Ok(())
}
