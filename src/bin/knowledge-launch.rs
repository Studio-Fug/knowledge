use clap::{Parser, Subcommand, ValueEnum};
use knowledge::{Error, Result, canonical};
use std::{fs::File, io::Read, path::PathBuf, process::Command};

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum Mode {
    Local,
    Tailnet,
}

#[derive(Parser)]
#[command(about = "Launch a persistent containerized Knowledge cache")]
struct Cli {
    #[arg(long, default_value = ".", global = true)]
    directory: PathBuf,
    #[arg(long, default_value = "knowledge", global = true)]
    project: String,
    #[arg(long, value_enum, default_value = "tailnet", global = true)]
    mode: Mode,
    /// Read a Tailscale auth key from a file; otherwise use TS_AUTHKEY.
    #[arg(long, global = true)]
    authkey_file: Option<PathBuf>,
    #[arg(long, default_value = "knowledge", global = true)]
    hostname: String,
    #[arg(long, value_delimiter = ',', global = true)]
    allow_publisher: Vec<String>,
    #[command(subcommand)]
    action: Action,
}

#[derive(Subcommand)]
enum Action {
    /// Build, start and wait for the cache (and optional tailnet node).
    Up,
    /// Stop containers, preserving cache data and node identity.
    Down,
    /// Display container health and status.
    Status,
    /// Follow service logs.
    Logs,
}

fn key(path: &PathBuf) -> Result<String> {
    let mut bytes = Vec::new();
    File::open(path)?.take(4097).read_to_end(&mut bytes)?;
    if bytes.len() > 4096 {
        return Err(Error::new(
            "invalid_key",
            "auth key file exceeds 4096 bytes",
        ));
    }
    let key =
        String::from_utf8(bytes).map_err(|_| Error::new("invalid_key", "auth key is not UTF-8"))?;
    let key = key.trim();
    if key.is_empty() || key.chars().any(char::is_whitespace) {
        return Err(Error::new(
            "invalid_key",
            "auth key must be a single nonempty value",
        ));
    }
    Ok(key.to_owned())
}

fn compose_command(cli: Cli) -> Result<Command> {
    if cli.project.is_empty()
        || !cli
            .project
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-' || c == b'_')
        || !cli.project.as_bytes()[0].is_ascii_alphanumeric()
    {
        return Err(Error::new(
            "invalid_config",
            "project must be a lowercase Compose project name",
        ));
    }
    if cli.hostname.is_empty()
        || cli.hostname.len() > 63
        || !cli
            .hostname
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-')
        || !cli.hostname.as_bytes()[0].is_ascii_alphanumeric()
        || !cli.hostname.as_bytes()[cli.hostname.len() - 1].is_ascii_alphanumeric()
    {
        return Err(Error::new("invalid_config", "hostname must be a DNS label"));
    }
    if cli
        .allow_publisher
        .iter()
        .any(|p| !canonical::is_hex(p, 32))
    {
        return Err(Error::new("invalid_key", "invalid allowed publisher"));
    }
    if cli.mode == Mode::Local && cli.authkey_file.is_some() {
        return Err(Error::new(
            "invalid_config",
            "auth key is only used in tailnet mode",
        ));
    }
    let directory = cli.directory.canonicalize()?;
    let overlay = match cli.mode {
        Mode::Local => "deploy/compose.local.yaml",
        Mode::Tailnet => "deploy/compose.tailnet.yaml",
    };
    for file in ["compose.yaml", overlay] {
        if !directory.join(file).is_file() {
            return Err(Error::new(
                "invalid_config",
                "--directory must identify the Knowledge repository",
            ));
        }
    }
    let mut command = Command::new("docker");
    command.current_dir(directory).args([
        "compose",
        "--project-name",
        &cli.project,
        "--file",
        "compose.yaml",
        "--file",
        overlay,
    ]);
    command.env("KNOWLEDGE_HOSTNAME", &cli.hostname);
    if !cli.allow_publisher.is_empty() {
        command.env("KNOWLEDGE_PUBLISHERS", cli.allow_publisher.join(","));
    }
    if matches!(cli.action, Action::Up) && cli.mode == Mode::Tailnet {
        if let Some(path) = &cli.authkey_file {
            command.env("TS_AUTHKEY", key(path)?);
        }
    } else {
        command.env("TS_AUTHKEY", "");
    }
    match cli.action {
        Action::Up => {
            command.args([
                "up",
                "--detach",
                "--build",
                "--wait",
                "--wait-timeout",
                "120",
            ]);
        }
        Action::Down => {
            command.arg("down");
        }
        Action::Status => {
            command.arg("ps");
        }
        Action::Logs => {
            command.args(["logs", "--follow", "--tail", "100"]);
        }
    }
    Ok(command)
}

fn run(cli: Cli) -> Result<()> {
    let status = compose_command(cli)?.status().map_err(|_| {
        Error::new(
            "container",
            "could not run Docker Compose; install Docker with Compose v2",
        )
    })?;
    if !status.success() {
        return Err(Error::new(
            "container",
            "Docker Compose failed; inspect service logs",
        ));
    }
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_key_input_is_bounded_and_not_reported_in_errors() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("authkey");
        std::fs::write(&path, "  tskey-auth-example\n").unwrap();
        assert_eq!(key(&path).unwrap(), "tskey-auth-example");
        std::fs::write(&path, "secret another-secret").unwrap();
        assert!(
            !key(&path)
                .unwrap_err()
                .to_string()
                .contains("another-secret")
        );
        std::fs::write(&path, "x".repeat(4097)).unwrap();
        assert!(key(&path).is_err());
    }

    #[test]
    fn launcher_passes_keys_only_in_environment_and_down_preserves_volumes() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("authkey");
        std::fs::write(&path, "test-auth-key").unwrap();
        let cli = Cli::parse_from([
            "knowledge-launch",
            "--directory",
            env!("CARGO_MANIFEST_DIR"),
            "--authkey-file",
            path.to_str().unwrap(),
            "up",
        ]);
        let command = compose_command(cli).unwrap();
        assert!(command.get_envs().any(|(name, value)| {
            name == "TS_AUTHKEY" && value == Some(std::ffi::OsStr::new("test-auth-key"))
        }));
        assert!(!command.get_args().any(|arg| arg == "test-auth-key"));
        let cli = Cli::parse_from([
            "knowledge-launch",
            "--directory",
            env!("CARGO_MANIFEST_DIR"),
            "down",
        ]);
        let command = compose_command(cli).unwrap();
        assert!(command.get_args().any(|arg| arg == "down"));
        assert!(
            !command
                .get_args()
                .any(|arg| arg == "--volumes" || arg == "-v")
        );
        assert!(command.get_envs().any(|(name, value)| {
            name == "TS_AUTHKEY" && value == Some(std::ffi::OsStr::new(""))
        }));
    }
}
