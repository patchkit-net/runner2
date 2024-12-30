use crate::Result;
use reqwest::Client;
use serde::{Deserialize};
use std::time::{Duration, Instant};
use std::path::Path;
use log::{debug, error, warn};
use futures_util::StreamExt;
use std::fs::File;
use std::io::Write;
use bytes::Bytes;

const DEFAULT_API_URL: &str = "https://api2.patchkit.net";
const NETWORK_TEST_URLS: &[&str] = &[
    "https://network-test.patchkit.net",
    "https://api2.patchkit.net",
    "https://google.com",
];

#[derive(Debug, Clone)]
pub struct NetworkManager {
    client: Client,
    api_url: String,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum VersionId {
    String(String),
    Number(i64),
}

impl ToString for VersionId {
    fn to_string(&self) -> String {
        match self {
            VersionId::String(s) => s.clone(),
            VersionId::Number(n) => n.to_string(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct VersionResponse {
    pub id: VersionId,
}

#[derive(Debug, Deserialize)]
pub struct ContentUrl {
    pub size: u64,
    pub url: String,
}

pub struct DownloadProgress {
    pub bytes: u64,
    pub total_bytes: u64,
    pub speed_kbps: f64,
}

impl NetworkManager {
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .expect("Failed to create HTTP client");
            
        Self {
            client,
            api_url: std::env::var("PK_RUNNER_API_URL")
                .unwrap_or_else(|_| DEFAULT_API_URL.to_string()),
        }
    }

    pub async fn check_connection(&self) -> Result<bool> {
        for url in NETWORK_TEST_URLS {
            debug!("Checking network connection to {}", url);
            
            match self.client.get(*url).send().await {
                Ok(response) => {
                    debug!("Network test response status for {}: {}", url, response.status());
                    if response.status().is_success() {
                        if *url == NETWORK_TEST_URLS[0] {
                            match response.text().await {
                                Ok(body) => {
                                    debug!("Network test response body from {}: {:?}", url, body);
                                    if body.trim() == "ok" {
                                        return Ok(true);
                                    }
                                    warn!("Unexpected response body from {}: {:?}", url, body);
                                },
                                Err(e) => {
                                    warn!("Failed to read network test response from {}: {}", url, e);
                                }
                            }
                        } else {
                            debug!("Successfully connected to {}", url);
                            return Ok(true);
                        }
                    } else {
                        warn!("Network test failed with status {} for {}", response.status(), url);
                    }
                },
                Err(e) => {
                    warn!("Network test request failed for {}: {}", url, e);
                }
            }
        }

        error!("All network connection attempts failed");
        Ok(false)
    }

    pub async fn get_latest_version(&self, secret: &str) -> Result<String> {
        let url = format!("{}/1/apps/{}/versions/latest/id", self.api_url, secret);
        debug!("Fetching latest version from {}", url);
        let response: VersionResponse = self.client.get(&url).send().await?.json().await?;
        debug!("Got version response: {:?}", response);
        Ok(response.id.to_string())
    }

    pub async fn get_content_urls(&self, secret: &str, version_id: &str) -> Result<Vec<ContentUrl>> {
        let url = format!(
            "{}/1/apps/{}/versions/{}/content_urls",
            self.api_url, secret, version_id
        );
        debug!("Fetching content URLs from {}", url);
        let response = self.client.get(&url).send().await?.json().await?;
        debug!("Got content URLs response: {:?}", response);
        Ok(response)
    }

    pub async fn download_file<P: AsRef<Path>>(
        &self, 
        url: &str, 
        path: P,
        progress_callback: impl Fn(DownloadProgress) + Send + 'static,
    ) -> Result<()> {
        debug!("Downloading file from {} to {}", url, path.as_ref().display());
        
        let response = self.client.get(url).send().await?;
        let total_size = response.content_length().unwrap_or(0);
        let mut file = File::create(path)?;
        let mut downloaded: u64 = 0;
        let mut stream = response.bytes_stream();
        let start_time = Instant::now();
        
        while let Some(chunk_result) = stream.next().await {
            let chunk: Bytes = chunk_result?;
            file.write_all(&chunk)?;
            downloaded += chunk.len() as u64;
            
            let elapsed = start_time.elapsed().as_secs_f64();
            let speed = if elapsed > 0.0 {
                (downloaded as f64) / (1024.0 * elapsed)
            } else {
                0.0
            };
            
            progress_callback(DownloadProgress {
                bytes: downloaded,
                total_bytes: total_size,
                speed_kbps: speed,
            });
        }
        
        debug!("Download complete");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockall::predicate::*;
    use mockall::mock;
    use tempfile::tempdir;

    mock! {
        Client {
            fn get(&self, url: &str) -> reqwest::RequestBuilder;
        }
    }

    #[tokio::test]
    async fn test_check_connection() {
        let manager = NetworkManager::new();
        let result = manager.check_connection().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_download_file() {
        let manager = NetworkManager::new();
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.zip");
        
        // This is a mock test - in real scenario we'd mock the HTTP client
        let result = manager
            .download_file(
                "https://network-test.patchkit.net/",
                &file_path,
                |progress| {
                    println!("Downloaded: {} / {} bytes, Speed: {:.2} KB/s",
                        progress.bytes,
                        progress.total_bytes,
                        progress.speed_kbps
                    );
                }
            )
            .await;
            
        assert!(result.is_ok());
        assert!(file_path.exists());
    }
} 