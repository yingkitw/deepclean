use anyhow::{Context, Result};
use serde::Deserialize;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};

const CONFIG_FILENAME: &str = ".deepclean.toml";

#[derive(Debug, Clone, Default, Deserialize)]
pub struct DeepcleanConfig {
    #[serde(default)]
    pub defaults: DefaultsConfig,
    #[serde(default)]
    pub output: OutputConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct DefaultsConfig {
    #[serde(default)]
    pub exclude: Vec<String>,
    pub jobs: Option<usize>,
    pub min_size: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct OutputConfig {
    pub color: Option<bool>,
    pub format: Option<String>,
}

/// Resolved settings after merging config file with CLI arguments.
#[derive(Debug, Clone)]
pub struct ResolvedSettings {
    pub exclude_patterns: Vec<String>,
    pub jobs: usize,
    pub min_size: Option<String>,
    pub json: bool,
    pub use_color: bool,
}

/// Load `.deepclean.toml` from `start_dir`, then fall back to the home directory.
pub fn load_config(start_dir: &Path) -> Result<Option<DeepcleanConfig>> {
    if let Some(config) = try_load_from(start_dir.join(CONFIG_FILENAME))? {
        return Ok(Some(config));
    }

    if let Some(home) = home_config_path() {
        if let Some(config) = try_load_from(home)? {
            return Ok(Some(config));
        }
    }

    Ok(None)
}

fn home_config_path() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|home| PathBuf::from(home).join(CONFIG_FILENAME))
}

fn try_load_from(path: PathBuf) -> Result<Option<DeepcleanConfig>> {
    if !path.is_file() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read config file: {}", path.display()))?;

    let config: DeepcleanConfig = toml::from_str(&content).with_context(|| {
        format!(
            "Failed to parse {}. Check TOML syntax and see ARCHITECTURE.md for the expected format.",
            path.display()
        )
    })?;

    Ok(Some(config))
}

/// Merge CLI arguments with config file values. CLI wins when both are set.
pub fn resolve_settings(
    config: Option<&DeepcleanConfig>,
    cli_exclude: &[String],
    cli_jobs: Option<usize>,
    cli_min_size: Option<&str>,
    cli_json: bool,
) -> ResolvedSettings {
    let defaults = config.map(|c| &c.defaults);
    let output = config.map(|c| &c.output);

    let mut exclude_patterns = defaults.map(|d| d.exclude.clone()).unwrap_or_default();
    exclude_patterns.extend_from_slice(cli_exclude);

    let jobs = cli_jobs
        .or_else(|| defaults.and_then(|d| d.jobs))
        .unwrap_or_else(num_cpus::get);

    let min_size = cli_min_size
        .map(str::to_string)
        .or_else(|| defaults.and_then(|d| d.min_size.clone()));

    let json = cli_json
        || output
            .and_then(|o| o.format.as_deref())
            .is_some_and(|f| f.eq_ignore_ascii_case("json"));

    let use_color = output
        .and_then(|o| o.color)
        .unwrap_or_else(|| std::io::stdout().is_terminal());

    ResolvedSettings {
        exclude_patterns,
        jobs,
        min_size,
        json,
        use_color,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_load_config_from_directory() {
        let temp = TempDir::new().unwrap();
        fs::write(
            temp.path().join(CONFIG_FILENAME),
            r#"
[defaults]
exclude = ["**/vendor"]
jobs = 2
min_size = "50MB"

[output]
color = false
format = "human"
"#,
        )
        .unwrap();

        let config = load_config(temp.path()).unwrap().expect("config should load");
        assert_eq!(config.defaults.exclude, vec!["**/vendor"]);
        assert_eq!(config.defaults.jobs, Some(2));
        assert_eq!(config.defaults.min_size.as_deref(), Some("50MB"));
        assert_eq!(config.output.color, Some(false));
    }

    #[test]
    fn test_resolve_settings_cli_overrides_config() {
        let config = DeepcleanConfig {
            defaults: DefaultsConfig {
                exclude: vec!["**/vendor".to_string()],
                jobs: Some(2),
                min_size: Some("50MB".to_string()),
            },
            output: OutputConfig {
                color: Some(true),
                format: Some("human".to_string()),
            },
        };

        let resolved = resolve_settings(
            Some(&config),
            &["**/node_modules".to_string()],
            Some(8),
            Some("1GB"),
            true,
        );

        assert_eq!(
            resolved.exclude_patterns,
            vec!["**/vendor", "**/node_modules"]
        );
        assert_eq!(resolved.jobs, 8);
        assert_eq!(resolved.min_size.as_deref(), Some("1GB"));
        assert!(resolved.json);
    }

    #[test]
    fn test_resolve_settings_uses_config_defaults() {
        let config = DeepcleanConfig {
            defaults: DefaultsConfig {
                exclude: vec!["**/vendor".to_string()],
                jobs: Some(4),
                min_size: Some("100MB".to_string()),
            },
            output: OutputConfig {
                color: Some(false),
                format: Some("json".to_string()),
            },
        };

        let resolved = resolve_settings(Some(&config), &[], None, None, false);

        assert_eq!(resolved.exclude_patterns, vec!["**/vendor"]);
        assert_eq!(resolved.jobs, 4);
        assert_eq!(resolved.min_size.as_deref(), Some("100MB"));
        assert!(resolved.json);
        assert!(!resolved.use_color);
    }
}
