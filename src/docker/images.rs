use anyhow::Result;
use bollard::image::{CreateImageOptions, ListImagesOptions, RemoveImageOptions};
use chrono::{DateTime, Utc};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::DockerClient;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageInfo {
    pub id: String,
    pub repo_tags: Vec<String>,
    pub repo_digests: Vec<String>,
    pub created: Option<DateTime<Utc>>,
    pub size: i64,
    pub virtual_size: Option<i64>,
    pub labels: HashMap<String, String>,
    pub architecture: Option<String>,
    pub os: Option<String>,
}

impl ImageInfo {
    pub fn short_id(&self) -> &str {
        let id = self.id.strip_prefix("sha256:").unwrap_or(&self.id);
        if id.len() >= 12 {
            &id[..12]
        } else {
            id
        }
    }

    pub fn display_name(&self) -> String {
        self.repo_tags
            .first()
            .cloned()
            .unwrap_or_else(|| self.short_id().to_string())
    }

    pub fn display_size(&self) -> String {
        bytesize::ByteSize(self.size as u64).to_string()
    }

    pub fn is_dangling(&self) -> bool {
        self.repo_tags.is_empty()
            || self.repo_tags.iter().all(|t| t == "<none>:<none>")
    }
}

impl DockerClient {
    pub async fn list_images(&self, all: bool) -> Result<Vec<ImageInfo>> {
        let docker = self.client()?;

        let options = ListImagesOptions::<String> {
            all,
            ..Default::default()
        };

        let images = docker.list_images(Some(options)).await?;

        let mut result = Vec::new();
        for image in images {
            let created = DateTime::from_timestamp(image.created, 0);

            result.push(ImageInfo {
                id: image.id,
                repo_tags: image.repo_tags,
                repo_digests: image.repo_digests,
                created,
                size: image.size,
                virtual_size: image.virtual_size,
                labels: image.labels,
                architecture: None,
                os: None,
            });
        }

        result.sort_by(|a, b| b.created.cmp(&a.created));
        Ok(result)
    }

    pub async fn remove_image(&self, id: &str, force: bool) -> Result<()> {
        let docker = self.client()?;
        docker
            .remove_image(
                id,
                Some(RemoveImageOptions {
                    force,
                    noprune: false,
                }),
                None,
            )
            .await?;
        Ok(())
    }

    pub async fn image_inspect(&self, id: &str) -> Result<ImageInfo> {
        let docker = self.client()?;
        let inspect = docker.inspect_image(id).await?;

        let created = inspect
            .created
            .as_ref()
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc));

        Ok(ImageInfo {
            id: inspect.id.unwrap_or_default(),
            repo_tags: inspect.repo_tags.unwrap_or_default(),
            repo_digests: inspect.repo_digests.unwrap_or_default(),
            created,
            size: inspect.size.unwrap_or(0),
            virtual_size: inspect.virtual_size,
            labels: inspect
                .config
                .and_then(|c| c.labels)
                .unwrap_or_default(),
            architecture: inspect.architecture,
            os: inspect.os,
        })
    }

    /// Check if an image exists locally
    pub async fn image_exists(&self, image: &str) -> bool {
        self.client()
            .map(|docker| docker.inspect_image(image))
            .ok()
            .map(|fut| async move { fut.await.is_ok() })
            .is_some()
            && self
                .client()
                .ok()
                .map(|docker| async move { docker.inspect_image(image).await.is_ok() })
                .is_some()
    }

    /// Pull an image from a registry
    pub async fn pull_image(&self, image: &str, platform: Option<&str>) -> Result<()> {
        let docker = self.client()?;

        // Parse image name into repository and tag
        let (repo, tag) = if let Some(pos) = image.rfind(':') {
            // Check if this is a port number (e.g., localhost:5000/image)
            let after_colon = &image[pos + 1..];
            if after_colon.contains('/') || after_colon.parse::<u16>().is_ok() {
                // It's a port, not a tag
                (image, "latest")
            } else {
                (&image[..pos], after_colon)
            }
        } else {
            (image, "latest")
        };

        let options = CreateImageOptions {
            from_image: repo,
            tag,
            platform: platform.unwrap_or_default(),
            ..Default::default()
        };

        let mut stream = docker.create_image(Some(options), None, None);

        // Consume the stream to completion
        while let Some(result) = stream.next().await {
            match result {
                Ok(_info) => {
                    // Progress info available in _info.status, _info.progress, etc.
                }
                Err(e) => {
                    return Err(anyhow::anyhow!("Failed to pull image: {}", e));
                }
            }
        }

        Ok(())
    }

    /// Ensure an image exists locally, pulling it if necessary
    pub async fn ensure_image(&self, image: &str, platform: Option<&str>) -> Result<()> {
        let docker = self.client()?;

        // Try to inspect the image to see if it exists
        match docker.inspect_image(image).await {
            Ok(_) => Ok(()), // Image exists
            Err(_) => {
                // Image doesn't exist, pull it
                self.pull_image(image, platform).await
            }
        }
    }
}
