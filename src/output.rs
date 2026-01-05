use crate::cleaner::CleanResult;
use crate::utils::format_bytes;
use colored::Colorize;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::sync::Arc;

#[derive(Debug, serde::Serialize)]
pub struct Summary {
    pub total_projects: usize,
    pub cleaned: usize,
    pub failed: usize,
    pub total_freed_bytes: u64,
    pub results: Vec<CleanResult>,
}

/// Create progress bars for cleaning operations
pub fn create_progress_bars(
    project_count: usize,
    show_progress: bool,
) -> (Option<Arc<MultiProgress>>, Option<ProgressBar>) {
    if !show_progress {
        return (None, None);
    }

    let multi = Arc::new(MultiProgress::new());
    let overall_pb = {
        let pb = multi.add(ProgressBar::new(project_count as u64));
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} projects completed")
                .unwrap()
                .progress_chars("#>-"),
        );
        pb.set_message("Starting...");
        pb
    };

    (Some(multi), Some(overall_pb))
}

/// Create a progress bar for an individual project
pub fn create_project_progress_bar(
    multi: &Arc<MultiProgress>,
    project_path: &std::path::Path,
) -> ProgressBar {
    let pb = multi.add(ProgressBar::new_spinner());
    pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.green} {msg}")
            .unwrap()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
    );
    let project_name = project_path
        .file_name()
        .and_then(|n| n.to_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| project_path.to_string_lossy().to_string());
    pb.set_message(format!("Cleaning: {}", project_name));
    pb.enable_steady_tick(std::time::Duration::from_millis(100));
    pb
}

/// Print initial information
pub fn print_start_info(root: &std::path::Path, project_count: usize, dry_run: bool) {
    println!("{} {}", "[INFO]".blue().bold(), format!("Starting cargo clean from: {:?}", root));
    println!("{} Searching for Cargo projects...", "[INFO]".blue().bold());
    println!("{} Found {} project(s)", "[INFO]".blue().bold(), project_count);
    if dry_run {
        println!("{} DRY RUN MODE - no changes will be made", "[INFO]".yellow().bold());
    }
    println!();
}

/// Print summary
pub fn print_summary(summary: &Summary) {
    println!();
    println!("{} {}", "[INFO]".blue().bold(), "=== SUMMARY ===");
    println!(
        "{} Successfully cleaned: {} project(s)",
        "[SUCCESS]".green().bold(),
        summary.cleaned
    );

    if summary.total_freed_bytes > 0 {
        println!(
            "{} Total storage freed: {}",
            "[SUCCESS]".green().bold(),
            format_bytes(summary.total_freed_bytes)
        );
    } else {
        println!("{} No storage was freed", "[INFO]".blue().bold());
    }

    if summary.failed > 0 {
        println!(
            "{} Failed to clean: {} project(s)",
            "[ERROR]".red().bold(),
            summary.failed
        );
    } else {
        println!("{} All done!", "[SUCCESS]".green().bold());
    }
}

/// Print verbose output for a cleaned project
pub fn print_verbose_cleaned(result: &CleanResult) {
    if result.freed_bytes > 0 {
        println!(
            "{} Cleaned: {} (freed: {})",
            "[SUCCESS]".green().bold(),
            result.path,
            format_bytes(result.freed_bytes)
        );
    } else {
        println!(
            "{} Cleaned: {} (already clean)",
            "[SUCCESS]".green().bold(),
            result.path
        );
    }
}

/// Print error message
pub fn print_error(project_path: &std::path::Path, error_msg: &str) {
    println!(
        "{} Failed to clean: {:?} - {}",
        "[ERROR]".red().bold(),
        project_path,
        error_msg
    );
}

