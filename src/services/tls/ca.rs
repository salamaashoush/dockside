//! Local Certificate Authority for `*.dockside.test`.
//!
//! On first launch we generate an Ed25519/ECDSA root CA and persist it under
//! `~/.config/dockside/ca/` (mode 0600 on private key). The root CA is
//! installed once into the OS trust store via the privileged
//! `dockside-helper` binary; from then on we mint short-lived leaf
//! certificates in-memory for every SNI host the proxy sees.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use rcgen::{
  BasicConstraints, Certificate, CertificateParams, DistinguishedName, DnType, ExtendedKeyUsagePurpose, IsCa, KeyPair,
  KeyUsagePurpose,
};

const CA_DIR: &str = "ca";
const CA_CERT_FILE: &str = "root.crt";
const CA_KEY_FILE: &str = "root.key";

pub struct LocalCa {
  pub cert: Certificate,
  pub key: KeyPair,
  /// Directory under which `root.crt` and `root.key` live. Surfaced so the
  /// settings UI can show the file path and pass it to the helper binary.
  pub dir: PathBuf,
}

impl std::fmt::Debug for LocalCa {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("LocalCa")
      .field("dir", &self.dir)
      .finish_non_exhaustive()
  }
}

impl LocalCa {
  /// Load the local CA from disk, generating it on first run.
  pub fn load_or_create() -> Result<Self> {
    let dir = ca_directory()?;
    std::fs::create_dir_all(&dir).with_context(|| format!("create CA dir {}", dir.display()))?;
    let cert_path = dir.join(CA_CERT_FILE);
    let key_path = dir.join(CA_KEY_FILE);

    if cert_path.is_file() && key_path.is_file() {
      let cert_pem = std::fs::read_to_string(&cert_path).with_context(|| format!("read {}", cert_path.display()))?;
      let key_pem = std::fs::read_to_string(&key_path).with_context(|| format!("read {}", key_path.display()))?;
      let key = KeyPair::from_pem(&key_pem).context("parse CA key PEM")?;
      let params = CertificateParams::from_ca_cert_pem(&cert_pem).context("parse CA cert PEM")?;
      let cert = params.self_signed(&key).context("re-sign loaded CA")?;
      return Ok(Self { cert, key, dir });
    }

    // First-launch: mint fresh CA.
    let mut params = CertificateParams::default();
    params.is_ca = IsCa::Ca(BasicConstraints::Constrained(0));
    params.key_usages = vec![KeyUsagePurpose::KeyCertSign, KeyUsagePurpose::CrlSign];
    params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
    let mut dn = DistinguishedName::new();
    dn.push(DnType::CommonName, "Dockside Local Root CA");
    dn.push(DnType::OrganizationName, "Dockside");
    params.distinguished_name = dn;
    let (before, after) = ca_validity();
    params.not_before = before;
    params.not_after = after;

    let key = KeyPair::generate().context("generate CA key")?;
    let cert = params.self_signed(&key).context("self-sign CA cert")?;
    let cert_pem = cert.pem();
    let key_pem = key.serialize_pem();

    write_secret_file(&cert_path, cert_pem.as_bytes())?;
    write_secret_file(&key_path, key_pem.as_bytes())?;

    Ok(Self { cert, key, dir })
  }

  /// Mint a short-lived leaf certificate covering the given DNS name. The
  /// returned PEM strings are suitable for `rustls`'s `CertifiedKey::from_der`
  /// flow via `rustls_pemfile`.
  pub fn issue_leaf(&self, dns_name: &str) -> Result<LeafKey> {
    let mut params = CertificateParams::new(vec![dns_name.to_string()]).context("leaf params")?;
    params.is_ca = IsCa::ExplicitNoCa;
    params.key_usages = vec![KeyUsagePurpose::DigitalSignature, KeyUsagePurpose::KeyEncipherment];
    params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
    let mut dn = DistinguishedName::new();
    dn.push(DnType::CommonName, dns_name);
    params.distinguished_name = dn;
    let (before, after) = leaf_validity();
    params.not_before = before;
    params.not_after = after;

    let key = KeyPair::generate().context("generate leaf key")?;
    let cert = params.signed_by(&key, &self.cert, &self.key).context("sign leaf")?;
    Ok(LeafKey {
      cert_pem: cert.pem(),
      key_pem: key.serialize_pem(),
    })
  }
}

pub struct LeafKey {
  pub cert_pem: String,
  pub key_pem: String,
}

fn ca_directory() -> Result<PathBuf> {
  let base = dirs::config_dir().ok_or_else(|| anyhow::anyhow!("no config dir"))?;
  Ok(base.join("dockside").join(CA_DIR))
}

fn write_secret_file(path: &Path, contents: &[u8]) -> Result<()> {
  use std::io::Write;
  // Always create with restrictive perms.
  let mut opts = std::fs::OpenOptions::new();
  opts.write(true).create(true).truncate(true);
  #[cfg(unix)]
  {
    use std::os::unix::fs::OpenOptionsExt;
    opts.mode(0o600);
  }
  let mut file = opts.open(path).with_context(|| format!("open {}", path.display()))?;
  file
    .write_all(contents)
    .with_context(|| format!("write {}", path.display()))?;
  Ok(())
}

fn ca_validity() -> (time::OffsetDateTime, time::OffsetDateTime) {
  let now = time::OffsetDateTime::now_utc();
  (now - time::Duration::minutes(1), now + time::Duration::days(10 * 365))
}

fn leaf_validity() -> (time::OffsetDateTime, time::OffsetDateTime) {
  let now = time::OffsetDateTime::now_utc();
  (now - time::Duration::minutes(1), now + time::Duration::days(90))
}
