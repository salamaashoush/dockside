//! Robust kubeconfig reader/writer.
//!
//! Mirrors how `kubectl`/Lens handle configs: the read set is the full
//! `$KUBECONFIG` list (or the settings override, else `~/.kube/config`,
//! else known distro paths). Each context/cluster/user remembers the
//! file it lives in; edits and deletes rewrite that file, new entries go
//! to the first file (the "primary"). Writes are atomic (temp file in
//! the same dir + rename) with a one-time `<file>.dockside.bak` backup,
//! and unknown YAML keys are preserved because we mutate a
//! `serde_yaml::Value` tree rather than the typed `kube` struct.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use serde_yaml::Value;

use super::diagnostics::first_existing_known_kubeconfig;

/// One context as shown in the Clusters view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextEntry {
  pub name: String,
  pub cluster: String,
  pub user: String,
  pub namespace: Option<String>,
  pub server: String,
  pub is_current: bool,
  /// Absolute path of the kubeconfig file this context is defined in.
  pub origin: String,
}

/// Auth method for a manually added cluster.
#[derive(Debug, Clone)]
pub enum AuthMethod {
  /// Bearer token.
  Token(String),
  /// Client certificate (PEM) + key (PEM).
  ClientCert { cert_pem: String, key_pem: String },
  /// Exec credential plugin (aws-iam-authenticator, gke-gcloud-auth-plugin…).
  Exec {
    command: String,
    args: Vec<String>,
    env: Vec<(String, String)>,
  },
}

/// Fields for the manual "Add cluster" form.
#[derive(Debug, Clone)]
pub struct NewCluster {
  pub context_name: String,
  pub server: String,
  /// PEM CA bundle (optional). Ignored when `insecure`.
  pub ca_pem: Option<String>,
  pub insecure: bool,
  pub namespace: Option<String>,
  pub auth: AuthMethod,
}

/// Ordered set of kubeconfig files. `files[0]` is the write target for
/// new entries and `current-context`.
pub struct Kubeconfigs {
  files: Vec<PathBuf>,
}

impl Kubeconfigs {
  /// Discover the active kubeconfig file set with `kubectl` precedence.
  #[must_use]
  pub fn discover() -> Self {
    let settings_path = crate::state::AppSettings::load().kubeconfig_path;
    if !settings_path.is_empty() {
      return Self {
        files: vec![PathBuf::from(settings_path)],
      };
    }

    let mut files: Vec<PathBuf> = Vec::new();
    if let Ok(env) = std::env::var("KUBECONFIG") {
      let sep = if cfg!(windows) { ';' } else { ':' };
      for p in env.split(sep).filter(|s| !s.is_empty()) {
        files.push(PathBuf::from(p));
      }
    }
    if files.is_empty()
      && let Some(home) = dirs::home_dir()
    {
      files.push(home.join(".kube").join("config"));
    }
    // Append known distro paths (k3s/kubeadm/microk8s) so a fresh box
    // with only /etc/rancher/k3s/k3s.yaml still lists something. The
    // first entry stays the primary write target.
    if let Some(distro) = first_existing_known_kubeconfig()
      && !files.contains(&distro)
    {
      files.push(distro);
    }
    Self { files }
  }

  /// File new entries + `current-context` are written to.
  #[must_use]
  pub fn primary(&self) -> PathBuf {
    self
      .files
      .first()
      .cloned()
      .unwrap_or_else(|| dirs::home_dir().map_or_else(|| PathBuf::from("config"), |h| h.join(".kube").join("config")))
  }

  fn existing(&self) -> Vec<PathBuf> {
    self.files.iter().filter(|p| p.exists()).cloned().collect()
  }

  /// Merged context list (with origin file + resolved server) and the
  /// effective `current-context` (first file that declares one wins).
  #[must_use]
  pub fn list(&self) -> (Vec<ContextEntry>, Option<String>) {
    let mut server_by_cluster: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    let mut current: Option<String> = None;
    let mut docs: Vec<(PathBuf, Value)> = Vec::new();

    for path in self.existing() {
      let doc = match load_doc(&path) {
        Ok(d) => d,
        Err(e) => {
          tracing::warn!("Skipping unreadable kubeconfig {}: {e}", path.display());
          continue;
        }
      };
      for c in seq(&doc, "clusters") {
        let Some(name) = c.get("name").and_then(Value::as_str) else {
          continue;
        };
        let server = c
          .get("cluster")
          .and_then(|m| m.get("server"))
          .and_then(Value::as_str)
          .unwrap_or_default()
          .to_string();
        server_by_cluster.entry(name.to_string()).or_insert(server);
      }
      if current.is_none()
        && let Some(cc) = doc.get("current-context").and_then(Value::as_str)
        && !cc.is_empty()
      {
        current = Some(cc.to_string());
      }
      docs.push((path, doc));
    }

    let mut out: Vec<ContextEntry> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    for (path, doc) in &docs {
      for c in seq(doc, "contexts") {
        let Some(name) = c.get("name").and_then(Value::as_str) else {
          continue;
        };
        if !seen.insert(name.to_string()) {
          continue; // first file wins, like kubectl merge
        }
        let ctx = c.get("context");
        let cluster = ctx
          .and_then(|m| m.get("cluster"))
          .and_then(Value::as_str)
          .unwrap_or_default()
          .to_string();
        let user = ctx
          .and_then(|m| m.get("user"))
          .and_then(Value::as_str)
          .unwrap_or_default()
          .to_string();
        let namespace = ctx
          .and_then(|m| m.get("namespace"))
          .and_then(Value::as_str)
          .map(ToString::to_string);
        out.push(ContextEntry {
          name: name.to_string(),
          server: server_by_cluster.get(&cluster).cloned().unwrap_or_default(),
          cluster,
          user,
          namespace,
          is_current: current.as_deref() == Some(name),
          origin: path.to_string_lossy().into_owned(),
        });
      }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    (out, current)
  }

  /// Path of the file that defines `ctx`, else the primary.
  fn origin_of(&self, ctx: &str) -> PathBuf {
    for path in self.existing() {
      if let Ok(doc) = load_doc(&path)
        && seq(&doc, "contexts")
          .iter()
          .any(|c| c.get("name").and_then(Value::as_str) == Some(ctx))
      {
        return path;
      }
    }
    self.primary()
  }

  /// Set `current-context` in the primary file.
  pub fn set_current(&self, ctx: &str) -> Result<()> {
    let path = self.primary();
    let mut doc = load_doc(&path)?;
    set_key(&mut doc, "current-context", Value::String(ctx.to_string()));
    save_doc(&path, &doc)
  }

  /// Delete a context (only the context stanza, like `kubectl
  /// config delete-context`). Clears `current-context` if it pointed here.
  pub fn remove_context(&self, ctx: &str) -> Result<()> {
    let path = self.origin_of(ctx);
    let mut doc = load_doc(&path)?;
    remove_named(&mut doc, "contexts", ctx);
    if doc.get("current-context").and_then(Value::as_str) == Some(ctx) {
      set_key(&mut doc, "current-context", Value::String(String::new()));
    }
    save_doc(&path, &doc)
  }

  /// Rename and/or repoint a context's namespace/user/cluster.
  pub fn edit_context(
    &self,
    old_name: &str,
    new_name: &str,
    namespace: Option<&str>,
    user: Option<&str>,
    cluster: Option<&str>,
  ) -> Result<()> {
    let path = self.origin_of(old_name);
    let mut doc = load_doc(&path)?;
    {
      let list = seq_mut(&mut doc, "contexts");
      let entry = list
        .iter_mut()
        .find(|c| c.get("name").and_then(Value::as_str) == Some(old_name))
        .ok_or_else(|| anyhow!("context '{old_name}' not found in {}", path.display()))?;
      if let Value::Mapping(m) = entry {
        m.insert(Value::String("name".into()), Value::String(new_name.to_string()));
        let inner = m
          .entry(Value::String("context".into()))
          .or_insert_with(|| Value::Mapping(serde_yaml::Mapping::new()));
        if let Value::Mapping(cm) = inner {
          if let Some(ns) = namespace {
            cm.insert(Value::String("namespace".into()), Value::String(ns.to_string()));
          }
          if let Some(u) = user {
            cm.insert(Value::String("user".into()), Value::String(u.to_string()));
          }
          if let Some(cl) = cluster {
            cm.insert(Value::String("cluster".into()), Value::String(cl.to_string()));
          }
        }
      }
    }
    if doc.get("current-context").and_then(Value::as_str) == Some(old_name) {
      set_key(&mut doc, "current-context", Value::String(new_name.to_string()));
    }
    save_doc(&path, &doc)
  }

  /// Add a cluster/user/context built from the manual form to the primary
  /// file (creating it if absent).
  pub fn add(&self, n: &NewCluster) -> Result<()> {
    if n.context_name.trim().is_empty() {
      return Err(anyhow!("context name is required"));
    }
    if n.server.trim().is_empty() {
      return Err(anyhow!("server URL is required"));
    }
    let path = self.primary();
    let mut doc = load_doc(&path)?;

    let cluster_name = n.context_name.clone();
    let user_name = format!("{}-user", n.context_name);

    let mut cluster_map = serde_yaml::Mapping::new();
    cluster_map.insert(Value::String("server".into()), Value::String(n.server.clone()));
    if n.insecure {
      cluster_map.insert(Value::String("insecure-skip-tls-verify".into()), Value::Bool(true));
    } else if let Some(ca) = &n.ca_pem
      && !ca.trim().is_empty()
    {
      use base64::Engine;
      let b64 = base64::engine::general_purpose::STANDARD.encode(ca.as_bytes());
      cluster_map.insert(Value::String("certificate-authority-data".into()), Value::String(b64));
    }
    upsert_named(
      &mut doc,
      "clusters",
      &cluster_name,
      "cluster",
      Value::Mapping(cluster_map),
    );

    let user_map = build_user(&n.auth);
    upsert_named(&mut doc, "users", &user_name, "user", user_map);

    let mut ctx_map = serde_yaml::Mapping::new();
    ctx_map.insert(Value::String("cluster".into()), Value::String(cluster_name));
    ctx_map.insert(Value::String("user".into()), Value::String(user_name));
    if let Some(ns) = &n.namespace
      && !ns.trim().is_empty()
    {
      ctx_map.insert(Value::String("namespace".into()), Value::String(ns.clone()));
    }
    upsert_named(
      &mut doc,
      "contexts",
      &n.context_name,
      "context",
      Value::Mapping(ctx_map),
    );

    ensure_header(&mut doc);
    save_doc(&path, &doc)
  }

  /// Merge every cluster/user/context from `src` into the primary file
  /// (overwriting same-named entries). Returns the context count merged.
  pub fn import_file(&self, src: &Path) -> Result<usize> {
    let src_doc = load_doc(src).with_context(|| format!("reading {}", src.display()))?;
    let dst_path = self.primary();
    let mut dst = load_doc(&dst_path)?;

    for kind in ["clusters", "users", "contexts"] {
      let inner = match kind {
        "clusters" => "cluster",
        "users" => "user",
        _ => "context",
      };
      for item in seq(&src_doc, kind) {
        let Some(name) = item.get("name").and_then(Value::as_str) else {
          continue;
        };
        let payload = item
          .get(inner)
          .cloned()
          .unwrap_or(Value::Mapping(serde_yaml::Mapping::new()));
        upsert_named(&mut dst, kind, name, inner, payload);
      }
    }
    let count = seq(&src_doc, "contexts").len();
    ensure_header(&mut dst);
    save_doc(&dst_path, &dst)?;
    Ok(count)
  }
}

fn build_user(auth: &AuthMethod) -> Value {
  let mut m = serde_yaml::Mapping::new();
  match auth {
    AuthMethod::Token(t) => {
      m.insert(Value::String("token".into()), Value::String(t.clone()));
    }
    AuthMethod::ClientCert { cert_pem, key_pem } => {
      use base64::Engine;
      let enc = |s: &str| base64::engine::general_purpose::STANDARD.encode(s.as_bytes());
      m.insert(
        Value::String("client-certificate-data".into()),
        Value::String(enc(cert_pem)),
      );
      m.insert(Value::String("client-key-data".into()), Value::String(enc(key_pem)));
    }
    AuthMethod::Exec { command, args, env } => {
      let mut exec = serde_yaml::Mapping::new();
      exec.insert(
        Value::String("apiVersion".into()),
        Value::String("client.authentication.k8s.io/v1beta1".into()),
      );
      exec.insert(Value::String("command".into()), Value::String(command.clone()));
      if !args.is_empty() {
        exec.insert(
          Value::String("args".into()),
          Value::Sequence(args.iter().cloned().map(Value::String).collect()),
        );
      }
      if !env.is_empty() {
        let seq = env
          .iter()
          .map(|(k, v)| {
            let mut em = serde_yaml::Mapping::new();
            em.insert(Value::String("name".into()), Value::String(k.clone()));
            em.insert(Value::String("value".into()), Value::String(v.clone()));
            Value::Mapping(em)
          })
          .collect();
        exec.insert(Value::String("env".into()), Value::Sequence(seq));
      }
      m.insert(Value::String("exec".into()), Value::Mapping(exec));
    }
  }
  Value::Mapping(m)
}

// ---- serde_yaml::Value helpers (preserve unknown keys) ----------------------

fn load_doc(path: &Path) -> Result<Value> {
  if !path.exists() {
    let mut m = serde_yaml::Mapping::new();
    m.insert(Value::String("apiVersion".into()), Value::String("v1".into()));
    m.insert(Value::String("kind".into()), Value::String("Config".into()));
    m.insert(Value::String("clusters".into()), Value::Sequence(vec![]));
    m.insert(Value::String("contexts".into()), Value::Sequence(vec![]));
    m.insert(Value::String("users".into()), Value::Sequence(vec![]));
    return Ok(Value::Mapping(m));
  }
  let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
  let val: Value = serde_yaml::from_str(&text).with_context(|| format!("parsing {} as YAML", path.display()))?;
  if !val.is_mapping() {
    return Err(anyhow!("{} is not a kubeconfig mapping", path.display()));
  }
  Ok(val)
}

/// Backup once, then write atomically via a temp file in the same dir.
fn save_doc(path: &Path, doc: &Value) -> Result<()> {
  if let Some(dir) = path.parent()
    && !dir.as_os_str().is_empty()
  {
    std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
  }
  if path.exists() {
    let bak = path.with_extension("dockside.bak");
    if !bak.exists() {
      std::fs::copy(path, &bak).with_context(|| format!("backing up to {}", bak.display()))?;
    }
  }
  let yaml = serde_yaml::to_string(doc).context("serializing kubeconfig")?;
  let fname = path
    .file_name()
    .map_or_else(|| "config".to_string(), |f| f.to_string_lossy().into_owned());
  let tmp = path.with_file_name(format!(".{fname}.dockside.tmp"));
  std::fs::write(&tmp, yaml).with_context(|| format!("writing {}", tmp.display()))?;
  std::fs::rename(&tmp, path).with_context(|| format!("replacing {}", path.display()))?;
  Ok(())
}

fn seq<'a>(doc: &'a Value, key: &str) -> Vec<&'a Value> {
  doc
    .get(key)
    .and_then(Value::as_sequence)
    .map(|s| s.iter().collect())
    .unwrap_or_default()
}

fn seq_mut<'a>(doc: &'a mut Value, key: &str) -> &'a mut Vec<Value> {
  let map = doc.as_mapping_mut().expect("kubeconfig root is a mapping");
  let entry = map
    .entry(Value::String(key.to_string()))
    .or_insert_with(|| Value::Sequence(vec![]));
  if !entry.is_sequence() {
    *entry = Value::Sequence(vec![]);
  }
  entry.as_sequence_mut().unwrap()
}

fn set_key(doc: &mut Value, key: &str, val: Value) {
  if let Some(m) = doc.as_mapping_mut() {
    m.insert(Value::String(key.to_string()), val);
  }
}

fn remove_named(doc: &mut Value, kind: &str, name: &str) {
  let list = seq_mut(doc, kind);
  list.retain(|c| c.get("name").and_then(Value::as_str) != Some(name));
}

/// Insert-or-replace a `{ name, <inner>: payload }` entry in `kind`.
fn upsert_named(doc: &mut Value, kind: &str, name: &str, inner: &str, payload: Value) {
  let list = seq_mut(doc, kind);
  let mut entry = serde_yaml::Mapping::new();
  entry.insert(Value::String("name".into()), Value::String(name.to_string()));
  entry.insert(Value::String(inner.to_string()), payload);
  if let Some(slot) = list
    .iter_mut()
    .find(|c| c.get("name").and_then(Value::as_str) == Some(name))
  {
    *slot = Value::Mapping(entry);
  } else {
    list.push(Value::Mapping(entry));
  }
}

fn ensure_header(doc: &mut Value) {
  if let Some(m) = doc.as_mapping_mut() {
    m.entry(Value::String("apiVersion".into()))
      .or_insert_with(|| Value::String("v1".into()));
    m.entry(Value::String("kind".into()))
      .or_insert_with(|| Value::String("Config".into()));
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn write(dir: &Path, name: &str, body: &str) -> PathBuf {
    let p = dir.join(name);
    std::fs::write(&p, body).unwrap();
    p
  }

  const SAMPLE: &str = r"
apiVersion: v1
kind: Config
current-context: alpha
clusters:
- name: alpha-cluster
  cluster:
    server: https://alpha:6443
contexts:
- name: alpha
  context:
    cluster: alpha-cluster
    user: alpha-user
    namespace: dev
users:
- name: alpha-user
  user:
    token: abc
custom-vendor-key: keep-me
";

  #[test]
  fn lists_with_origin_and_server() {
    let dir = std::env::temp_dir().join(format!("kc-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let f = write(&dir, "config", SAMPLE);
    let kc = Kubeconfigs { files: vec![f.clone()] };
    let (ctxs, current) = kc.list();
    assert_eq!(current.as_deref(), Some("alpha"));
    assert_eq!(ctxs.len(), 1);
    assert_eq!(ctxs[0].name, "alpha");
    assert_eq!(ctxs[0].server, "https://alpha:6443");
    assert_eq!(ctxs[0].namespace.as_deref(), Some("dev"));
    assert_eq!(ctxs[0].origin, f.to_string_lossy());
    std::fs::remove_dir_all(&dir).ok();
  }

  #[test]
  fn edit_remove_preserve_unknown_keys() {
    let dir = std::env::temp_dir().join(format!("kc-test2-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let f = write(&dir, "config", SAMPLE);
    let kc = Kubeconfigs { files: vec![f.clone()] };

    kc.edit_context("alpha", "alpha", Some("prod"), None, None).unwrap();
    let body = std::fs::read_to_string(&f).unwrap();
    assert!(body.contains("custom-vendor-key"), "unknown keys must survive");
    assert!(body.contains("prod"));

    kc.remove_context("alpha").unwrap();
    let (ctxs, _) = kc.list();
    assert!(ctxs.is_empty());
    assert!(f.with_extension("dockside.bak").exists(), "backup written");
    std::fs::remove_dir_all(&dir).ok();
  }

  #[test]
  fn add_and_import() {
    let dir = std::env::temp_dir().join(format!("kc-test3-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("config");
    let kc = Kubeconfigs { files: vec![f.clone()] };
    kc.add(&NewCluster {
      context_name: "edge".into(),
      server: "https://edge:6443".into(),
      ca_pem: None,
      insecure: true,
      namespace: Some("ns".into()),
      auth: AuthMethod::Token("tok".into()),
    })
    .unwrap();
    let (ctxs, _) = kc.list();
    assert_eq!(ctxs.len(), 1);
    assert_eq!(ctxs[0].name, "edge");
    assert_eq!(ctxs[0].server, "https://edge:6443");

    let src = write(&dir, "other", SAMPLE);
    let n = kc.import_file(&src).unwrap();
    assert_eq!(n, 1);
    let (ctxs, _) = kc.list();
    assert_eq!(ctxs.len(), 2);
    std::fs::remove_dir_all(&dir).ok();
  }
}
