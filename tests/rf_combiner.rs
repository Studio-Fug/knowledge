use knowledge::{
    canonical,
    model::{Artifact, Completeness},
    query::{self, Query},
    store::Cache,
};

#[test]
fn public_rf_candidate_preserves_failures_and_search_gating() {
    let artifact: Artifact =
        canonical::parse(include_bytes!("../examples/rf-combiner/artifact.json")).unwrap();
    artifact.validate().unwrap();
    assert_eq!(
        artifact.address().unwrap(),
        "9cdb703565b6e97f87064670c3646c4d566506af4e155964fe5069bd6695b88e"
    );
    assert_eq!(artifact.payload.version, 2);
    assert_eq!(artifact.payload.specification.requirements.len(), 15);
    assert_eq!(artifact.payload.evidence.len(), 10);
    assert_eq!(artifact.payload.sources.len(), 3);
    let summary = artifact.summary(None, "example".into()).unwrap();
    assert!(summary.signature_valid);
    assert_eq!(summary.completeness, Completeness::Failed);
    assert!(!summary.reusable);
    assert_eq!(
        summary.missing_requirements,
        [
            "REQ-1", "REQ-3", "REQ-6", "REQ-11", "REQ-12", "REQ-13", "REQ-14", "REQ-15"
        ]
    );
    let dir = tempfile::tempdir().unwrap();
    let cache = Cache::open(dir.path()).unwrap();
    cache.put(&artifact).unwrap();
    let mut query = Query {
        text: "28 GHz combiner".into(),
        limit: 10,
        ..Query::default()
    };
    assert!(query::search(&cache, &query).unwrap().matches.is_empty());
    query.include_incomplete = true;
    assert_eq!(query::search(&cache, &query).unwrap().matches.len(), 1);
}
