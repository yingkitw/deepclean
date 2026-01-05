mod cleaner;
mod deps;
mod output;
mod project;
mod utils;

use anyhow::{Context, Result};
use clap::Parser;
use colored::*;
use cleaner::{clean_project, CleanResult};
use deps::clean_dependencies;
use output::{create_progress_bars, create_project_progress_bar, print_summary, print_verbose_cleaned, print_error, Summary};
use project::find_cargo_projects;
use rayon::prelude::*;
use utils::{get_directory_size, parse_size};

#[derive(Parser, Debug)]
#[command(name = "cargo-rclean")]
#[command(about = "Recursively clean Cargo projects with workspace support", long_about = None)]
#[command(bin_name = "cargo rclean")]
struct Args {
    /// Directory to start cleaning from
    #[arg(default_value = ".")]
    directory: std::path::PathBuf,

    /// Dry run mode (don't actually clean, just show what would be cleaned)
    #[arg(long)]
    dry_run: bool,

    /// Exclude patterns (glob patterns, can be specified multiple times)
    #[arg(short = 'e', long = "exclude")]
    exclude_patterns: Vec<String>,

    /// Number of parallel jobs
    #[arg(short = 'j', long = "jobs", default_value_t = num_cpus::get())]
    jobs: usize,

    /// Verbose output
    #[arg(short, long)]
    verbose: bool,

    /// JSON output
    #[arg(long)]
    json: bool,

    /// Minimum size threshold (e.g., "100MB", "1GB") - only clean projects above this size
    #[arg(long)]
    min_size: Option<String>,

    /// Clean unused dependencies (requires cargo-udeps or cargo-machete)
    #[arg(long)]
    clean_deps: bool,

    /// Remove unused dependencies (requires --clean-deps and cargo-remove)
    #[arg(long, requires = "clean_deps")]
    remove_deps: bool,
}

fn main() -> Result<()> {
    // Handle being called as a cargo subcommand
    // When invoked as `cargo rclean`, cargo passes "rclean" as the first argument
    let mut args_iter = std::env::args();
    let program_name = args_iter.next();
    
    // Check if we're being called as `cargo rclean` (first arg is "rclean")
    let args = if args_iter.next().as_deref() == Some("rclean") {
        // Skip "rclean" and parse the rest
        Args::parse_from(args_iter)
    } else {
        // Called directly as `cargo-rclean`, reconstruct args
        let mut all_args = vec![program_name.unwrap_or_else(|| "cargo-rclean".to_string())];
        all_args.extend(args_iter);
        Args::parse_from(all_args)
    };
    
    let root = args.directory.canonicalize()
        .with_context(|| format!("Failed to canonicalize path: {:?}", args.directory))?;

    if !args.json {
        println!("{} {}", "[INFO]".blue().bold(), format!("Starting cargo clean from: {:?}", root));
        println!("{} Searching for Cargo projects...", "[INFO]".blue().bold());
    }

    let projects = find_cargo_projects(&root, &args.exclude_patterns)
        .context("Failed to find Cargo projects")?;

    if projects.is_empty() {
        if !args.json {
            println!("{} No Cargo projects found", "[WARNING]".yellow().bold());
        }
        return Ok(());
    }

    // Filter by minimum size if specified
    let min_size_bytes = if let Some(ref min_size_str) = args.min_size {
        Some(parse_size(min_size_str)
            .with_context(|| format!("Invalid --min-size value: '{}'. Expected format like '100MB' or '1GB'", min_size_str))?)
    } else {
        None
    };

    let projects: Vec<_> = if let Some(min_bytes) = min_size_bytes {
        projects
            .into_iter()
            .filter(|project| {
                let target_dir = project.path.join("target");
                if target_dir.exists() {
                    get_directory_size(&target_dir).unwrap_or(0) >= min_bytes
                } else {
                    false
                }
            })
            .collect()
    } else {
        projects
    };

    if projects.is_empty() {
        if !args.json {
            if min_size_bytes.is_some() {
                println!("{} No projects found above the minimum size threshold", "[INFO]".blue().bold());
            } else {
                println!("{} No Cargo projects found", "[WARNING]".yellow().bold());
            }
        }
        return Ok(());
    }

    if !args.json {
        println!("{} Found {} project(s)", "[INFO]".blue().bold(), projects.len());
        if args.dry_run {
            println!("{} DRY RUN MODE - no changes will be made", "[INFO]".yellow().bold());
        }
        if args.clean_deps {
            println!("{} Dependency cleaning enabled (native detection)", "[INFO]".blue().bold());
            if args.remove_deps {
                println!("{} Will remove unused dependencies (requires cargo-remove)", "[INFO]".yellow().bold());
            }
        } else {
            // Hint about dependency cleaning feature
            println!("{} Tip: Use --clean-deps to also check for unused dependencies", "[INFO]".blue().bold());
        }
        println!();
    }

    let (multi, overall_pb) = create_progress_bars(projects.len(), !args.json && !args.verbose);

    let results: Vec<CleanResult> = projects
        .par_iter()
        .with_min_len(1)
        .map(|project| {
            // Create individual progress bar for this project
            let project_pb = if let Some(ref multi) = multi {
                Some(create_project_progress_bar(multi, &project.path))
            } else {
                None
            };

            if args.verbose && !args.json {
                println!("{} Cleaning: {:?}", "[INFO]".blue().bold(), project.path);
            }

            // Clean target directory
            let result = clean_project(project, args.dry_run, args.verbose);

            // Clean unused dependencies if requested
            if args.clean_deps {
                let deps_result = clean_dependencies(project, args.dry_run, args.remove_deps, args.verbose);
                match deps_result {
                    Ok(deps_clean) => {
                        if !deps_clean.unused_deps.is_empty() {
                            if !args.json {
                                // Always show unused dependencies, not just in verbose mode
                                println!(
                                    "{} Found {} unused dependency(ies) in {}:",
                                    "[INFO]".blue().bold(),
                                    deps_clean.unused_deps.len(),
                                    project.path.display()
                                );
                                for dep in &deps_clean.unused_deps {
                                    println!("  {} {} ({})", "•".yellow(), dep.name.bright_yellow(), dep.location);
                                }
                                if deps_clean.removed_count > 0 {
                                    println!(
                                        "{} Removed {} unused dependency(ies)",
                                        "[SUCCESS]".green().bold(),
                                        deps_clean.removed_count
                                    );
                                } else if args.remove_deps && !args.dry_run {
                                    println!(
                                        "{} Could not remove dependencies (install cargo-remove: cargo install cargo-edit)",
                                        "[WARNING]".yellow().bold()
                                    );
                                } else if args.dry_run {
                                    println!(
                                        "{} Would remove {} dependency(ies) (use --remove-deps to actually remove)",
                                        "[INFO]".blue().bold(),
                                        deps_clean.unused_deps.len()
                                    );
                                }
                            }
                        } else if args.verbose && !args.json {
                            println!(
                                "{} No unused dependencies found in {}",
                                "[INFO]".blue().bold(),
                                project.path.display()
                            );
                        }
                    }
                    Err(e) => {
                        if !args.json {
                            println!(
                                "{} Failed to check dependencies in {:?}: {}",
                                "[WARNING]".yellow().bold(),
                                project.path,
                                e
                            );
                            if args.verbose {
                                println!("  Hint: Install cargo-machete (works on stable): cargo install cargo-machete");
                                println!("  Or install cargo-udeps (requires nightly): cargo install cargo-udeps");
                            }
                        }
                    }
                }
            }

            // Finish individual progress bar
            if let Some(ref pb) = project_pb {
                let project_name = project.path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| project.path.to_string_lossy().to_string());
                pb.finish_with_message(format!("✓ {}", project_name));
            }

            // Update overall progress
            if let Some(ref overall) = overall_pb {
                overall.inc(1);
            }

            match result {
                Ok(r) => {
                    if args.verbose && !args.json {
                        print_verbose_cleaned(&r);
                    }
                    Ok(r)
                }
                Err(e) => {
                    let error_msg = e.to_string();
                    if !args.json {
                        print_error(&project.path, &error_msg);
                    }
                    Ok(CleanResult {
                        path: project.path.to_string_lossy().to_string(),
                        success: false,
                        freed_bytes: 0,
                        error: Some(error_msg),
                    })
                }
            }
        })
        .collect::<Result<Vec<_>>>()?;

    if let Some(ref overall) = overall_pb {
        overall.finish_with_message("All projects completed!");
    }

    let cleaned = results.iter().filter(|r| r.success).count();
    let failed = results.len() - cleaned;
    let total_freed: u64 = results.iter().map(|r| r.freed_bytes).sum();

    let summary = Summary {
        total_projects: projects.len(),
        cleaned,
        failed,
        total_freed_bytes: total_freed,
        results,
    };

    if args.json {
        println!("{}", serde_json::to_string_pretty(&summary)?);
    } else {
        print_summary(&summary);
    }

    if failed > 0 {
        std::process::exit(1);
    }

    Ok(())
}
