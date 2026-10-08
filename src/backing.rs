//! Deterministic Git source archive realizations; never unpack or execute archives.
use crate::{Error, Result, canonical};
use flate2::read::GzDecoder;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs::{self, File},
    io::{Read, Write},
    path::Path,
    time::Duration,
};
use url::Url;

const MAX_ARCHIVE: usize = 32 * 1024 * 1024;
const MAX_TREE: u64 = 64 * 1024 * 1024;
const MAX_FILE: u64 = 16 * 1024 * 1024;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Design,
    Record,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Format {
    GitTarGzipV1,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub path: String,
    pub role: Role,
    pub repository: String,
    pub revision: String,
    pub archive: String,
    pub subdirectory: String,
    pub format: Format,
    pub sha256: String,
}

pub fn safe_path(path: &str) -> bool {
    !path.is_empty()
        && !path.contains(['\\', ':'])
        && path.len() <= 4096
        && path
            .split('/')
            .all(|p| !p.is_empty() && p != "." && p != "..")
        && !path.chars().any(char::is_control)
}

fn https(value: &str) -> Result<Url> {
    let url = Url::parse(value)
        .map_err(|_| Error::new("invalid_source", "source must be an absolute HTTPS URL"))?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(Error::new(
            "invalid_source",
            "source URLs must use HTTPS without credentials, query or fragment",
        ));
    }
    Ok(url)
}

impl Source {
    pub fn validate(&self) -> Result<()> {
        if !safe_path(&self.path)
            || (!self.subdirectory.is_empty() && !safe_path(&self.subdirectory))
        {
            return Err(Error::new(
                "invalid_source",
                "source paths must be normalized relative paths",
            ));
        }
        if !(canonical::is_hex(&self.revision, 20) || canonical::is_hex(&self.revision, 32))
            || !canonical::is_hex(&self.sha256, 32)
        {
            return Err(Error::new(
                "invalid_source",
                "source needs a full Git revision and a SHA-256 realization commitment",
            ));
        }
        if self.repository.len() > 8192 || self.archive.len() > 8192 {
            return Err(Error::new("too_large", "source URL exceeds limits"));
        }
        https(&self.repository)?;
        let archive = https(&self.archive)?;
        if !archive.path().contains(&self.revision) {
            return Err(Error::new(
                "invalid_source",
                "archive URL must identify the pinned revision",
            ));
        }
        Ok(())
    }

    pub fn fetch(&self, allowed_origins: &[String]) -> Result<Realization> {
        self.validate()?;
        let url = https(&self.archive)?;
        if !allowed_origins.iter().any(|origin| {
            https(origin)
                .is_ok_and(|allowed| allowed.path() == "/" && allowed.origin() == url.origin())
        }) {
            return Err(Error::new(
                "source_forbidden",
                "source origin is not explicitly allowed by the caller",
            ));
        }
        let response = ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(30))
            .redirects(0)
            .build()
            .get(&self.archive)
            .call()
            .map_err(|_| Error::new("source_unavailable", "source archive could not be fetched"))?;
        if response.status() != 200 {
            return Err(Error::new(
                "source_unavailable",
                "source archive did not return 200",
            ));
        }
        let mut bytes = Vec::new();
        response
            .into_reader()
            .take((MAX_ARCHIVE + 1) as u64)
            .read_to_end(&mut bytes)?;
        self.check(&bytes)
    }

    pub fn check(&self, archive: &[u8]) -> Result<Realization> {
        self.validate()?;
        let realized = realize(archive, &self.subdirectory)?;
        if realized.sha256 != self.sha256 {
            return Err(Error::new(
                "source_hash_mismatch",
                "realized source does not match its committed SHA-256",
            ));
        }
        Ok(realized)
    }
}

#[derive(Debug)]
pub struct RealizedFile {
    pub path: String,
    pub executable: bool,
    pub bytes: Vec<u8>,
}
#[derive(Debug)]
pub struct Realization {
    pub sha256: String,
    pub files: Vec<RealizedFile>,
}

/// Hash frame: domain, file count (u64 BE), then path length/path, executable
/// byte, content length (u64 BE), content. Paths are sorted by their UTF-8 bytes.
pub fn realize(bytes: &[u8], subdirectory: &str) -> Result<Realization> {
    if bytes.len() > MAX_ARCHIVE || (!subdirectory.is_empty() && !safe_path(subdirectory)) {
        return Err(Error::new(
            "too_large",
            "invalid or oversized source archive",
        ));
    }
    let decoder = GzDecoder::new(bytes).take(MAX_TREE + 8 * 1024 * 1024 + 1);
    let mut archive = tar::Archive::new(decoder);
    let mut files = Vec::new();
    let mut paths = BTreeSet::new();
    let mut root = None::<String>;
    let mut total = 0u64;
    let mut entries = 0;
    for item in archive
        .entries()
        .map_err(|_| Error::new("invalid_archive", "cannot read archive"))?
    {
        let mut entry = item.map_err(|_| Error::new("invalid_archive", "invalid archive entry"))?;
        entries += 1;
        if entries > 10000 {
            return Err(Error::new("too_large", "archive exceeds 10000 entries"));
        }
        let kind = entry.header().entry_type();
        if matches!(kind.as_byte(), b'g' | b'x') {
            if entry
                .header()
                .size()
                .map_err(|_| Error::new("invalid_archive", "invalid metadata size"))?
                > 65536
            {
                return Err(Error::new("too_large", "archive metadata exceeds 64 KiB"));
            }
            continue;
        }
        let path = entry
            .path()
            .map_err(|_| Error::new("invalid_archive", "invalid archive path"))?;
        let path = path
            .to_str()
            .ok_or_else(|| Error::new("invalid_archive", "archive paths must be UTF-8"))?
            .trim_end_matches('/')
            .to_owned();
        if !safe_path(&path) || (!kind.is_dir() && !kind.is_file()) {
            return Err(Error::new(
                "invalid_archive",
                "archive contains unsafe paths, links or special files",
            ));
        }
        let (prefix, relative) = path.split_once('/').unwrap_or((&path, ""));
        if root.as_ref().is_some_and(|r| r != prefix) {
            return Err(Error::new(
                "invalid_archive",
                "archive must have one root directory",
            ));
        }
        root = Some(prefix.to_owned());
        if !paths.insert(path.clone()) {
            return Err(Error::new("invalid_archive", "duplicate archive path"));
        }
        if kind.is_dir() {
            continue;
        }
        if relative.is_empty() {
            return Err(Error::new(
                "invalid_archive",
                "source archive files must be under its root directory",
            ));
        }
        let size = entry
            .header()
            .size()
            .map_err(|_| Error::new("invalid_archive", "invalid file size"))?;
        total = total
            .checked_add(size)
            .ok_or_else(|| Error::new("too_large", "source size overflow"))?;
        if size > MAX_FILE || total > MAX_TREE {
            return Err(Error::new(
                "too_large",
                "source exceeds 16 MiB/file or 64 MiB/tree",
            ));
        }
        let selected = if subdirectory.is_empty() {
            Some(relative)
        } else {
            relative
                .strip_prefix(subdirectory)
                .and_then(|p| p.strip_prefix('/'))
        };
        if let Some(relative) = selected {
            let executable = entry
                .header()
                .mode()
                .map_err(|_| Error::new("invalid_archive", "invalid mode"))?
                & 0o111
                != 0;
            let mut bytes = Vec::new();
            entry.by_ref().take(MAX_FILE + 1).read_to_end(&mut bytes)?;
            if bytes.len() as u64 != size {
                return Err(Error::new("invalid_archive", "truncated source file"));
            }
            files.push(RealizedFile {
                path: relative.to_owned(),
                executable,
                bytes,
            });
        }
    }
    // Drain the decoder so malformed gzip trailers cannot be silently accepted.
    let mut decoder = archive.into_inner();
    std::io::copy(&mut decoder, &mut std::io::sink())?;
    if decoder.limit() == 0 {
        return Err(Error::new("too_large", "expanded archive exceeds limits"));
    }
    if files.is_empty() {
        return Err(Error::new(
            "invalid_archive",
            "selected source tree has no files",
        ));
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    let file_paths: BTreeSet<_> = files.iter().map(|f| f.path.as_str()).collect();
    for file in &files {
        let mut parent = file.path.as_str();
        while let Some((prefix, _)) = parent.rsplit_once('/') {
            if file_paths.contains(prefix) {
                return Err(Error::new(
                    "invalid_archive",
                    "a source file is also a directory",
                ));
            }
            parent = prefix;
        }
    }
    let mut hash = Sha256::new();
    hash.update(b"knowledge:git-tree:v1\0");
    hash.update((files.len() as u64).to_be_bytes());
    for file in &files {
        hash.update((file.path.len() as u64).to_be_bytes());
        hash.update(file.path.as_bytes());
        hash.update([u8::from(file.executable)]);
        hash.update((file.bytes.len() as u64).to_be_bytes());
        hash.update(&file.bytes);
    }
    Ok(Realization {
        sha256: hex::encode(hash.finalize()),
        files,
    })
}

/// Export only already-checked files into a new directory. No overwriting.
pub fn export(trees: &[(String, Realization)], output: &Path) -> Result<()> {
    if output.exists() {
        return Err(Error::new(
            "already_exists",
            "realization output must not exist",
        ));
    }
    let parent = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let staging = tempfile::tempdir_in(parent)?;
    let mut written = BTreeSet::new();
    for (prefix, tree) in trees {
        for file in &tree.files {
            let relative = format!("{prefix}/{}", file.path);
            if !safe_path(&relative) || !written.insert(relative.clone()) {
                return Err(Error::new(
                    "invalid_source",
                    "overlapping realization paths",
                ));
            }
            let path = staging.path().join(relative);
            fs::create_dir_all(
                path.parent()
                    .ok_or_else(|| Error::new("invalid_source", "missing parent"))?,
            )?;
            let mut destination = File::create_new(path)?;
            destination.write_all(&file.bytes)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                destination.set_permissions(fs::Permissions::from_mode(if file.executable {
                    0o700
                } else {
                    0o600
                }))?;
            }
        }
    }
    fs::rename(staging.path(), output)?;
    Ok(())
}
