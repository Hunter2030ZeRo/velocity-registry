use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    fs,
    io::Cursor,
    path::{Path, PathBuf},
};
use thiserror::Error;
use velocity_manifest::{load_manifest, Artifact, ManifestError, PackageManifest, TargetPredicate};
use walkdir::WalkDir;

const INDEX_FORMAT_VERSION: u32 = 1;
const INDEX_MAGIC: &[u8; 8] = b"VLTIDX1\0";

type PackageId = u64;

#[derive(Debug, Parser)]
#[command(name = "velocity-index")]
#[command(about = "Validate Velocity manifests and compile the registry index")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Validate all manifests and cross-package dependencies.
    Validate {
        #[arg(long, default_value = "packages")]
        packages: PathBuf,
    },
    /// Compile manifests into a compressed binary index.
    Build {
        #[arg(long, default_value = "packages")]
        packages: PathBuf,
        #[arg(long, default_value = "generated/velocity.idx.zst")]
        output: PathBuf,
    },
}

#[derive(Debug, Error)]
enum IndexError {
    #[error(transparent)]
    Manifest(#[from] ManifestError),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("index serialization failed: {0}")]
    Serialize(#[from] Box<bincode::ErrorKind>),
    #[error("registry validation failed: {0}")]
    Validation(String),
}

#[derive(Debug, Clone)]
struct Registry {
    packages: Vec<PackageManifest>,
    ids: HashMap<String, PackageId>,
}

#[derive(Debug, Serialize, Deserialize)]
struct CompiledIndex {
    format_version: u32,
    packages: Vec<CompiledPackage>,
    aliases: Vec<CompiledAlias>,
}

#[derive(Debug, Serialize, Deserialize)]
struct CompiledPackage {
    id: PackageId,
    name: String,
    version: String,
    description: String,
    homepage: Option<String>,
    license: Option<String>,
    provides: Vec<String>,
    conflicts: Vec<String>,
    dependencies: Vec<CompiledDependency>,
    artifacts: Vec<Artifact>,
}

#[derive(Debug, Serialize, Deserialize)]
struct CompiledDependency {
    package_id: PackageId,
    requirement: String,
    when: TargetPredicate,
}

#[derive(Debug, Serialize, Deserialize)]
struct CompiledAlias {
    alias: String,
    package_id: PackageId,
}

fn main() -> Result<(), IndexError> {
    let cli = Cli::parse();

    match cli.command {
        Command::Validate { packages } => {
            let registry = load_registry(&packages)?;
            println!("validated {} package(s)", registry.packages.len());
        }
        Command::Build { packages, output } => {
            let registry = load_registry(&packages)?;
            let index = compile_registry(&registry)?;
            write_index(&index, &output)?;
            println!(
                "compiled {} package(s) -> {}",
                index.packages.len(),
                output.display()
            );
        }
    }

    Ok(())
}

fn load_registry(packages_dir: &Path) -> Result<Registry, IndexError> {
    if !packages_dir.exists() {
        return Err(IndexError::Validation(format!(
            "package directory does not exist: {}",
            packages_dir.display()
        )));
    }

    let mut paths: Vec<PathBuf> = WalkDir::new(packages_dir)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| entry.into_path())
        .filter(|path| path.extension().and_then(|s| s.to_str()) == Some("toml"))
        .collect();
    paths.sort();

    let mut packages = Vec::with_capacity(paths.len());
    for path in paths {
        packages.push(load_manifest(path)?);
    }

    packages.sort_by(|a, b| a.name.cmp(&b.name));

    let mut ids = HashMap::new();
    let mut reverse_ids: HashMap<PackageId, String> = HashMap::new();
    for package in &packages {
        let id = package_id(&package.name);
        if let Some(existing) = reverse_ids.insert(id, package.name.clone()) {
            return Err(IndexError::Validation(format!(
                "PackageId collision: {existing:?} and {:?} both map to {id:#x}",
                package.name
            )));
        }
        insert_identity(&mut ids, &package.name, id)?;
        for alias in &package.aliases {
            insert_identity(&mut ids, alias, id)?;
        }
    }

    for package in &packages {
        for dependency in &package.dependencies {
            let Some(dep_id) = ids.get(&dependency.name).copied() else {
                return Err(IndexError::Validation(format!(
                    "{}: dependency {:?} does not exist in the registry",
                    package.name, dependency.name
                )));
            };
            let dep_package = packages
                .iter()
                .find(|candidate| package_id(&candidate.name) == dep_id)
                .expect("identity map must point to a package");
            if !dependency.version.matches(&dep_package.version) {
                return Err(IndexError::Validation(format!(
                    "{} requires {} {}, but registry contains {}",
                    package.name, dependency.name, dependency.version, dep_package.version
                )));
            }
        }
    }

    validate_dependency_cycles(&packages, &ids)?;

    Ok(Registry { packages, ids })
}

fn insert_identity(
    ids: &mut HashMap<String, PackageId>,
    identity: &str,
    id: PackageId,
) -> Result<(), IndexError> {
    if let Some(existing) = ids.insert(identity.to_string(), id) {
        if existing != id {
            return Err(IndexError::Validation(format!(
                "package name/alias collision for {identity:?}"
            )));
        }
    }
    Ok(())
}

fn validate_dependency_cycles(
    packages: &[PackageManifest],
    ids: &HashMap<String, PackageId>,
) -> Result<(), IndexError> {
    let mut graph: HashMap<PackageId, Vec<PackageId>> = HashMap::new();
    let mut names: HashMap<PackageId, &str> = HashMap::new();

    for package in packages {
        let id = package_id(&package.name);
        names.insert(id, &package.name);
        let edges = package
            .dependencies
            .iter()
            .filter_map(|dep| ids.get(&dep.name).copied())
            .collect();
        graph.insert(id, edges);
    }

    fn visit(
        node: PackageId,
        graph: &HashMap<PackageId, Vec<PackageId>>,
        temporary: &mut HashSet<PackageId>,
        permanent: &mut HashSet<PackageId>,
        stack: &mut Vec<PackageId>,
    ) -> Option<Vec<PackageId>> {
        if permanent.contains(&node) {
            return None;
        }
        if !temporary.insert(node) {
            if let Some(start) = stack.iter().position(|id| *id == node) {
                let mut cycle = stack[start..].to_vec();
                cycle.push(node);
                return Some(cycle);
            }
            return Some(vec![node, node]);
        }

        stack.push(node);
        if let Some(edges) = graph.get(&node) {
            for &next in edges {
                if let Some(cycle) = visit(next, graph, temporary, permanent, stack) {
                    return Some(cycle);
                }
            }
        }
        stack.pop();
        temporary.remove(&node);
        permanent.insert(node);
        None
    }

    let mut temporary = HashSet::new();
    let mut permanent = HashSet::new();
    let mut stack = Vec::new();

    for &node in graph.keys() {
        if let Some(cycle) = visit(
            node,
            &graph,
            &mut temporary,
            &mut permanent,
            &mut stack,
        ) {
            let pretty = cycle
                .iter()
                .map(|id| names.get(id).copied().unwrap_or("<unknown>"))
                .collect::<Vec<_>>()
                .join(" -> ");
            return Err(IndexError::Validation(format!(
                "dependency cycle detected: {pretty}"
            )));
        }
    }

    Ok(())
}

fn compile_registry(registry: &Registry) -> Result<CompiledIndex, IndexError> {
    let mut packages = Vec::with_capacity(registry.packages.len());
    let mut aliases = Vec::new();

    for package in &registry.packages {
        let id = package_id(&package.name);
        let dependencies = package
            .dependencies
            .iter()
            .map(|dep| {
                let package_id = registry.ids.get(&dep.name).copied().ok_or_else(|| {
                    IndexError::Validation(format!(
                        "{}: unresolved dependency {:?}",
                        package.name, dep.name
                    ))
                })?;
                Ok(CompiledDependency {
                    package_id,
                    requirement: dep.version.to_string(),
                    when: dep.when.clone(),
                })
            })
            .collect::<Result<Vec<_>, IndexError>>()?;

        for alias in &package.aliases {
            aliases.push(CompiledAlias {
                alias: alias.clone(),
                package_id: id,
            });
        }

        packages.push(CompiledPackage {
            id,
            name: package.name.clone(),
            version: package.version.to_string(),
            description: package.description.clone(),
            homepage: package.homepage.clone(),
            license: package.license.clone(),
            provides: package.provides.clone(),
            conflicts: package.conflicts.clone(),
            dependencies,
            artifacts: package.artifacts.clone(),
        });
    }

    packages.sort_by_key(|package| package.id);
    aliases.sort_by(|a, b| a.alias.cmp(&b.alias));

    Ok(CompiledIndex {
        format_version: INDEX_FORMAT_VERSION,
        packages,
        aliases,
    })
}

fn write_index(index: &CompiledIndex, output: &Path) -> Result<(), IndexError> {
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }

    let payload = bincode::serialize(index)?;
    let mut raw = Vec::with_capacity(INDEX_MAGIC.len() + 4 + payload.len());
    raw.extend_from_slice(INDEX_MAGIC);
    raw.extend_from_slice(&INDEX_FORMAT_VERSION.to_le_bytes());
    raw.extend_from_slice(&payload);

    let compressed = zstd::stream::encode_all(Cursor::new(raw), 12)?;
    fs::write(output, &compressed)?;

    let digest = Sha256::digest(&compressed);
    let checksum_path = PathBuf::from(format!("{}.sha256", output.display()));
    let file_name = output
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("velocity.idx.zst");
    fs::write(
        checksum_path,
        format!("{}  {}\n", hex::encode(digest), file_name),
    )?;

    Ok(())
}

fn package_id(name: &str) -> PackageId {
    // Stable FNV-1a 64-bit ID. The registry compiler rejects collisions.
    const OFFSET: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;

    let mut hash = OFFSET;
    for byte in name.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_ids_are_stable() {
        assert_eq!(package_id("ripgrep"), package_id("ripgrep"));
        assert_ne!(package_id("ripgrep"), package_id("fd"));
    }
}
