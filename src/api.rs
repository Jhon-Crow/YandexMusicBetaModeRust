//! API module for fetching Yandex Music build information
//!
//! This module handles communication with the Yandex Music update server
//! to fetch the latest stable builds and download them.

use anyhow::Result;
use serde::Deserialize;
use std::fs::File;
use std::io::Write;
use tracing::{debug, info};

/// Update server base URL
const UPDATE_DOMAIN: &str = "https://music-desktop-application.s3.yandex.net";

/// Represents a single file in the update info
#[derive(Debug, Deserialize)]
struct UpdateFile {
    url: String,
    sha512: String,
    size: u64,
}

/// Common configuration from the update server
#[derive(Debug, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
struct CommonConfig {
    deprecated_versions: Option<String>,
}

/// Raw update info structure from the YAML response
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateInfo {
    files: Vec<UpdateFile>,
    release_date: Option<String>,
    update_probability: Option<f64>,
    version: String,
    common_config: Option<CommonConfig>,
}

/// Processed build information
#[derive(Debug, Clone)]
pub struct AppBuild {
    pub path: String,
    pub hash: String,
    pub size: u64,
    pub release_date: Option<String>,
    pub update_probability: Option<f64>,
    pub version: String,
    pub deprecated_versions: Option<String>,
}

/// Fetches the latest stable build information from the update server
pub async fn get_stable_build() -> Result<Vec<AppBuild>> {
    let url = format!("{}/stable/latest.yml", UPDATE_DOMAIN);
    debug!("Fetching update info from: {}", url);

    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .header(
            "User-Agent",
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
        )
        .send()
        .await?;

    let yaml_text = response.text().await?;
    debug!("Received YAML response:\n{}", yaml_text);

    let info: UpdateInfo = serde_yaml::from_str(&yaml_text)?;
    debug!("Parsed update info: {:?}", info);

    let deprecated_versions = info
        .common_config
        .as_ref()
        .and_then(|c| c.deprecated_versions.clone());

    let builds: Vec<AppBuild> = info
        .files
        .into_iter()
        .map(|file| AppBuild {
            path: file.url,
            hash: file.sha512,
            size: file.size,
            release_date: info.release_date.clone(),
            update_probability: info.update_probability,
            version: info.version.clone(),
            deprecated_versions: deprecated_versions.clone(),
        })
        .collect();

    info!("Found {} build(s)", builds.len());
    Ok(builds)
}

/// Downloads a build from the update server to the specified path
pub async fn download_build(build: &AppBuild, output_path: &str) -> Result<()> {
    let url = format!("{}/stable/{}", UPDATE_DOMAIN, build.path);
    info!("Downloading build from: {}", url);

    let client = reqwest::Client::new();
    let response = client.get(&url).send().await?;

    let bytes = response.bytes().await?;
    info!("Downloaded {} bytes", bytes.len());

    let mut file = File::create(output_path)?;
    file.write_all(&bytes)?;

    info!("Saved to: {}", output_path);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_stable_build() {
        let result = get_stable_build().await;
        assert!(result.is_ok(), "Failed to get stable build: {:?}", result);

        let builds = result.unwrap();
        assert!(!builds.is_empty(), "No builds found");

        let build = &builds[0];
        assert!(!build.version.is_empty(), "Version should not be empty");
        assert!(!build.path.is_empty(), "Path should not be empty");
        assert!(!build.hash.is_empty(), "Hash should not be empty");
    }
}
