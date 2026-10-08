//! Run explicitly with a prebuilt knowledge:test image and a Docker daemon.
use ed25519_dalek::SigningKey;
use knowledge::{
    canonical,
    model::{Artifact, Payload},
};
use serde_json::Value;
use std::{
    process::{Command, Output},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

fn docker(arguments: &[&str]) -> Output {
    Command::new("docker")
        .args(arguments)
        .output()
        .expect("Docker must be installed")
}

fn success(arguments: &[&str]) -> String {
    let output = docker(arguments);
    assert!(
        output.status.success(),
        "docker {arguments:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

struct Cleanup {
    containers: Vec<String>,
    volume: String,
}
impl Drop for Cleanup {
    fn drop(&mut self) {
        for container in &self.containers {
            let _ = docker(&["rm", "--force", container]);
        }
        let _ = docker(&["volume", "rm", &self.volume]);
    }
}

fn launch(cleanup: &mut Cleanup, image: &str, suffix: &str, publisher: Option<&str>) -> String {
    let name = format!("{}-{suffix}", cleanup.volume);
    cleanup.containers.push(name.clone());
    let mount = format!("{}:/data", cleanup.volume);
    let mut arguments = vec![
        "run",
        "--detach",
        "--init",
        "--name",
        &name,
        "--read-only",
        "--cap-drop",
        "ALL",
        "--security-opt",
        "no-new-privileges:true",
        "--volume",
        &mount,
        "--publish",
        "127.0.0.1::8787",
        image,
    ];
    if let Some(publisher) = publisher {
        arguments.extend(["serve", "--allow-publisher", publisher]);
    }
    success(&arguments);
    for _ in 0..100 {
        let port = success(&["port", &name, "8787/tcp"]);
        let origin = format!("http://{port}");
        if ureq::get(&format!("{origin}/health"))
            .timeout(Duration::from_secs(1))
            .call()
            .is_ok()
        {
            return origin;
        }
        let running = success(&["inspect", "--format", "{{.State.Running}}", &name]);
        assert_eq!(running, "true", "{}", success(&["logs", &name]));
        thread::sleep(Duration::from_millis(100));
    }
    panic!(
        "container did not become ready: {}",
        success(&["logs", &name])
    );
}

#[test]
#[ignore = "requires Docker and a prebuilt Knowledge image"]
fn image_serves_as_nonroot_and_preserves_signed_artifacts_across_recreation() {
    let image = std::env::var("KNOWLEDGE_TEST_IMAGE").unwrap_or_else(|_| "knowledge:test".into());
    let config: Value = serde_json::from_str(&success(&["image", "inspect", &image])).unwrap();
    assert_eq!(config[0]["Config"]["User"], "10001:10001");
    assert!(config[0]["Config"]["Healthcheck"]["Test"].is_array());
    let volume = format!(
        "knowledge-test-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let mut cleanup = Cleanup {
        containers: vec![],
        volume,
    };
    success(&["volume", "create", &cleanup.volume]);
    let payload: Payload = canonical::parse(include_bytes!("../examples/payload.json")).unwrap();
    let artifact = Artifact::seal(payload, &SigningKey::from_bytes(&[7; 32])).unwrap();
    let bytes = canonical::encode(&artifact).unwrap();
    let address = artifact.address().unwrap();

    let origin = launch(&mut cleanup, &image, "readonly", None);
    let result = ureq::post(&format!("{origin}/v1/artifacts")).send_bytes(&bytes);
    assert!(matches!(result, Err(ureq::Error::Status(403, _))));
    success(&["exec", &cleanup.containers[0], "knowledge", "healthcheck"]);
    success(&["stop", "--time", "3", &cleanup.containers[0]]);
    success(&["rm", &cleanup.containers[0]]);

    let origin = launch(
        &mut cleanup,
        &image,
        "writer",
        Some(&artifact.payload.publisher),
    );
    let inserted: Value = serde_json::from_reader(
        ureq::post(&format!("{origin}/v1/artifacts"))
            .send_bytes(&bytes)
            .unwrap()
            .into_reader(),
    )
    .unwrap();
    assert_eq!(inserted["address"], address);
    // A different signing identity cannot write, even when publication is enabled.
    let other =
        Artifact::seal(artifact.payload.clone(), &SigningKey::from_bytes(&[8; 32])).unwrap();
    assert!(matches!(
        ureq::post(&format!("{origin}/v1/artifacts"))
            .send_bytes(&canonical::encode(&other).unwrap()),
        Err(ureq::Error::Status(403, _))
    ));
    success(&["stop", "--time", "3", &cleanup.containers[1]]);
    success(&["rm", &cleanup.containers[1]]);

    let origin = launch(&mut cleanup, &image, "recreated", None);
    let mut returned = Vec::new();
    use std::io::Read;
    ureq::get(&format!("{origin}/v1/artifacts/{address}"))
        .call()
        .unwrap()
        .into_reader()
        .read_to_end(&mut returned)
        .unwrap();
    let fetched: Artifact = canonical::parse(&returned).unwrap();
    assert_eq!(fetched.address().unwrap(), address);
    let found: Value = serde_json::from_reader(
        ureq::post(&format!("{origin}/v1/search"))
            .send_string(r#"{"include_incomplete":true}"#)
            .unwrap()
            .into_reader(),
    )
    .unwrap();
    assert_eq!(found["matches"][0]["address"], address);
    assert_eq!(found["matches"][0]["reusable"], false);

    for _ in 0..100 {
        let state = success(&[
            "inspect",
            "--format",
            "{{.State.Health.Status}}",
            &cleanup.containers[2],
        ]);
        if state == "healthy" {
            return;
        }
        thread::sleep(Duration::from_millis(200));
    }
    panic!("Docker healthcheck failed");
}

#[test]
#[ignore = "requires Docker Compose"]
fn compose_keeps_tailnet_routing_outside_the_cache_image() {
    let output = Command::new("docker")
        .args([
            "compose",
            "--file",
            "compose.yaml",
            "--file",
            "deploy/compose.tailnet.yaml",
            "config",
            "--format",
            "json",
        ])
        .env("TS_AUTHKEY", "")
        .env("KNOWLEDGE_PUBLISHERS", "")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let config: Value = serde_json::from_slice(&output.stdout).unwrap();
    let cache = &config["services"]["knowledge"];
    assert_eq!(cache["network_mode"], "service:tailscale");
    assert!(cache.get("ports").is_none());
    assert_eq!(cache["command"][2], "127.0.0.1:8787");
    let sidecar = &config["services"]["tailscale"];
    assert_eq!(sidecar["environment"]["TS_USERSPACE"], "true");
    assert_eq!(sidecar["environment"]["TS_AUTH_ONCE"], "true");
    assert!(sidecar.get("cap_add").is_none());
    assert!(sidecar.get("devices").is_none());
    assert!(sidecar.get("ports").is_none());
    let serve: Value =
        serde_json::from_str(include_str!("../deploy/tailscale/serve.json")).unwrap();
    assert_eq!(serve["TCP"]["8787"]["TCPForward"], "127.0.0.1:8787");
    assert!(serve.get("AllowFunnel").is_none());
}
