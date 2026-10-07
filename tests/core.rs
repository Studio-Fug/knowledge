use ed25519_dalek::SigningKey;
use knowledge::{canonical, model::*, query::{self, Query}, server, store::Cache};

fn fixture() -> Artifact {
    let key = SigningKey::from_bytes(&[7; 32]);
    let publisher = hex::encode(key.verifying_key().to_bytes());
    let mut payload = Payload {
        version: 1, publisher: publisher.clone(),
        specification: Specification { title: "Bucket".into(), requirements: vec![Requirement { id: "WATER".into(), text: "Move water".into(), minimum_rigor: Some(Rigor::Physical) }] },
        design: vec![Blob { path: "design.txt".into(), data: hex::encode("test fixture, not a real verified design") }],
        procedure: vec![Step { id: "TEST".into(), instructions: "Transfer the specified volume under the stated conditions".into(), acceptance: "Measured volume reaches destination".into() }],
        subject: PhysicalScope { kind: ScopeKind::Unit, identifier: "fixture-1".into(), conditions: "Synthetic test data only".into() },
        evidence: vec![], records: vec![Blob { path: "record.txt".into(), data: hex::encode("synthetic passing report") }],
        claims: vec![Claim { kind: ClaimKind::Capability, text: "Moves water".into(), aliases: vec!["method for moving water".into()], evidence: vec!["REPORT".into()] }],
        predecessors: vec![], dependencies: vec![],
    };
    payload.evidence.push(Evidence { id: "REPORT".into(), subject_hash: payload.subject_hash().unwrap(), requirements: vec!["WATER".into()], step: "TEST".into(), records: vec!["record.txt".into()], outcome: Outcome::Pass, rigor: Rigor::Physical, reporter: publisher, origin: EvidenceOrigin::Publisher, reproduction_notes: String::new() });
    Artifact::seal(payload, &key).unwrap()
}

#[test]
fn canonical_input_rejects_ambiguity() {
    let a: serde_json::Value = canonical::parse(br#"{"z":2,"a":1}"#).unwrap();
    let b: serde_json::Value = canonical::parse(br#"{ "a":1, "z":2 }"#).unwrap();
    assert_eq!(canonical::encode(&a).unwrap(), canonical::encode(&b).unwrap());
    assert!(canonical::parse::<serde_json::Value>(br#"{"a":1,"a":2}"#).is_err());
    assert!(canonical::parse::<serde_json::Value>(br#"{"a":1.5}"#).is_err());
    assert!(canonical::parse::<Artifact>(br#"{"payload":{},"signature":"","unhashed":true}"#).is_err());
}

#[test]
fn signature_hash_and_history_cover_every_field() {
    let artifact = fixture();
    artifact.validate().unwrap();
    let mut changed = artifact.clone();
    changed.payload.subject.conditions.push_str(" changed");
    assert!(changed.validate().is_err());
    let mut payload = artifact.payload.clone();
    payload.predecessors.push(Predecessor { address: artifact.address().unwrap(), contribution: "Display-only explanation".into(), revision: true });
    let next = Artifact::seal(payload, &SigningKey::from_bytes(&[7; 32])).unwrap();
    assert_ne!(artifact.address().unwrap(), next.address().unwrap());
    assert_eq!(artifact.payload.subject_hash().unwrap(), next.payload.subject_hash().unwrap());
}

#[test]
fn completeness_is_not_authenticity_and_stale_evidence_is_incomplete() {
    let artifact = fixture();
    let summary = artifact.summary(None, "test".into()).unwrap();
    assert_eq!(summary.completeness, Completeness::ReportedComplete);
    assert!(!summary.reusable);
    assert!(artifact.summary(Some(&artifact.payload.publisher), "test".into()).unwrap().reusable);
    let mut payload = artifact.payload.clone();
    payload.design[0].data = hex::encode("different design");
    let changed = Artifact::seal(payload, &SigningKey::from_bytes(&[7; 32])).unwrap();
    let summary = changed.summary(Some(&changed.payload.publisher), "test".into()).unwrap();
    assert_eq!(summary.completeness, Completeness::Incomplete);
    assert!(!summary.reusable);
    assert_eq!(summary.claims[0].status, "inferred");
    let mut payload = artifact.payload;
    payload.evidence[0].outcome = Outcome::Fail;
    let failed = Artifact::seal(payload, &SigningKey::from_bytes(&[7; 32])).unwrap();
    assert_eq!(failed.summary(None, "test".into()).unwrap().completeness, Completeness::Failed);
}

#[test]
fn cache_is_immutable_locked_and_detects_corruption() {
    let dir = tempfile::tempdir().unwrap();
    let cache = Cache::open(dir.path()).unwrap();
    assert!(Cache::open(dir.path()).is_err());
    let artifact = fixture();
    let address = cache.put(&artifact).unwrap();
    assert_eq!(cache.put(&artifact).unwrap(), address);
    assert_eq!(cache.get(&address).unwrap().address().unwrap(), address);
    assert!(cache.get("../../secret").is_err());
    std::fs::write(dir.path().join("objects").join(format!("{address}.json")), b"{}").unwrap();
    assert!(cache.get(&address).is_err());
    assert!(cache.put(&artifact).is_err());
}

#[test]
fn search_uses_labels_rigor_and_eligible_revision_history() {
    let dir = tempfile::tempdir().unwrap();
    let cache = Cache::open(dir.path()).unwrap();
    let original = fixture();
    let address = cache.put(&original).unwrap();
    let query = Query { text: "method for moving water".into(), minimum_rigor: Some(Rigor::Physical), supported_claims_only: true, exclude_superseded: true, ..Default::default() };
    let found = query::search(&cache, &query).unwrap();
    assert_eq!(found.matches.len(), 1);
    assert!(!found.matches[0].reusable);
    let mut payload = original.payload;
    payload.predecessors.push(Predecessor { address: address.clone(), contribution: "Changed design".into(), revision: true });
    payload.design[0].data = hex::encode("unverified revision");
    cache.put(&Artifact::seal(payload, &SigningKey::from_bytes(&[7; 32])).unwrap()).unwrap();
    let found = query::search(&cache, &query).unwrap();
    assert_eq!(found.matches.len(), 1);
    assert_eq!(found.matches[0].address, address);
    assert_eq!(query::search(&cache, &Query { include_incomplete: true, ..Default::default() }).unwrap().matches.len(), 2);
}

#[test]
fn http_is_loopback_read_only_and_substitution_checks_hash() {
    assert!(server::bind("0.0.0.0:0".parse().unwrap()).is_err());
    let server = server::bind("127.0.0.1:0".parse().unwrap()).unwrap();
    let origin = format!("http://{}", server.server_addr());
    let artifact = fixture();
    let address = artifact.address().unwrap();
    let bytes = canonical::encode(&artifact).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let worker = std::thread::spawn(move || {
        let cache = Cache::open(dir.path()).unwrap();
        cache.put(&artifact).unwrap();
        for _ in 0..3 { server::handle(server.recv().unwrap(), &cache, &[]).unwrap(); }
    });
    let response = ureq::post(&format!("{origin}/v1/artifacts")).send_bytes(&bytes);
    assert!(matches!(response, Err(ureq::Error::Status(403, _))));
    let client_dir = tempfile::tempdir().unwrap();
    let client = Cache::open(client_dir.path()).unwrap();
    assert_eq!(client.fetch(&address, std::slice::from_ref(&origin)).unwrap().address().unwrap(), address);
    assert!(client.fetch(&"0".repeat(64), &[origin]).is_err());
    worker.join().unwrap();
}
