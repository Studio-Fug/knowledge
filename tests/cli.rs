use knowledge::{canonical, model::{Artifact, Completeness}};
use std::process::Command;

#[test]
fn command_line_publishes_and_queries_the_unverified_example() {
    let directory = tempfile::tempdir().unwrap();
    let cache = directory.path().join("cache");
    let key = directory.path().join("publisher.secret");
    let run = |arguments: &[&str]| {
        let output = Command::new(env!("CARGO_BIN_EXE_knowledge")).arg("--cache").arg(&cache).args(arguments).output().unwrap();
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
        output.stdout
    };
    let public: serde_json::Value = serde_json::from_slice(&run(&["keygen", "--output", key.to_str().unwrap()])).unwrap();
    assert!(public["publisher"].as_str().unwrap().len() == 64);
    let example = format!("{}/examples/payload.json", env!("CARGO_MANIFEST_DIR"));
    let sealed = run(&["seal", "--input", &example, "--key", key.to_str().unwrap()]);
    let artifact: Artifact = canonical::parse(&sealed).unwrap();
    assert_eq!(artifact.summary(None, "test".into()).unwrap().completeness, Completeness::Incomplete);
    let input = directory.path().join("artifact.json");
    std::fs::write(&input, sealed).unwrap();
    let inserted: serde_json::Value = serde_json::from_slice(&run(&["put", "--input", input.to_str().unwrap()])).unwrap();
    assert_eq!(inserted["address"], artifact.address().unwrap());
    let default: serde_json::Value = serde_json::from_slice(&run(&["search", "moving water"])).unwrap();
    assert!(default["matches"].as_array().unwrap().is_empty());
    let draft: serde_json::Value = serde_json::from_slice(&run(&["search", "moving water", "--include-incomplete"])).unwrap();
    assert_eq!(draft["matches"].as_array().unwrap().len(), 1);
    assert_eq!(draft["matches"][0]["reusable"], false);
    let fetched: Artifact = canonical::parse(&run(&["get", inserted["address"].as_str().unwrap()])).unwrap();
    assert_eq!(fetched.address().unwrap(), artifact.address().unwrap());
}
