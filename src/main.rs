use clap::{Parser, Subcommand};
use ed25519_dalek::SigningKey;
use knowledge::{canonical, model::{Artifact, Payload, Rigor, ScopeKind}, query::{self, Query}, server, store::Cache, Error, Result};
use rand_core::OsRng;
use serde::Serialize;
use std::{fs::{File, OpenOptions}, io::{Read, Write}, net::SocketAddr, path::{Path, PathBuf}};

#[derive(Parser)]
#[command(version, about = "Content-addressed designs and traceable verification")]
struct Cli {
    #[arg(long, default_value = ".knowledge", global = true)]
    cache: PathBuf,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Generate a private signing key; prints the public key only.
    Keygen { #[arg(long)] output: PathBuf },
    /// Compute the verification subject hash of a draft payload.
    Subject { #[arg(long)] input: PathBuf },
    /// Sign a payload without changing its evidence or traceability.
    Seal { #[arg(long)] input: PathBuf, #[arg(long)] key: PathBuf },
    /// Validate and insert a signed public artifact.
    Put { #[arg(long)] input: PathBuf },
    /// Read an exact artifact, optionally substituting from other caches.
    Get { address: String, #[arg(long)] substituter: Vec<String> },
    /// Inspect publisher-reported verification and structural traceability.
    Inspect { #[arg(long)] input: PathBuf, #[arg(long)] trust_publisher: Option<String> },
    /// Search locally by specification and supplied semantic labels.
    Search {
        #[arg(default_value = "")] text: String,
        #[arg(long)] trust_publisher: Option<String>,
        #[arg(long)] include_incomplete: bool,
        #[arg(long)] physical: bool,
        #[arg(long, value_enum, conflicts_with = "physical")] rigor: Option<Rigor>,
        #[arg(long, value_enum)] scope: Option<ScopeKind>,
        #[arg(long)] supported_claims_only: bool,
        #[arg(long)] exclude_superseded: bool,
        #[arg(long, default_value_t = 20)] limit: usize,
    },
    /// Serve public artifacts and queries on loopback; read-only by default.
    Serve {
        #[arg(long, default_value = "127.0.0.1:8787")] listen: SocketAddr,
        #[arg(long)] allow_publisher: Vec<String>,
    },
}

fn read(path: &Path) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    File::open(path)?.take((canonical::MAX_BYTES + 1) as u64).read_to_end(&mut bytes)?;
    if bytes.len() > canonical::MAX_BYTES { return Err(Error::new("too_large", "input exceeds 4 MiB")); }
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
        Command::Keygen { output } => {
            let key = SigningKey::generate(&mut OsRng);
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)] {
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
            #[cfg(unix)] {
                use std::os::unix::fs::PermissionsExt;
                if std::fs::metadata(&key)?.permissions().mode() & 0o077 != 0 { return Err(Error::new("unsafe_key", "private key must have no group or other permissions")); }
            }
            let secret = read(&key)?;
            let secret = std::str::from_utf8(&secret).map_err(|_| Error::new("invalid_key", "private key is not hex"))?.trim();
            if !canonical::is_hex(secret, 32) { return Err(Error::new("invalid_key", "private key must contain 32 lowercase hex bytes")); }
            let bytes: [u8; 32] = hex::decode(secret).map_err(|_| Error::new("invalid_key", "invalid private key"))?.try_into().map_err(|_| Error::new("invalid_key", "invalid private key length"))?;
            let payload = canonical::parse(&read(&input)?)?;
            print(&Artifact::seal(payload, &SigningKey::from_bytes(&bytes))?)
        }
        Command::Inspect { input, trust_publisher } => {
            let artifact: Artifact = canonical::parse(&read(&input)?)?;
            print(&artifact.summary(trust_publisher.as_deref(), "input".into())?)
        }
        command => {
            let cache = Cache::open(cli.cache)?;
            match command {
                Command::Put { input } => {
                    let artifact = canonical::parse(&read(&input)?)?;
                    print(&serde_json::json!({"address":cache.put(&artifact)?}))
                }
                Command::Get { address, substituter } => print(&cache.fetch(&address, &substituter)?),
                Command::Search { text, trust_publisher, include_incomplete, physical, rigor, scope, supported_claims_only, exclude_superseded, limit } => print(&query::search(&cache, &Query { text, trust_publisher, include_incomplete, rigor: if physical { Some(Rigor::Physical) } else { rigor }, supported_claims_only, exclude_superseded, limit, scope })?),
                Command::Serve { listen, allow_publisher } => {
                    if allow_publisher.iter().any(|p| !canonical::is_hex(p, 32)) { return Err(Error::new("invalid_key", "invalid allowed publisher")); }
                    let http = server::bind(listen)?;
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
        eprintln!("{}", serde_json::to_string(&error).unwrap_or_else(|_| error.to_string()));
        std::process::exit(1);
    }
}
