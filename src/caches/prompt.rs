//! Interactive selection and human-readable cache output: parsing the user's
//! multi-select answer, confirmation prompts, and printing the cache list and
//! clean summary. No deletion logic lives here.

use super::clean::CacheCleanResult;
use super::registry::CacheEntry;
use crate::utils::format_bytes;
use anyhow::Result;
use colored::Colorize;
use std::io::{self, BufRead, Write};

/// Parse a 1-based, comma/space separated selection into 0-based indices,
/// deduplicated and bounded by `count`. Out-of-range and non-numeric tokens are
/// silently ignored.
pub fn parse_selection(input: &str, count: usize) -> Vec<usize> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for token in input.split(|c: char| c == ',' || c.is_whitespace()) {
        if let Ok(n) = token.trim().parse::<usize>()
            && n >= 1
            && n <= count
            && seen.insert(n - 1)
        {
            out.push(n - 1);
        }
    }
    out
}

pub(crate) fn print_cache_list(caches: &[CacheEntry]) {
    println!("{} Available caches to clean:", "[INFO]".blue().bold());
    println!();
    println!(
        "  {:>3}  {:<26} {:<10} {:<6} {:>10}",
        "#", "Cache", "Category", "Risk", "Size"
    );
    for (i, c) in caches.iter().enumerate() {
        println!(
            "  {:>3}  {:<26} {:<10} {:<6} {:>10}",
            i + 1,
            c.name,
            c.category,
            c.risk,
            format_bytes(c.size_bytes)
        );
        if let Some(ref note) = c.note {
            println!("        {}", note.dimmed());
        }
    }
}

pub(crate) fn print_cache_results(results: &[CacheCleanResult]) {
    println!();
    println!("{} === CACHE CLEAN SUMMARY ===", "[INFO]".blue().bold());
    let total_freed: u64 = results.iter().map(|r| r.freed_bytes).sum();
    let ok = results.iter().filter(|r| r.success).count();
    let failed = results.len() - ok;
    for r in results {
        if r.success {
            println!(
                "  {} {} (freed: {})",
                "✓".green(),
                r.name,
                format_bytes(r.freed_bytes)
            );
        } else {
            println!(
                "  {} {} - {}",
                "✗".red(),
                r.name,
                r.error.as_deref().unwrap_or("failed")
            );
        }
    }
    println!();
    if failed == 0 {
        println!(
            "{} Cleaned {} cache(s), freed {}",
            "[SUCCESS]".green().bold(),
            ok,
            format_bytes(total_freed)
        );
    } else {
        println!(
            "{} Cleaned {}, failed {}, freed {}",
            "[INFO]".blue().bold(),
            ok,
            failed,
            format_bytes(total_freed)
        );
    }
}

pub(crate) fn prompt_selection(count: usize) -> Result<Vec<usize>> {
    print!("Select caches to clean (comma-separated numbers, 'all', or 'q' to quit): ");
    io::stdout().flush()?;
    let mut line = String::new();
    io::stdin().lock().read_line(&mut line)?;
    let trimmed = line.trim().to_ascii_lowercase();
    match trimmed.as_str() {
        "q" | "quit" | "exit" | "n" | "" => Ok(Vec::new()),
        "all" | "a" | "y" | "yes" => Ok((0..count).collect()),
        other => Ok(parse_selection(other, count)),
    }
}

pub(crate) fn confirm() -> Result<bool> {
    print!("Proceed? [y/N]: ");
    io::stdout().flush()?;
    let mut line = String::new();
    io::stdin().lock().read_line(&mut line)?;
    Ok(matches!(
        line.trim().to_ascii_lowercase().as_str(),
        "y" | "yes"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_selection_basic() {
        assert_eq!(parse_selection("1,3", 5), vec![0, 2]);
        assert_eq!(parse_selection("1 3", 5), vec![0, 2]);
    }

    #[test]
    fn test_parse_selection_out_of_range() {
        assert!(parse_selection("0", 5).is_empty());
        assert!(parse_selection("6", 5).is_empty());
    }

    #[test]
    fn test_parse_selection_dedup() {
        assert_eq!(parse_selection("1,1,2", 5), vec![0, 1]);
    }

    #[test]
    fn test_parse_selection_invalid_ignored() {
        assert_eq!(parse_selection("1,foo,3", 5), vec![0, 2]);
    }

    #[test]
    fn test_parse_selection_all_tokens() {
        assert_eq!(parse_selection("1,2,3,4,5", 5), vec![0, 1, 2, 3, 4]);
    }

    #[test]
    fn test_parse_selection_mixed_separators() {
        assert_eq!(parse_selection("1, 2 3", 5), vec![0, 1, 2]);
    }

    #[test]
    fn test_parse_selection_empty() {
        assert!(parse_selection("", 5).is_empty());
    }
}
