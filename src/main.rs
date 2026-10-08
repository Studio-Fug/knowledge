use clap::{Parser, Subcommand};
use ed25519_dalek::SigningKey;
use knowledge::{
    Error, Result, canonical, backing,
    model::{Artifact, Payload, Rigor, ScopeKind},
    query::{self, Query},
    server,
    store::Cache,
};
use rand_core::OsRng;
use serde::Serialize;
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    net::SocketAddr,
    path::{Path, PathBuf},
};

#[derive(Parser)]
#[command(
    version,
    about = "Content-addressed designs and traceable verification"
)]
struct Cli {
    #[arg(
        long,
        env = "KNOWLEDGE_CACHE",
        default_value = ".knowledge",
        global = true
    )]
    cache: PathBuf,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Compute a deterministic source-tree SHA-256 from a Git tar.gz archive.
    SourceHash {
        #[arg(long)] input: PathBuf,
        #[arg(long, default_value = "")] subdirectory: String,
    },
    /// Fetch, verify and export all content for an exact artifact into a new directory.
    Realize {
        address: String,
        #[arg(long)] output: PathBuf,
        #[arg(long)] allow_origin: Vec<String>,
        #[arg(long)] substituter: Vec<String>,
    },
    /// Check an HTTP cache without opening or locking local storage.
    Healthcheck {
        #[arg(long, default_value = "http://127.0.0.1:8787/health")]
        url: String,
    },
    /// Generate a private signing key; prints the public key only.
    Keygen {
        #[arg(long)]
        output: PathBuf,
    },
    /// Compute the verification subject hash of a draft payload.
    Subject {
        #[arg(long)]
        input: PathBuf,
    },
    /// Sign a payload without changing its evidence or traceability.
    Seal {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        key: PathBuf,
    },
    /// Validate and insert a signed public artifact.
    Put {
        #[arg(long)]
        input: PathBuf,
    },
    /// Read an exact artifact, optionally substituting from other caches.
    Get {
        address: String,
        #[arg(long)]
        substituter: Vec<String>,
    },
    /// Inspect publisher-reported verification and structural traceability.
    Inspect {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        trust_publisher: Option<String>,
    },
    /// Search locally by specification and supplied semantic labels.
    Search {
        #[arg(default_value = "")]
        text: String,
        #[arg(long)]
        trust_publisher: Option<String>,
        #[arg(long)]
        include_incomplete: bool,
        #[arg(long)]
        physical: bool,
        #[arg(long, value_enum, conflicts_with = "physical")]
        rigor: Option<Rigor>,
        #[arg(long, value_enum)]
        scope: Option<ScopeKind>,
        #[arg(long)]
        supported_claims_only: bool,
        #[arg(long)]
        exclude_superseded: bool,
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    /// Serve public artifacts and queries; read-only and loopback by default.
    Serve {
        #[arg(long, env = "KNOWLEDGE_LISTEN", default_value = "127.0.0.1:8787")]
        listen: SocketAddr,
        #[arg(long, env = "KNOWLEDGE_ALLOW_NETWORK")]
        allow_network: bool,
        #[arg(long, env = "KNOWLEDGE_PUBLISHERS", value_delimiter = ',')]
        allow_publisher: Vec<String>,
    },
}

fn read(path: &Path) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    File::open(path)?
        .take((canonical::MAX_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.len() > canonical::MAX_BYTES {
        return Err(Error::new("too_large", "input exceeds 4 MiB"));
    }
    Ok(bytes)
}

fn print(value: &impl Serialize) -> Result<()> {
    let mut stdout = std::io::stdout().lock();
    stdout.write_all(&canonical::encode(value)?)?;
    stdout.write_all(b"\n")?;
    Ok(())
}

fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Command::SourceHash { input, subdirectory } => {
            let mut bytes = Vec::new();
            File::open(input)?.take(32 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
            let tree = backing::realize(&bytes, &subdirectory)?;
            print(&serde_json::json!({"sha256":tree.sha256,"files":tree.files.len(),"format":"git_tar_gzip_v1"}))
        }
        Command::Healthcheck { url } => {
            let response = ureq::AgentBuilder::new()
                .timeout(std::time::Duration::from_secs(2))
                .redirects(0)
                .build()
                .get(&url)
                .call()
                .map_err(|_| Error::new("unhealthy", "cache health endpoint is unavailable"))?;
            if response.status() != 200 {
                return Err(Error::new(
                    "unhealthy",
                    "cache health endpoint did not return 200",
                ));
            }
            let mut bytes = Vec::new();
            response.into_reader().take(1025).read_to_end(&mut bytes)?;
            if bytes.len() > 1024 {
                return Err(Error::new("unhealthy", "health response exceeds limits"));
            }
            let status: serde_json::Value = canonical::parse(&bytes)?;
            if status.get("status").and_then(serde_json::Value::as_str) != Some("ok") {
                return Err(Error::new("unhealthy", "cache is not healthy"));
            }
            print(&status)
        }
        Command::Keygen { output } => {
            let key = SigningKey::generate(&mut OsRng);
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut file = options.open(output)?;
            file.write_all(hex::encode(key.to_bytes()).as_bytes())?;
            file.sync_all()?;
            print(&serde_json::json!({"publisher":hex::encode(key.verifying_key().to_bytes())}))
        }
        Command::Subject { input } => {
            let payload: Payload = canonical::parse(&read(&input)?)?;
            print(&serde_json::json!({"subject_hash":payload.subject_hash()?}))
        }
        Command::Seal { input, key } => {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if std::fs::metadata(&key)?.permissions().mode() & 0o077 != 0 {
                    return Err(Error::new(
                        "unsafe_key",
                        "private key must have no group or other permissions",
                    ));
                }
            }
            let secret = read(&key)?;
            let secret = std::str::from_utf8(&secret)
                .map_err(|_| Error::new("invalid_key", "private key is not hex"))?
                .trim();
            if !canonical::is_hex(secret, 32) {
                return Err(Error::new(
                    "invalid_key",
                    "private key must contain 32 lowercase hex bytes",
                ));
            }
            let bytes: [u8; 32] = hex::decode(secret)
                .map_err(|_| Error::new("invalid_key", "invalid private key"))?
                .try_into()
                .map_err(|_| Error::new("invalid_key", "invalid private key length"))?;
            let payload = canonical::parse(&read(&input)?)?;
            print(&Artifact::seal(payload, &SigningKey::from_bytes(&bytes))?)
        }
        Command::Inspect {
            input,
            trust_publisher,
        } => {
            let artifact: Artifact = canonical::parse(&read(&input)?)?;
            print(&artifact.summary(trust_publisher.as_deref(), "input".into())?)
        }
        command => {
            let cache = Cache::open(cli.cache)?;
            match command {
                Command::Realize { address, output, allow_origin, substituter } => {
                    let artifact = cache.fetch(&address, &substituter)?;
                    let mut trees = Vec::new();
                    let mut total = 0usize;
                    for (prefix, blobs) in [("design", &artifact.payload.design), ("records", &artifact.payload.records)] {
                        for blob in blobs {
                            let bytes = hex::decode(&blob.data).map_err(|_| Error::new("invalid_artifact", "invalid blob bytes"))?;
                            total += bytes.len();
                            trees.push((prefix.to_owned(), backing::Realization { sha256: canonical::digest(&bytes), files: vec![backing::RealizedFile { path: blob.path.clone(), executable: false, bytes }] }));
                        }
                    }
                    for source in &artifact.payload.sources {
                        let tree = source.fetch(&allow_origin)?;
                        total += tree.files.iter().map(|f| f.bytes.len()).sum::<usize>();
                        if total > 64 * 1024 * 1024 { return Err(Error::new("too_large", "combined realization exceeds 64 MiB")); }
                        let role = if source.role == backing::Role::Design { "design" } else { "records" };
                        trees.push((format!("{role}/{}", source.path), tree));
                    }
                    backing::export(&trees, &output)?;
                    print(&serde_json::json!({"address":address,"output":output,"source_commitments_checked":artifact.payload.sources.len(),"bytes":total,"checking":"content_and_signatures_checked; tests_not_rerun"}))
                }
                Command::Put { input } => {
                    let artifact = canonical::parse(&read(&input)?)?;
                    print(&serde_json::json!({"address":cache.put(&artifact)?}))
                }
                Command::Get {
                    address,
                    substituter,
                } => print(&cache.fetch(&address, &substituter)?),
                Command::Search {
                    text,
                    trust_publisher,
                    include_incomplete,
                    physical,
                    rigor,
                    scope,
                    supported_claims_only,
                    exclude_superseded,
                    limit,
                } => print(&query::search(
                    &cache,
                    &Query {
                        text,
                        trust_publisher,
                        include_incomplete,
                        rigor: if physical {
                            Some(Rigor::Physical)
                        } else {
                            rigor
                        },
                        supported_claims_only,
                        exclude_superseded,
                        limit,
                        scope,
                    },
                )?),
                Command::Serve {
                    listen,
                    allow_network,
                    allow_publisher,
                } => {
                    let allow_publisher: Vec<_> = allow_publisher
                        .into_iter()
                        .filter(|p| !p.is_empty())
                        .collect();
                    if allow_publisher.iter().any(|p| !canonical::is_hex(p, 32)) {
                        return Err(Error::new("invalid_key", "invalid allowed publisher"));
                    }
                    let http = server::bind_with_network(listen, allow_network)?;
                    eprintln!("knowledge listening on {listen}; public artifacts only");
                    server::serve(http, &cache, &allow_publisher)
                }
                _ => unreachable!(),
            }
        }
    }
}

fn main() {
    if let Err(error) = run(Cli::parse()) {
        eprintln!(
            "{}",
            serde_json::to_string(&error).unwrap_or_else(|_| error.to_string())
        );
        std::process::exit(1);
    }
}
