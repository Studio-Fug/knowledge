use crate::{Error, Result, canonical, model::Artifact};
use fs2::FileExt;
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    time::Duration,
};

pub struct Cache {
    root: PathBuf,
    _lock: File,
}

fn regular(path: &Path) -> Result<()> {
    if fs::symlink_metadata(path)?.file_type().is_symlink() {
        return Err(Error::new(
            "unsafe_path",
            "cache paths cannot be symbolic links",
        ));
    }
    Ok(())
}

impl Cache {
    pub fn open(root: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref();
        fs::create_dir_all(root)?;
        regular(root)?;
        let objects = root.join("objects");
        fs::create_dir_all(&objects)?;
        regular(&objects)?;
        let lock_path = root.join("lock");
        if lock_path.exists() {
            regular(&lock_path)?;
        }
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(lock_path)?;
        lock.try_lock_exclusive()
            .map_err(|_| Error::new("cache_busy", "another process owns this cache"))?;
        Ok(Self {
            root: root.to_owned(),
            _lock: lock,
        })
    }

    fn path(&self, address: &str) -> Result<PathBuf> {
        if !canonical::is_hex(address, 32) {
            return Err(Error::new("invalid_address", "expected lowercase SHA-256"));
        }
        Ok(self.root.join("objects").join(format!("{address}.json")))
    }

    pub fn get(&self, address: &str) -> Result<Artifact> {
        let path = self.path(address)?;
        if !path.try_exists()? {
            return Err(Error::new("not_found", "artifact is not cached"));
        }
        regular(&path)?;
        let mut bytes = Vec::new();
        File::open(path)?
            .take((canonical::MAX_BYTES + 1) as u64)
            .read_to_end(&mut bytes)?;
        let artifact: Artifact = canonical::parse(&bytes)?;
        artifact.validate()?;
        if artifact.address()? != address {
            return Err(Error::new(
                "hash_mismatch",
                "stored artifact does not match its address",
            ));
        }
        Ok(artifact)
    }

    pub fn put(&self, artifact: &Artifact) -> Result<String> {
        artifact.validate()?;
        let address = artifact.address()?;
        let path = self.path(&address)?;
        if path.exists() {
            self.get(&address)?;
            return Ok(address);
        }
        let existing = self.addresses()?;
        if existing.len() >= 1000 {
            return Err(Error::new(
                "cache_full",
                "prototype cache limit is 1000 artifacts",
            ));
        }
        let bytes = canonical::encode(artifact)?;
        let size: u64 = existing
            .iter()
            .map(|h| self.path(h).and_then(|p| Ok(fs::metadata(p)?.len())))
            .collect::<Result<Vec<_>>>()?
            .iter()
            .sum();
        if size + bytes.len() as u64 > 256 * 1024 * 1024 {
            return Err(Error::new("cache_full", "prototype cache limit is 256 MiB"));
        }
        let mut temp = tempfile::NamedTempFile::new_in(self.root.join("objects"))?;
        temp.write_all(&bytes)?;
        temp.as_file().sync_all()?;
        temp.persist_noclobber(path)
            .map_err(|e| Error::new("io", e.to_string()))?;
        #[cfg(unix)]
        File::open(self.root.join("objects"))?.sync_all()?;
        Ok(address)
    }

    pub fn addresses(&self) -> Result<Vec<String>> {
        let mut addresses = Vec::new();
        for entry in fs::read_dir(self.root.join("objects"))? {
            let entry = entry?;
            if let Some(name) = entry
                .file_name()
                .to_str()
                .and_then(|n| n.strip_suffix(".json"))
                && canonical::is_hex(name, 32)
            {
                addresses.push(name.to_owned());
            }
            if addresses.len() > 1000 {
                return Err(Error::new("cache_full", "too many cached artifacts"));
            }
        }
        addresses.sort();
        Ok(addresses)
    }

    pub fn fetch(&self, address: &str, remotes: &[String]) -> Result<Artifact> {
        let path = self.path(address)?;
        if path.exists() {
            return self.get(address);
        }
        let agent = ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(10))
            .redirects(0)
            .build();
        for remote in remotes {
            let attempt = (|| -> Result<Artifact> {
                if !(remote.starts_with("http://") || remote.starts_with("https://"))
                    || remote.contains(['?', '#', '@'])
                {
                    return Err(Error::new(
                        "invalid_remote",
                        "remote must be an HTTP(S) origin without credentials",
                    ));
                }
                let response = agent
                    .get(&format!(
                        "{}/v1/artifacts/{address}",
                        remote.trim_end_matches('/')
                    ))
                    .call()
                    .map_err(|e| Error::new("remote", e.to_string()))?;
                if response.status() != 200 {
                    return Err(Error::new("remote", "unexpected response status"));
                }
                let mut bytes = Vec::new();
                response
                    .into_reader()
                    .take((canonical::MAX_BYTES + 1) as u64)
                    .read_to_end(&mut bytes)?;
                let artifact: Artifact = canonical::parse(&bytes)?;
                artifact.validate()?;
                if artifact.address()? != address {
                    return Err(Error::new(
                        "hash_mismatch",
                        "substituter returned a different artifact",
                    ));
                }
                self.put(&artifact)?;
                Ok(artifact)
            })();
            if let Ok(artifact) = attempt {
                return Ok(artifact);
            }
        }
        Err(Error::new(
            "not_found",
            "artifact unavailable locally and from configured substituters",
        ))
    }
}
