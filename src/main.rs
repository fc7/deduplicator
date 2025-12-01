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
use processor::Processor;
use scanner::Scanner;
use std::fs;
use std::sync::atomic::Ordering;

use crate::fileinfo::FileSource;

fn main() -> Result<()> {
    let app_args = Params::parse();

    if app_args.comparison_mode {
        // Comparison mode: scan both staging and target directories
        let staging_dir = app_args.get_staging_directory()?;
        let target_dir = app_args.get_target_directory()?;

        let scanner = Scanner::build(&app_args)?;
        
        let mut staging_files = scanner.scan_with_source(staging_dir, FileSource::Staging)?;
        let mut target_files = scanner.scan_with_source(target_dir, FileSource::Target)?;

        // Combine all files for processing
        staging_files.append(&mut target_files);
        let all_files = staging_files;

        let processor = Processor::new(all_files);
        // In comparison mode, we hash all files (not just duplicates) to find files
        // that exist in both staging and target folders
        let comparison_result = processor.comparison_mode()?;

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
        // Normal mode: use Server architecture
        let server = Server::new(app_args.clone());

        server.start()?;

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
