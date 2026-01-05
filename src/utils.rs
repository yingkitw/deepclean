use anyhow::Result;
use std::path::Path;
use walkdir::WalkDir;

/// Format bytes into human-readable string
pub fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_idx = 0;

    while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
        size /= 1024.0;
        unit_idx += 1;
    }

    if unit_idx == 0 {
        format!("{} {}", bytes, UNITS[unit_idx])
    } else {
        format!("{:.2} {}", size, UNITS[unit_idx])
    }
}

/// Get the total size of a directory in bytes
pub fn get_directory_size(path: &Path) -> Result<u64> {
    let mut total = 0u64;
    if !path.exists() {
        return Ok(0);
    }

    for entry in WalkDir::new(path) {
        let entry = entry?;
        if entry.file_type().is_file() {
            total += entry.metadata()?.len();
        }
    }
    Ok(total)
}

/// Parse size string (e.g., "100MB", "1GB") to bytes
pub fn parse_size(size_str: &str) -> Result<u64> {
    use anyhow::anyhow;
    let size_str = size_str.trim().to_uppercase();
    let (number_str, unit) = if size_str.ends_with("B") {
        if size_str.ends_with("KB") {
            (&size_str[..size_str.len() - 2], "KB")
        } else if size_str.ends_with("MB") {
            (&size_str[..size_str.len() - 2], "MB")
        } else if size_str.ends_with("GB") {
            (&size_str[..size_str.len() - 2], "GB")
        } else if size_str.ends_with("TB") {
            (&size_str[..size_str.len() - 2], "TB")
        } else {
            (&size_str[..size_str.len() - 1], "B")
        }
    } else {
        return Err(anyhow!("Invalid size format: expected format like '100MB' or '1GB'"));
    };

    let number: f64 = number_str
        .trim()
        .parse()
        .map_err(|_| anyhow!("Invalid number in size: {}", number_str))?;

    let multiplier = match unit {
        "B" => 1,
        "KB" => 1024,
        "MB" => 1024 * 1024,
        "GB" => 1024_u64 * 1024 * 1024,
        "TB" => 1024_u64 * 1024 * 1024 * 1024,
        _ => return Err(anyhow!("Unknown unit: {}", unit)),
    };

    Ok((number * multiplier as f64) as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1024), "1.00 KB");
        assert_eq!(format_bytes(1536), "1.50 KB");
        assert_eq!(format_bytes(1048576), "1.00 MB");
        assert_eq!(format_bytes(1073741824), "1.00 GB");
    }

    #[test]
    fn test_parse_size() {
        assert_eq!(parse_size("100B").unwrap(), 100);
        assert_eq!(parse_size("1KB").unwrap(), 1024);
        assert_eq!(parse_size("1MB").unwrap(), 1048576);
        assert_eq!(parse_size("1GB").unwrap(), 1073741824);
        assert_eq!(parse_size("1.5MB").unwrap(), 1572864);
        assert!(parse_size("invalid").is_err());
    }

    #[test]
    fn test_get_directory_size_nonexistent() {
        let size = get_directory_size(Path::new("/nonexistent/path"));
        assert!(size.is_ok());
        assert_eq!(size.unwrap(), 0);
    }
}

