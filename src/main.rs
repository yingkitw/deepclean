mod cleaner;
mod config;
mod deps;
mod output;
mod project;
mod utils;

use anyhow::{Context, Result};
use clap::Parser;
use colored::control;
use colored::Colorize;
use cleaner::{clean_project, CleanResult};
use config::{load_config, resolve_settings};
use deps::clean_dependencies;
use output::{create_progress_bars, create_project_progress_bar, print_error, print_summary, print_verbose_cleaned, Summary};
use project::find_cargo_projects;
use rayon::prelude::*;
use std::io::{self, BufRead, Write};
use utils::{get_directory_size, parse_size};

#[derive(Parser, Debug)]
#[command(name = "cargo-deepclean")]
#[command(about = "Recursively clean Cargo projects with workspace support", long_about = None)]
#[command(bin_name = "cargo deepclean")]
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
    #[arg(short = 'j', long = "jobs")]
    jobs: Option<usize>,

    /// Verbose output
    #[arg(short, long)]
    verbose: bool,

    /// JSON output
    #[arg(long)]
    json: bool,

    /// Minimum size threshold (e.g., "100MB", "1GB") - only clean projects above this size
    #[arg(long)]
    min_size: Option<String>,

    /// Check for unused dependencies (native detection)
    #[arg(long)]
    clean_deps: bool,

    /// Remove unused dependencies (automatically enables --clean-deps, requires cargo-remove)
    #[arg(long)]
    remove_deps: bool,

    /// Prompt for confirmation before cleaning
    #[arg(long)]
    interactive: bool,
}

fn parse_args() -> Args {
    let mut args_iter = std::env::args();
    let program_name = args_iter.next();

    let first_arg = args_iter.next();
    if first_arg.as_deref() == Some("deepclean") {
        Args::parse_from(args_iter)
    } else {
        let mut all_args = vec![program_name.unwrap_or_else(|| "cargo-deepclean".to_string())];
        if let Some(arg) = first_arg {
            all_args.push(arg);
        }
        all_args.extend(args_iter);
        Args::parse_from(all_args)
    }
}

fn confirm_interactive(projects: &[project::Project], dry_run: bool) -> Result<bool> {
    println!();
    println!(
        "{} Found {} project(s) to clean:",
        "[INFO]".blue().bold(),
        projects.len()
    );
    for project in projects {
        let target_dir = project.path.join("target");
        let size = if target_dir.exists() {
            get_directory_size(&target_dir).unwrap_or(0)
        } else {
            0
        };
        println!(
            "  • {} ({})",
            project.path.display(),
            utils::format_bytes(size)
        );
    }

    if dry_run {
        println!(
            "{} Dry run mode: no changes will be made if you continue.",
            "[INFO]".yellow().bold()
        );
    }

    print!(
        "{} Proceed with cleaning? [y/N]: ",
        "[PROMPT]".cyan().bold()
    );
    io::stdout().flush()?;

    let mut line = String::new();
    io::stdin().lock().read_line(&mut line)?;
    Ok(matches!(line.trim().to_ascii_lowercase().as_str(), "y" | "yes"))
}

fn main() -> Result<()> {
    let args = parse_args();

    let config = load_config(&args.directory)
        .context("Failed to load configuration. Check .deepclean.toml syntax and permissions.")?;

    let settings = resolve_settings(
        config.as_ref(),
        &args.exclude_patterns,
        args.jobs,
        args.min_size.as_deref(),
        args.json,
    );

    control::set_override(settings.use_color);

    let root = args
        .directory
        .canonicalize()
        .with_context(|| {
            format!(
                "Failed to resolve directory {:?}. \
Ensure the path exists and you have permission to read it.",
                args.directory
            )
        })?;

    if !settings.json {
        println!(
            "{} Starting cargo clean from: {:?}",
            "[INFO]".blue().bold(),
            root
        );
        println!(
            "{} Searching for Cargo projects...",
            "[INFO]".blue().bold()
        );
    }

    rayon::ThreadPoolBuilder::new()
        .num_threads(settings.jobs)
        .build_global()
        .context("Failed to configure parallel job count")?;

    let projects = find_cargo_projects(&root, &settings.exclude_patterns)
        .context("Failed to find Cargo projects while scanning the directory tree")?;

    if projects.is_empty() {
        if !settings.json {
            println!(
                "{} No Cargo projects found",
                "[WARNING]".yellow().bold()
            );
        }
        return Ok(());
    }

    let min_size_bytes = if let Some(ref min_size_str) = settings.min_size {
        Some(parse_size(min_size_str).with_context(|| {
            format!(
                "Invalid min-size value '{}'. Expected a size like '100MB', '1GB', or '500KB'.",
                min_size_str
            )
        })?)
    } else {
        None
    };

    let projects: Vec<_> = if let Some(min_bytes) = min_size_bytes {
        projects
            .par_iter()
            .filter(|project| {
                let target_dir = project.path.join("target");
                if target_dir.exists() {
                    get_directory_size(&target_dir).unwrap_or(0) >= min_bytes
                } else {
                    false
                }
            })
            .cloned()
            .collect()
    } else {
        projects
    };

    if projects.is_empty() {
        if !settings.json {
            if min_size_bytes.is_some() {
                println!(
                    "{} No projects found above the minimum size threshold",
                    "[INFO]".blue().bold()
                );
            } else {
                println!(
                    "{} No Cargo projects found",
                    "[WARNING]".yellow().bold()
                );
            }
        }
        return Ok(());
    }

    if args.interactive && !confirm_interactive(&projects, args.dry_run)? {
        if !settings.json {
            println!("{} Cancelled by user", "[INFO]".blue().bold());
        }
        return Ok(());
    }

    if !settings.json {
        println!(
            "{} Found {} project(s)",
            "[INFO]".blue().bold(),
            projects.len()
        );
        if args.dry_run {
            println!(
                "{} DRY RUN MODE - no changes will be made",
                "[INFO]".yellow().bold()
            );
        }
        let clean_deps = args.clean_deps || args.remove_deps;
        if clean_deps {
            println!(
                "{} Dependency cleaning enabled (native detection)",
                "[INFO]".blue().bold()
            );
            if args.remove_deps {
                println!(
                    "{} Will remove unused dependencies (requires cargo-remove)",
                    "[INFO]".yellow().bold()
                );
            }
        }
        println!();
    }

    let (multi, overall_pb) =
        create_progress_bars(projects.len(), !settings.json && !args.verbose);

    let results: Vec<CleanResult> = projects
        .par_iter()
        .with_min_len(1)
        .map(|project| {
            let project_pb = multi
                .as_ref()
                .map(|multi| create_project_progress_bar(multi, &project.path));

            if args.verbose && !settings.json {
                println!(
                    "{} Cleaning: {:?}",
                    "[INFO]".blue().bold(),
                    project.path
                );
            }

            let result = clean_project(project, args.dry_run, args.verbose);

            if args.clean_deps || args.remove_deps {
                let deps_result =
                    clean_dependencies(project, args.dry_run, args.remove_deps, args.verbose);
                match deps_result {
                    Ok(deps_clean) => {
                        if !deps_clean.unused_deps.is_empty() {
                            if !settings.json {
                                println!(
                                    "{} Found {} unused dependency(ies) in {}:",
                                    "[INFO]".blue().bold(),
                                    deps_clean.unused_deps.len(),
                                    project.path.display()
                                );
                                for dep in &deps_clean.unused_deps {
                                    println!(
                                        "  {} {} ({})",
                                        "•".yellow(),
                                        dep.name.bright_yellow(),
                                        dep.location
                                    );
                                }
                                if deps_clean.removed_count > 0 {
                                    println!(
                                        "{} Removed {} unused dependency(ies)",
                                        "[SUCCESS]".green().bold(),
                                        deps_clean.removed_count
                                    );
                                } else if args.remove_deps && !args.dry_run {
                                    if let Some(ref error) = deps_clean.error {
                                        println!(
                                            "{} Failed to remove dependencies: {}. \
Install cargo-edit with `cargo install cargo-edit`.",
                                            "[ERROR]".red().bold(),
                                            error
                                        );
                                    } else {
                                        println!(
                                            "{} Could not remove dependencies. \
Install cargo-remove with `cargo install cargo-edit`.",
                                            "[WARNING]".yellow().bold()
                                        );
                                    }
                                } else if args.dry_run {
                                    println!(
                                        "{} Would remove {} dependency(ies) (use --remove-deps to actually remove)",
                                        "[INFO]".blue().bold(),
                                        deps_clean.unused_deps.len()
                                    );
                                }
                            }
                        } else if !settings.json && args.verbose {
                            println!(
                                "{} No unused dependencies found in {}",
                                "[INFO]".blue().bold(),
                                project.path.display()
                            );
                        }

                        if let Some(ref error) = deps_clean.error {
                            if !settings.json {
                                println!(
                                    "{} Error during dependency removal in {:?}: {}",
                                    "[ERROR]".red().bold(),
                                    project.path,
                                    error
                                );
                            }
                        }
                    }
                    Err(e) => {
                        if !settings.json {
                            println!(
                                "{} Failed to check dependencies in {:?}: {}",
                                "[WARNING]".yellow().bold(),
                                project.path,
                                e
                            );
                        }
                    }
                }
            }

            if let Some(ref pb) = project_pb {
                let project_name = project
                    .path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| project.path.to_string_lossy().to_string());
                pb.finish_with_message(format!("✓ {}", project_name));
            }

            if let Some(ref overall) = overall_pb {
                overall.inc(1);
            }

            match result {
                Ok(r) => {
                    if args.verbose && !settings.json {
                        print_verbose_cleaned(&r);
                    }
                    Ok(r)
                }
                Err(e) => {
                    let error_msg = e.to_string();
                    if !settings.json {
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

    if settings.json {
        println!("{}", serde_json::to_string_pretty(&summary)?);
    } else {
        print_summary(&summary);
    }

    if failed > 0 {
        std::process::exit(1);
    }

    Ok(())
}
