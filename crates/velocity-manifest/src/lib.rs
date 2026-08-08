use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, fs, path::Path};
use thiserror::Error;

pub const MANIFEST_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackageManifest {
    pub schema: u32,
    pub name: String,
    pub version: Version,
    pub description: String,
    #[serde(default)]
    pub homepage: Option<String>,
    #[serde(default)]
    pub license: Option<String>,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub provides: Vec<String>,
    #[serde(default)]
    pub conflicts: Vec<String>,
    #[serde(default)]
    pub dependencies: Vec<Dependency>,
    #[serde(default)]
    pub artifacts: Vec<Artifact>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Dependency {
    pub name: String,
    #[serde(default = "any_version")]
    pub version: VersionReq,
    #[serde(default)]
    pub when: TargetPredicate,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TargetPredicate {
    #[serde(default)]
    pub targets: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Artifact {
    pub target: String,
    pub url: String,
    pub sha256: String,
    pub archive: String,
    #[serde(default)]
    pub strip_components: u32,
    #[serde(default)]
    pub binaries: Vec<BinaryMapping>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BinaryMapping {
    pub source: String,
    pub name: String,
}

#[derive(Debug, Error)]
pub enum ManifestError {
    #[error("failed to read {path}: {source}")]
    Read {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse {path}: {source}")]
    Parse {
        path: String,
        #[source]
        source: toml::de::Error,
    },
    #[error("invalid package manifest: {0}")]
    Invalid(String),
}

pub fn load_manifest(path: impl AsRef<Path>) -> Result<PackageManifest, ManifestError> {
    let path = path.as_ref();
    let text = fs::read_to_string(path).map_err(|source| ManifestError::Read {
        path: path.display().to_string(),
        source,
    })?;

    let manifest: PackageManifest =
        toml::from_str(&text).map_err(|source| ManifestError::Parse {
            path: path.display().to_string(),
            source,
        })?;

    manifest.validate()?;
    Ok(manifest)
}

impl PackageManifest {
    pub fn validate(&self) -> Result<(), ManifestError> {
        if self.schema != MANIFEST_SCHEMA_VERSION {
            return invalid(format!(
                "{}: unsupported schema {}, expected {}",
                self.name, self.schema, MANIFEST_SCHEMA_VERSION
            ));
        }
        validate_name(&self.name)?;
        if self.description.trim().is_empty() {
            return invalid(format!("{}: description must not be empty", self.name));
        }
        if let Some(homepage) = &self.homepage {
            validate_https_url(&self.name, "homepage", homepage)?;
        }

        let mut names = HashSet::new();
        names.insert(self.name.as_str());
        for alias in &self.aliases {
            validate_name(alias)?;
            if !names.insert(alias.as_str()) {
                return invalid(format!("{}: duplicate alias {alias}", self.name));
            }
        }

        for capability in &self.provides {
            validate_name(capability)?;
        }
        for conflict in &self.conflicts {
            validate_name(conflict)?;
        }

        for dep in &self.dependencies {
            validate_name(&dep.name)?;
            if dep.name == self.name || self.aliases.iter().any(|a| a == &dep.name) {
                return invalid(format!("{}: package may not depend on itself", self.name));
            }
            for target in &dep.when.targets {
                validate_target(target)?;
            }
        }

        if self.artifacts.is_empty() {
            return invalid(format!("{}: at least one artifact is required", self.name));
        }

        let mut targets = HashSet::new();
        for artifact in &self.artifacts {
            validate_target(&artifact.target)?;
            if !targets.insert(artifact.target.as_str()) {
                return invalid(format!(
                    "{}: duplicate artifact target {}",
                    self.name, artifact.target
                ));
            }
            validate_https_url(&self.name, "artifact URL", &artifact.url)?;
            validate_sha256(&self.name, &artifact.sha256)?;
            validate_archive(&self.name, &artifact.archive)?;

            let mut exposed_names = HashSet::new();
            for binary in &artifact.binaries {
                if binary.source.trim().is_empty() || binary.name.trim().is_empty() {
                    return invalid(format!(
                        "{}: binary source/name must not be empty",
                        self.name
                    ));
                }
                if !exposed_names.insert(binary.name.as_str()) {
                    return invalid(format!(
                        "{}: duplicate exposed binary {} for target {}",
                        self.name, binary.name, artifact.target
                    ));
                }
            }
        }

        Ok(())
    }
}

pub fn validate_name(name: &str) -> Result<(), ManifestError> {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return invalid("package name must not be empty".into());
    };
    if !first.is_ascii_lowercase() && !first.is_ascii_digit() {
        return invalid(format!(
            "invalid package name {name:?}: must start with lowercase ASCII or digit"
        ));
    }
    if !chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "+-._".contains(c)) {
        return invalid(format!(
            "invalid package name {name:?}: allowed characters are [a-z0-9+._-]"
        ));
    }
    Ok(())
}

pub fn validate_target(target: &str) -> Result<(), ManifestError> {
    const SUPPORTED: &[&str] = &[
        "x86_64-pc-windows-msvc",
        "aarch64-pc-windows-msvc",
        "x86_64-unknown-linux-gnu",
        "aarch64-unknown-linux-gnu",
        "x86_64-unknown-linux-musl",
        "aarch64-unknown-linux-musl",
    ];
    if SUPPORTED.contains(&target) {
        Ok(())
    } else {
        invalid(format!("unsupported target {target:?}"))
    }
}

fn validate_https_url(package: &str, field: &str, url: &str) -> Result<(), ManifestError> {
    if !url.starts_with("https://") || url.chars().any(char::is_whitespace) {
        return invalid(format!(
            "{package}: {field} must be a whitespace-free https:// URL"
        ));
    }
    Ok(())
}

fn validate_sha256(package: &str, hash: &str) -> Result<(), ManifestError> {
    if hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
        return invalid(format!(
            "{package}: sha256 must contain exactly 64 hexadecimal characters"
        ));
    }
    Ok(())
}

fn validate_archive(package: &str, archive: &str) -> Result<(), ManifestError> {
    const SUPPORTED: &[&str] = &["zip", "tar.gz", "tar.xz", "tar.zst", "raw"];
    if SUPPORTED.contains(&archive) {
        Ok(())
    } else {
        invalid(format!("{package}: unsupported archive format {archive:?}"))
    }
}

fn any_version() -> VersionReq {
    VersionReq::STAR
}

fn invalid<T>(message: String) -> Result<T, ManifestError> {
    Err(ManifestError::Invalid(message))
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID: &str = r#"
schema = 1
name = "demo"
version = "1.2.3"
description = "Demo package"
homepage = "https://example.com/demo"
aliases = ["demo-cli"]

[[dependencies]]
name = "runtime"
version = ">=1.0"

[[artifacts]]
target = "x86_64-pc-windows-msvc"
url = "https://example.com/demo.zip"
sha256 = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
archive = "zip"
strip_components = 1

[[artifacts.binaries]]
source = "demo.exe"
name = "demo"
"#;

    #[test]
    fn parses_and_validates_manifest() {
        let manifest: PackageManifest = toml::from_str(VALID).unwrap();
        manifest.validate().unwrap();
        assert_eq!(manifest.name, "demo");
    }

    #[test]
    fn rejects_bad_hash() {
        let mut manifest: PackageManifest = toml::from_str(VALID).unwrap();
        manifest.artifacts[0].sha256 = "bad".into();
        assert!(manifest.validate().is_err());
    }

    #[test]
    fn rejects_unknown_target() {
        let mut manifest: PackageManifest = toml::from_str(VALID).unwrap();
        manifest.artifacts[0].target = "wasm32-unknown-unknown".into();
        assert!(manifest.validate().is_err());
    }
}
