use ed25519_dalek::SigningKey;
use flate2::{write::GzEncoder, Compression};
use knowledge::{backing::{self, Format, Role, Source}, canonical, model::{Artifact, Payload}, store::Cache};
use std::io::Write;

fn archive(entries: &[(&str, &[u8], u32)], modified: u64) -> Vec<u8> {
    let mut tar = tar::Builder::new(Vec::new());
    for (path, bytes, mode) in entries {
        let mut header = tar::Header::new_gnu();
        header.set_size(bytes.len() as u64);
        header.set_mode(*mode);
        header.set_mtime(modified);
        header.set_cksum();
        tar.append_data(&mut header, path, *bytes).unwrap();
    }
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&tar.into_inner().unwrap()).unwrap();
    encoder.finish().unwrap()
}

fn source(sha256: String) -> Source {
    let revision = "a".repeat(40);
    Source { path: "source".into(), role: Role::Design, repository: "https://github.com/example/design".into(), archive: format!("https://codeload.github.com/example/design/tar.gz/{revision}"), revision, subdirectory: "src".into(), format: Format::GitTarGzipV1, sha256 }
}

#[test]
fn realization_is_order_and_transport_metadata_independent_but_content_sensitive() {
    let a = archive(&[("repo/src/b.rs", b"b", 0o644), ("repo/src/a.rs", b"a", 0o755)], 1);
    let b = archive(&[("different-root/src/a.rs", b"a", 0o700), ("different-root/src/b.rs", b"b", 0o600)], 2);
    let realized = backing::realize(&a, "src").unwrap();
    assert_eq!(realized.sha256, backing::realize(&b, "src").unwrap().sha256);
    source(realized.sha256.clone()).check(&a).unwrap();
    let changed = archive(&[("repo/src/a.rs", b"changed", 0o755), ("repo/src/b.rs", b"b", 0o644)], 1);
    assert!(source(realized.sha256.clone()).check(&changed).is_err());
    let changed_mode = archive(&[("repo/src/a.rs", b"a", 0o644), ("repo/src/b.rs", b"b", 0o644)], 1);
    assert_ne!(realized.sha256, backing::realize(&changed_mode, "src").unwrap().sha256);
}

#[test]
fn descriptors_are_pinned_hashed_and_fetches_need_explicit_host_permission() {
    let bytes = archive(&[("repo/src/main.rs", b"fn main() {}", 0o644)], 1);
    let descriptor = source(backing::realize(&bytes, "src").unwrap().sha256);
    descriptor.validate().unwrap();
    assert_eq!(descriptor.fetch(&[]).unwrap_err().code, "source_forbidden");
    let mut mutable = descriptor.clone(); mutable.revision = "main".into();
    assert!(mutable.validate().is_err());
    let mut credential = descriptor.clone(); credential.archive = credential.archive.replace("https://", "https://secret@");
    assert!(credential.validate().is_err());
    let mut payload: Payload = canonical::parse(include_bytes!("../examples/payload.json")).unwrap();
    payload.version = 2; payload.design.clear(); payload.sources.push(descriptor);
    let key = SigningKey::from_bytes(&[7; 32]);
    let artifact = Artifact::seal(payload.clone(), &key).unwrap();
    artifact.validate().unwrap();
    let summary = artifact.summary(Some(&artifact.payload.publisher), "test".into()).unwrap();
    assert!(!summary.reusable);
    assert!(summary.backing_check.contains("unchecked"));
    payload.sources[0].subdirectory = "other".into();
    let changed = Artifact::seal(payload, &key).unwrap();
    assert_ne!(artifact.address().unwrap(), changed.address().unwrap());
    assert_ne!(artifact.payload.subject_hash().unwrap(), changed.payload.subject_hash().unwrap());
}

#[test]
fn cache_persists_only_the_signed_manifest_and_exports_checked_content_separately() {
    let data = "large fixture".repeat(10000);
    let archive = archive(&[("repo/src/main.rs", data.as_bytes(), 0o644)], 1);
    let mut payload: Payload = canonical::parse(include_bytes!("../examples/payload.json")).unwrap();
    payload.version = 2; payload.design.clear(); payload.sources.push(source(backing::realize(&archive, "src").unwrap().sha256));
    let artifact = Artifact::seal(payload, &SigningKey::from_bytes(&[7; 32])).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let cache = Cache::open(directory.path()).unwrap();
    let address = cache.put(&artifact).unwrap();
    let path = directory.path().join("objects").join(format!("{address}.json"));
    assert!(std::fs::metadata(path).unwrap().len() < 4096);
    drop(cache);
    let cache = Cache::open(directory.path()).unwrap();
    let stored = cache.get(&address).unwrap();
    let checked = stored.payload.sources[0].check(&archive).unwrap();
    let output = directory.path().join("realized");
    backing::export(&[("design/source".into(), checked)], &output).unwrap();
    assert_eq!(std::fs::read_to_string(output.join("design/source/main.rs")).unwrap(), data);
    assert!(backing::export(&[], &output).is_err());
    std::fs::remove_dir_all(output).unwrap();
    assert_eq!(cache.get(&address).unwrap().address().unwrap(), address);
}

#[test]
fn malformed_archives_do_not_escape_or_publish_partial_output() {
    let duplicate = archive(&[("repo/src/a", b"a", 0o644), ("repo/src/a", b"b", 0o644)], 1);
    assert!(backing::realize(&duplicate, "src").is_err());
    let collision = archive(&[("repo/src/a", b"a", 0o644), ("repo/src/a/b", b"b", 0o644)], 1);
    assert!(backing::realize(&collision, "src").is_err());
    let mut tar = tar::Builder::new(Vec::new());
    let mut header = tar::Header::new_gnu();
    header.set_entry_type(tar::EntryType::Symlink); header.set_size(0); header.set_mode(0o777); header.set_link_name("../../escape").unwrap(); header.set_cksum();
    tar.append_data(&mut header, "repo/src/link", std::io::empty()).unwrap();
    let mut gzip = GzEncoder::new(Vec::new(), Compression::default());
    gzip.write_all(&tar.into_inner().unwrap()).unwrap();
    assert!(backing::realize(&gzip.finish().unwrap(), "src").is_err());
    assert!(!backing::safe_path("../escape"));
    assert!(!backing::safe_path("C:/escape"));
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("output");
    let tree = backing::realize(&archive(&[("repo/src/a", b"a", 0o644)], 1), "src").unwrap();
    assert!(backing::export(&[("../escape".into(), tree)], &output).is_err());
    assert!(!output.exists());
}

#[test]
fn report_sources_do_not_create_circular_subjects_and_complete_external_artifacts_stay_unchecked() {
    use knowledge::model::{Completeness, Evidence, EvidenceOrigin, Outcome, Rigor, Step};
    let bytes = archive(&[("repo/src/report.txt", b"synthetic passing report", 0o644)], 1);
    let mut descriptor = source(backing::realize(&bytes, "src").unwrap().sha256);
    descriptor.role = Role::Record;
    let mut payload: Payload = canonical::parse(include_bytes!("../examples/payload.json")).unwrap();
    payload.version = 2;
    let subject = payload.subject_hash().unwrap();
    payload.sources.push(descriptor);
    assert_eq!(subject, payload.subject_hash().unwrap());
    payload.procedure.push(Step {id:"TEST".into(),instructions:"Synthetic test procedure".into(),acceptance:"Synthetic result".into()});
    payload.evidence.push(Evidence {id:"REPORT".into(),subject_hash:payload.subject_hash().unwrap(),requirements:vec!["WATER".into()],step:"TEST".into(),records:vec!["source".into()],outcome:Outcome::Pass,rigor:Rigor::Software,reporter:hex::encode(SigningKey::from_bytes(&[7;32]).verifying_key().to_bytes()),origin:EvidenceOrigin::Publisher,reproduction_notes:String::new()});
    let artifact = Artifact::seal(payload.clone(), &SigningKey::from_bytes(&[7;32])).unwrap();
    let summary = artifact.summary(Some(&artifact.payload.publisher), "test".into()).unwrap();
    assert_eq!(summary.completeness, Completeness::ReportedComplete);
    assert!(!summary.reusable);
    payload.sources[0].sha256 = "b".repeat(64);
    let changed = Artifact::seal(payload, &SigningKey::from_bytes(&[7;32])).unwrap();
    assert_eq!(artifact.payload.subject_hash().unwrap(), changed.payload.subject_hash().unwrap());
    assert_ne!(artifact.address().unwrap(), changed.address().unwrap());
    assert!(changed.payload.sources[0].check(&bytes).is_err());
}

#[test]
#[ignore = "requires public GitHub archive network access"]
fn pinned_github_archive_is_realized_and_checked() {
    use std::io::Read;
    let revision = "3e2664811ce919752e5de4e8668724198f4f2bca";
    let url = format!("https://codeload.github.com/Studio-Fug/knowledge/tar.gz/{revision}");
    let mut bytes = Vec::new();
    ureq::AgentBuilder::new().timeout(std::time::Duration::from_secs(30)).redirects(0).build().get(&url).call().unwrap().into_reader().take(32*1024*1024+1).read_to_end(&mut bytes).unwrap();
    let realization = backing::realize(&bytes, "examples").unwrap();
    println!("PINNED_GITHUB_REALIZATION:{}", realization.sha256);
    assert!(realization.files.iter().any(|file| file.path == "payload.json"));
    let descriptor = Source {path:"example".into(),role:Role::Design,repository:"https://github.com/Studio-Fug/knowledge".into(),revision:revision.into(),archive:url,subdirectory:"examples".into(),format:Format::GitTarGzipV1,sha256:realization.sha256};
    descriptor.fetch(&["https://codeload.github.com".into()]).unwrap();
}
