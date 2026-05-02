use anyhow::Result;
use bollard::auth::DockerCredentials;
use bollard::query_parameters::{
  BuildImageOptionsBuilder, CreateImageOptions, ImportImageOptionsBuilder, ListImagesOptions, PushImageOptionsBuilder,
  RemoveImageOptions, SearchImagesOptionsBuilder, TagImageOptionsBuilder,
};
use chrono::{DateTime, Utc};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

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

/// Match returned from a Docker Hub registry search.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RegistrySearchResult {
  pub name: String,
  pub description: String,
  pub stars: i64,
  pub official: bool,
  pub automated: bool,
}

/// One streamed event from `build_image_with_progress`. Either `stream`
/// (output line, e.g. "Step 3/8 : RUN apt-get install …"), `status`
/// (layer status), or `error` (build failure detail) will be populated.
#[derive(Debug, Clone, Default)]
pub struct BuildProgressEvent {
  pub stream: String,
  pub status: String,
  pub error: String,
}

/// One streamed event from `pull_image_with_progress`. Most events report
/// a layer id; the leading "Pulling from..." event has an empty id and the
/// status string in `status`.
#[derive(Debug, Clone, Default)]
pub struct PullProgressEvent {
  pub id: String,
  pub status: String,
  pub current: Option<i64>,
  pub total: Option<i64>,
}

fn format_push_event(info: &bollard::models::PushImageInfo) -> String {
  let id_part = match &info.progress_detail {
    Some(_) => String::new(),
    None => String::new(),
  };
  let status = info.status.clone().unwrap_or_default();
  let progress = info.progress.clone().unwrap_or_default();
  let mut out = status;
  if !progress.is_empty() {
    if !out.is_empty() {
      out.push(' ');
    }
    out.push_str(&progress);
  }
  if !id_part.is_empty() {
    if !out.is_empty() {
      out.push(' ');
    }
    out.push_str(&id_part);
  }
  out
}

/// One entry in `docker image history` output. `id == "<missing>"` when
/// Docker has dropped the original layer metadata for intermediate layers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageHistoryEntry {
  pub id: String,
  pub created: Option<DateTime<Utc>>,
  pub created_by: String,
  pub size: i64,
  pub comment: String,
  pub tags: Vec<String>,
}

impl ImageHistoryEntry {
  pub fn display_size(&self) -> String {
    bytesize::ByteSize(u64::try_from(self.size).unwrap_or(0)).to_string()
  }

  /// Strip the "/bin/sh -c #(nop) " prefix Docker adds to non-RUN history
  /// entries so the table reads as actual Dockerfile-ish commands.
  pub fn short_command(&self) -> String {
    let nop = "/bin/sh -c #(nop) ";
    let shc = "/bin/sh -c ";
    if let Some(rest) = self.created_by.strip_prefix(nop) {
      rest.trim().to_string()
    } else if let Some(rest) = self.created_by.strip_prefix(shc) {
      format!("RUN {}", rest.trim())
    } else {
      self.created_by.trim().to_string()
    }
  }
}

impl ImageInfo {
  pub fn short_id(&self) -> &str {
    let id = self.id.strip_prefix("sha256:").unwrap_or(&self.id);
    if id.len() >= 12 { &id[..12] } else { id }
  }

  pub fn display_name(&self) -> String {
    self
      .repo_tags
      .first()
      .cloned()
      .unwrap_or_else(|| self.short_id().to_string())
  }

  pub fn display_size(&self) -> String {
    bytesize::ByteSize(u64::try_from(self.size).unwrap_or(0)).to_string()
  }
}

impl DockerClient {
  pub async fn list_images(&self, all: bool) -> Result<Vec<ImageInfo>> {
    let docker = self.client()?;

    let options = ListImagesOptions {
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
      .remove_image(id, Some(RemoveImageOptions { force, noprune: false }), None)
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
      labels: inspect.config.and_then(|c| c.labels).unwrap_or_default(),
      architecture: inspect.architecture,
      os: inspect.os,
    })
  }

  /// Fetch the image history (per-layer breakdown) for an image.
  pub async fn image_history(&self, id: &str) -> Result<Vec<ImageHistoryEntry>> {
    let docker = self.client()?;
    let history = docker.image_history(id).await?;
    Ok(
      history
        .into_iter()
        .map(|h| ImageHistoryEntry {
          id: h.id,
          created: DateTime::from_timestamp(h.created, 0),
          created_by: h.created_by,
          size: h.size,
          comment: h.comment,
          tags: h.tags,
        })
        .collect(),
    )
  }

  /// Search Docker Hub via the daemon's `/images/search` endpoint.
  /// Returns up to `limit` matches sorted by stars descending.
  pub async fn search_images(&self, query: &str, limit: usize) -> Result<Vec<RegistrySearchResult>> {
    let docker = self.client()?;
    let opts = SearchImagesOptionsBuilder::default()
      .term(query)
      .limit(i32::try_from(limit).unwrap_or(25))
      .build();
    let resp = docker.search_images(opts).await?;
    Ok(
      resp
        .into_iter()
        .map(|r| RegistrySearchResult {
          name: r.name.unwrap_or_default(),
          description: r.description.unwrap_or_default(),
          stars: r.star_count.unwrap_or(0),
          official: r.is_official.unwrap_or(false),
          automated: r.is_automated.unwrap_or(false),
        })
        .collect(),
    )
  }

  /// Build an image from a context directory + Dockerfile. The context
  /// is tar-bundled in the background, then the bollard build stream is
  /// driven and each event is delivered to `on_progress`.
  ///
  /// `context_dir` is the build root (everything under it is shipped).
  /// `dockerfile` is relative to the context (e.g. `Dockerfile`).
  pub async fn build_image_with_progress<F>(
    &self,
    context_dir: &Path,
    dockerfile: &str,
    tag: &str,
    build_args: Vec<(String, String)>,
    target: Option<&str>,
    platform: Option<&str>,
    no_cache: bool,
    pull: bool,
    mut on_progress: F,
  ) -> Result<()>
  where
    F: FnMut(BuildProgressEvent) + Send,
  {
    let docker = self.client()?;

    // Tar the build context. Bollard expects the request body to be a
    // tarball; build it in-memory (fine for typical small contexts).
    let context_dir = context_dir.to_path_buf();
    let tar_bytes = tokio::task::spawn_blocking(move || -> Result<Vec<u8>> {
      let mut buf: Vec<u8> = Vec::new();
      {
        let mut builder = tar::Builder::new(&mut buf);
        builder.follow_symlinks(false);
        builder
          .append_dir_all(".", &context_dir)
          .map_err(|e| anyhow::anyhow!("tar build failed: {e}"))?;
        builder.finish().map_err(|e| anyhow::anyhow!("tar finalize failed: {e}"))?;
      }
      Ok(buf)
    })
    .await
    .map_err(|e| anyhow::anyhow!("tar task panicked: {e}"))??;

    let pull_str = if pull { "true" } else { "" };
    let mut builder = BuildImageOptionsBuilder::default()
      .dockerfile(dockerfile)
      .t(tag)
      .nocache(no_cache)
      .pull(pull_str)
      .rm(true);
    if let Some(t) = target {
      builder = builder.target(t);
    }
    if let Some(p) = platform {
      builder = builder.platform(p);
    }
    let build_args_owned: HashMap<String, String> = build_args.into_iter().collect();
    let build_args_ref: HashMap<&str, &str> = build_args_owned
      .iter()
      .map(|(k, v)| (k.as_str(), v.as_str()))
      .collect();
    if !build_args_ref.is_empty() {
      builder = builder.buildargs(&build_args_ref);
    }
    let opts = builder.build();

    let body = bollard::body_full(bytes::Bytes::from(tar_bytes));
    let mut stream = docker.build_image(opts, None, Some(body));

    while let Some(result) = stream.next().await {
      match result {
        Ok(info) => {
          let stream_text = info.stream.unwrap_or_default();
          let status = info.status.unwrap_or_default();
          let error = info.error.unwrap_or_default();
          on_progress(BuildProgressEvent {
            stream: stream_text,
            status,
            error,
          });
        }
        Err(e) => return Err(anyhow::anyhow!("build failed: {e}")),
      }
    }
    Ok(())
  }

  /// Tag an existing image as `repo:tag`. `repo` may include a registry
  /// host (e.g. `ghcr.io/foo/bar`) per docker tag rules.
  pub async fn tag_image(&self, source: &str, repo: &str, tag: &str) -> Result<()> {
    let docker = self.client()?;
    let opts = TagImageOptionsBuilder::default().repo(repo).tag(tag).build();
    docker.tag_image(source, Some(opts)).await?;
    Ok(())
  }

  /// Push `image_name` to its registry. Supports an optional username +
  /// password pair; pass `None` to use the daemon's stored credentials.
  /// Streams progress via the callback (one entry per layer event).
  pub async fn push_image_with_progress<F>(
    &self,
    image_name: &str,
    tag: &str,
    auth: Option<(String, String)>,
    mut on_progress: F,
  ) -> Result<()>
  where
    F: FnMut(String) + Send,
  {
    let docker = self.client()?;
    let opts = PushImageOptionsBuilder::default().tag(tag).build();
    let credentials = auth.map(|(username, password)| DockerCredentials {
      username: Some(username),
      password: Some(password),
      ..Default::default()
    });
    let mut stream = docker.push_image(image_name, Some(opts), credentials);
    while let Some(result) = stream.next().await {
      match result {
        Ok(info) => {
          let line = format_push_event(&info);
          if !line.is_empty() {
            on_progress(line);
          }
        }
        Err(e) => return Err(anyhow::anyhow!("Push failed: {e}")),
      }
    }
    Ok(())
  }

  /// Stream `docker save <image_ref>` bytes and write them to `dest`.
  /// `on_progress` is called as bytes accumulate so the UI can render a
  /// progress indicator.
  pub async fn save_image<F>(&self, image_ref: &str, dest: &Path, mut on_progress: F) -> Result<u64>
  where
    F: FnMut(u64) + Send,
  {
    use tokio::io::AsyncWriteExt;
    let docker = self.client()?;
    let mut stream = docker.export_image(image_ref);
    let mut file = tokio::fs::File::create(dest).await?;
    let mut total: u64 = 0;
    while let Some(chunk) = stream.next().await {
      let bytes = chunk.map_err(|e| anyhow::anyhow!("export_image: {e}"))?;
      file.write_all(&bytes).await?;
      total += bytes.len() as u64;
      on_progress(total);
    }
    file.flush().await?;
    Ok(total)
  }

  /// POST a tarball at `src` to `/images/load` and stream the daemon
  /// response. Each event is forwarded to `on_progress` (status string
  /// or final image id).
  pub async fn load_image<F>(&self, src: &Path, mut on_progress: F) -> Result<()>
  where
    F: FnMut(String) + Send,
  {
    let docker = self.client()?;
    let bytes = tokio::fs::read(src).await?;
    let opts = ImportImageOptionsBuilder::default().build();
    let body = bollard::body_full(bytes::Bytes::from(bytes));
    let mut stream = docker.import_image(opts, body, None);
    while let Some(result) = stream.next().await {
      match result {
        Ok(info) => {
          let mut line = info.stream.unwrap_or_default();
          if line.is_empty() {
            line = info.status.unwrap_or_default();
          }
          let line = line.trim();
          if !line.is_empty() {
            on_progress(line.to_string());
          }
        }
        Err(e) => return Err(anyhow::anyhow!("import_image: {e}")),
      }
    }
    Ok(())
  }

  /// Pull an image from a registry
  pub async fn pull_image(&self, image: &str, platform: Option<&str>) -> Result<()> {
    self
      .pull_image_with_progress(image, platform, |_| {})
      .await
  }

  /// Pull an image from a registry, calling `on_progress` for each
  /// streamed event. Each event reports a layer id, status, and current
  /// / total bytes (when available) so callers can render a progress UI.
  pub async fn pull_image_with_progress<F>(
    &self,
    image: &str,
    platform: Option<&str>,
    mut on_progress: F,
  ) -> Result<()>
  where
    F: FnMut(PullProgressEvent) + Send,
  {
    let docker = self.client()?;

    // Parse image name into repository and tag
    let (repo, tag) = if let Some(pos) = image.rfind(':') {
      let after_colon = &image[pos + 1..];
      if after_colon.contains('/') || after_colon.parse::<u16>().is_ok() {
        (image, "latest")
      } else {
        (&image[..pos], after_colon)
      }
    } else {
      (image, "latest")
    };

    let options = CreateImageOptions {
      from_image: Some(repo.to_string()),
      tag: Some(tag.to_string()),
      platform: platform.map(String::from).unwrap_or_default(),
      ..Default::default()
    };

    let mut stream = docker.create_image(Some(options), None, None);
    while let Some(result) = stream.next().await {
      match result {
        Ok(info) => {
          let detail = info.progress_detail.unwrap_or_default();
          on_progress(PullProgressEvent {
            id: info.id.unwrap_or_default(),
            status: info.status.unwrap_or_default(),
            current: detail.current,
            total: detail.total,
          });
        }
        Err(e) => {
          return Err(anyhow::anyhow!("Failed to pull image: {e}"));
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

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_image_info_short_id() {
    // With sha256 prefix
    let image = ImageInfo {
      id: "sha256:abc123def456789012345678901234567890".to_string(),
      repo_tags: vec![],
      repo_digests: vec![],
      created: None,
      size: 0,
      virtual_size: None,
      labels: HashMap::new(),
      architecture: None,
      os: None,
    };
    assert_eq!(image.short_id(), "abc123def456");

    // Without sha256 prefix
    let image2 = ImageInfo {
      id: "xyz789012345678901234567890".to_string(),
      ..image.clone()
    };
    assert_eq!(image2.short_id(), "xyz789012345");

    // Short id
    let short = ImageInfo {
      id: "sha256:abc".to_string(),
      ..image.clone()
    };
    assert_eq!(short.short_id(), "abc");
  }

  #[test]
  fn test_image_info_display_name() {
    // With tag
    let image = ImageInfo {
      id: "sha256:abc123".to_string(),
      repo_tags: vec!["nginx:latest".to_string(), "nginx:1.25".to_string()],
      repo_digests: vec![],
      created: None,
      size: 0,
      virtual_size: None,
      labels: HashMap::new(),
      architecture: None,
      os: None,
    };
    assert_eq!(image.display_name(), "nginx:latest");

    // Without tag (falls back to short id)
    let no_tags = ImageInfo {
      repo_tags: vec![],
      ..image.clone()
    };
    assert_eq!(no_tags.display_name(), "abc123");
  }

  #[test]
  fn test_image_info_display_size() {
    let image = ImageInfo {
      id: "sha256:abc".to_string(),
      repo_tags: vec![],
      repo_digests: vec![],
      created: None,
      size: 1024 * 1024 * 150, // 150 MiB
      virtual_size: None,
      labels: HashMap::new(),
      architecture: None,
      os: None,
    };
    assert_eq!(image.display_size(), "150.0 MiB");

    let small = ImageInfo {
      size: 1024 * 50, // 50 KiB
      ..image.clone()
    };
    assert_eq!(small.display_size(), "50.0 KiB");

    let zero = ImageInfo {
      size: 0,
      ..image.clone()
    };
    assert_eq!(zero.display_size(), "0 B");
  }

  #[test]
  fn test_image_info_with_metadata() {
    let image = ImageInfo {
      id: "sha256:abc123".to_string(),
      repo_tags: vec!["alpine:3.18".to_string()],
      repo_digests: vec!["alpine@sha256:abc...".to_string()],
      created: None,
      size: 5 * 1024 * 1024, // 5 MiB
      virtual_size: Some(10 * 1024 * 1024),
      labels: HashMap::from([("maintainer".to_string(), "test@example.com".to_string())]),
      architecture: Some("amd64".to_string()),
      os: Some("linux".to_string()),
    };

    assert_eq!(image.display_name(), "alpine:3.18");
    assert_eq!(image.display_size(), "5.0 MiB");
    assert_eq!(image.architecture, Some("amd64".to_string()));
    assert_eq!(image.os, Some("linux".to_string()));
    assert_eq!(image.labels.get("maintainer"), Some(&"test@example.com".to_string()));
  }
}
