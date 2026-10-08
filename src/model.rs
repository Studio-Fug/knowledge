use crate::{Error, Result, canonical, backing::{Role, Source}};
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    path::{Component, Path},
};

const SIGNING_CONTEXT: &[u8] = b"knowledge:public-artifact:v1\0";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Artifact {
    pub payload: Payload,
    pub signature: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Payload {
    pub version: u32,
    pub publisher: String,
    pub specification: Specification,
    pub design: Vec<Blob>,
    pub procedure: Vec<Step>,
    pub subject: PhysicalScope,
    pub evidence: Vec<Evidence>,
    pub records: Vec<Blob>,
    pub claims: Vec<Claim>,
    pub predecessors: Vec<Predecessor>,
    pub dependencies: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<Source>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Specification {
    pub title: String,
    pub requirements: Vec<Requirement>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Requirement {
    pub id: String,
    pub text: String,
    pub required_rigor: Option<Rigor>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Blob {
    pub path: String,
    /// Lowercase hexadecimal bytes. Binary designs and records are permitted.
    pub data: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Step {
    pub id: String,
    pub instructions: String,
    pub acceptance: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum Rigor {
    Analysis,
    Simulation,
    Software,
    Physical,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum ScopeKind {
    Design,
    Batch,
    Unit,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PhysicalScope {
    pub kind: ScopeKind,
    pub identifier: String,
    pub conditions: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Pass,
    Fail,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Evidence {
    pub id: String,
    pub subject_hash: String,
    pub requirements: Vec<String>,
    pub step: String,
    pub records: Vec<String>,
    pub outcome: Outcome,
    pub rigor: Rigor,
    pub reporter: String,
    pub origin: EvidenceOrigin,
    pub reproduction_notes: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceOrigin {
    Publisher,
    Recipient,
    ThirdParty,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ClaimKind {
    Capability,
    Constraint,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Claim {
    pub kind: ClaimKind,
    pub text: String,
    pub aliases: Vec<String>,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Predecessor {
    pub address: String,
    pub contribution: String,
    /// True only for a revision; conceptual sources and reproductions use false.
    pub revision: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Completeness {
    ReportedComplete,
    Incomplete,
    Failed,
}

#[derive(Debug, Clone, Serialize)]
pub struct ClaimSummary {
    pub kind: ClaimKind,
    pub text: String,
    pub status: &'static str,
    pub evidence: Vec<String>,
    pub rigor: Vec<Rigor>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Summary {
    pub address: String,
    pub publisher: String,
    pub title: String,
    pub completeness: Completeness,
    pub missing_requirements: Vec<String>,
    pub claims: Vec<ClaimSummary>,
    pub subject: PhysicalScope,
    pub signature_valid: bool,
    pub checking: &'static str,
    pub reusable: bool,
    pub backing_check: &'static str,
    pub source: String,
}

fn required(text: &str) -> Result<()> {
    if text.trim().is_empty() || text.len() > 8192 {
        return Err(Error::new(
            "invalid_artifact",
            "required text is empty or exceeds 8192 bytes",
        ));
    }
    Ok(())
}

fn identifiers<'a>(values: impl Iterator<Item = &'a str>) -> Result<BTreeSet<&'a str>> {
    let mut result = BTreeSet::new();
    for value in values {
        required(value)?;
        if !result.insert(value) {
            return Err(Error::new("invalid_artifact", "duplicate identifier"));
        }
    }
    Ok(result)
}

fn blobs(blobs: &[Blob]) -> Result<BTreeSet<&str>> {
    let paths = identifiers(blobs.iter().map(|b| b.path.as_str()))?;
    for blob in blobs {
        if blob.path.contains('\\')
            || blob
                .path
                .split('/')
                .any(|part| part.is_empty() || part == "." || part == "..")
            || !Path::new(&blob.path)
                .components()
                .all(|c| matches!(c, Component::Normal(_)))
        {
            return Err(Error::new(
                "invalid_artifact",
                "blob path must be relative without traversal",
            ));
        }
        if blob.data.len() % 2 != 0
            || !blob
                .data
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
        {
            return Err(Error::new(
                "invalid_artifact",
                "blob data must be lowercase hexadecimal",
            ));
        }
    }
    Ok(paths)
}

impl Payload {
    /// Excludes evidence, signatures, labels, and ancestry: no self-reference.
    pub fn subject_hash(&self) -> Result<String> {
        #[derive(Serialize)]
        struct Subject<'a> {
            context: &'static str,
            specification: &'a Specification,
            design: &'a [Blob],
            procedure: &'a [Step],
            subject: &'a PhysicalScope,
            dependencies: &'a [String],
            #[serde(skip_serializing_if = "Vec::is_empty")]
            sources: Vec<&'a Source>,
        }
        Ok(canonical::digest(&canonical::encode(&Subject {
            context: if self.version == 2 { "knowledge:verification-subject:v2" } else { "knowledge:verification-subject:v1" },
            specification: &self.specification,
            design: &self.design,
            procedure: &self.procedure,
            subject: &self.subject,
            dependencies: &self.dependencies,
            sources: self.sources.iter().filter(|s| s.role == Role::Design).collect(),
        })?))
    }

    pub fn validate(&self) -> Result<()> {
        if ![1, 2].contains(&self.version) || (self.version == 1 && !self.sources.is_empty()) {
            return Err(Error::new(
                "unsupported_version",
                "public versions 1 and 2 are supported; external sources require version 2",
            ));
        }
        if !canonical::is_hex(&self.publisher, 32) {
            return Err(Error::new(
                "invalid_key",
                "publisher must be a lowercase Ed25519 public key",
            ));
        }
        required(&self.specification.title)?;
        required(&self.subject.identifier)?;
        required(&self.subject.conditions)?;
        if self.specification.requirements.is_empty() || (self.design.is_empty() && !self.sources.iter().any(|s| s.role == Role::Design)) {
            return Err(Error::new(
                "invalid_artifact",
                "a specification and concrete design are required",
            ));
        }
        for count in [
            self.specification.requirements.len(),
            self.design.len(),
            self.procedure.len(),
            self.evidence.len(),
            self.records.len(),
            self.claims.len(),
            self.predecessors.len(),
            self.dependencies.len(),
            self.sources.len(),
        ] {
            if count > 256 {
                return Err(Error::new(
                    "too_large",
                    "an artifact section exceeds 256 entries",
                ));
            }
        }
        let requirements = identifiers(
            self.specification
                .requirements
                .iter()
                .map(|r| r.id.as_str()),
        )?;
        for r in &self.specification.requirements {
            required(&r.text)?;
        }
        let mut records = blobs(&self.records)?;
        let mut design = blobs(&self.design)?;
        for source in &self.sources {
            source.validate()?;
            let paths = if source.role == Role::Design { &mut design } else { &mut records };
            if !paths.insert(source.path.as_str()) { return Err(Error::new("invalid_source", "duplicate source or inline content path")); }
        }
        let steps = identifiers(self.procedure.iter().map(|s| s.id.as_str()))?;
        for step in &self.procedure {
            required(&step.instructions)?;
            required(&step.acceptance)?;
        }
        let evidence = identifiers(self.evidence.iter().map(|e| e.id.as_str()))?;
        for e in &self.evidence {
            if !canonical::is_hex(&e.subject_hash, 32)
                || e.requirements.is_empty()
                || e.records.is_empty()
            {
                return Err(Error::new(
                    "invalid_artifact",
                    "evidence needs a subject, requirement trace, and records",
                ));
            }
            if e.requirements.len() > 256 || e.records.len() > 256 {
                return Err(Error::new(
                    "too_large",
                    "evidence trace exceeds 256 entries",
                ));
            }
            let traced = identifiers(e.requirements.iter().map(String::as_str))?;
            let record_refs = identifiers(e.records.iter().map(String::as_str))?;
            if !traced.is_subset(&requirements)
                || !record_refs.is_subset(&records)
                || !steps.contains(e.step.as_str())
            {
                return Err(Error::new("invalid_artifact", "dangling evidence trace"));
            }
            if !canonical::is_hex(&e.reporter, 32) {
                return Err(Error::new(
                    "invalid_key",
                    "reporter must identify an Ed25519 public key",
                ));
            }
            // Reporter identity/independence are assertions, not authenticated attestations.
            if e.origin != EvidenceOrigin::Publisher {
                required(&e.reproduction_notes)?;
            }
        }
        for claim in &self.claims {
            required(&claim.text)?;
            if claim.aliases.len() > 64 || claim.evidence.len() > 256 {
                return Err(Error::new("too_large", "claim references exceed limits"));
            }
            for alias in &claim.aliases {
                required(alias)?;
            }
            let refs = identifiers(claim.evidence.iter().map(String::as_str))?;
            if !refs.is_subset(&evidence) {
                return Err(Error::new("invalid_artifact", "dangling claim evidence"));
            }
        }
        let deps = identifiers(self.dependencies.iter().map(String::as_str))?;
        let parents = identifiers(self.predecessors.iter().map(|p| p.address.as_str()))?;
        if deps
            .iter()
            .chain(parents.iter())
            .any(|h| !canonical::is_hex(h, 32))
        {
            return Err(Error::new(
                "invalid_address",
                "references must be lowercase SHA-256 addresses",
            ));
        }
        for parent in &self.predecessors {
            required(&parent.contribution)?;
        }
        Ok(())
    }
}

impl Artifact {
    pub fn seal(mut payload: Payload, key: &SigningKey) -> Result<Self> {
        payload.publisher = hex::encode(key.verifying_key().to_bytes());
        payload.validate()?;
        let message = Self::message(&payload)?;
        Ok(Self {
            payload,
            signature: hex::encode(key.sign(&message).to_bytes()),
        })
    }

    fn message(payload: &Payload) -> Result<Vec<u8>> {
        let hash = canonical::digest(&canonical::encode(payload)?);
        let mut message = if payload.version == 2 { b"knowledge:public-artifact:v2\0".to_vec() } else { SIGNING_CONTEXT.to_vec() };
        message.extend_from_slice(hash.as_bytes());
        Ok(message)
    }

    pub fn validate(&self) -> Result<()> {
        self.payload.validate()?;
        if !canonical::is_hex(&self.signature, 64) {
            return Err(Error::new(
                "invalid_signature",
                "signature must be 64 lowercase hex bytes",
            ));
        }
        let public: [u8; 32] = hex::decode(&self.payload.publisher)
            .map_err(|_| Error::new("invalid_key", "invalid public key"))?
            .try_into()
            .map_err(|_| Error::new("invalid_key", "invalid public key length"))?;
        let key = VerifyingKey::from_bytes(&public)
            .map_err(|_| Error::new("invalid_key", "invalid Ed25519 public key"))?;
        let signature = Signature::from_slice(
            &hex::decode(&self.signature)
                .map_err(|_| Error::new("invalid_signature", "invalid signature"))?,
        )
        .map_err(|_| Error::new("invalid_signature", "invalid signature length"))?;
        key.verify_strict(&Self::message(&self.payload)?, &signature)
            .map_err(|_| {
                Error::new(
                    "invalid_signature",
                    "signature does not authenticate payload",
                )
            })?;
        Ok(())
    }

    pub fn address(&self) -> Result<String> {
        Ok(canonical::digest(&canonical::encode(self)?))
    }

    pub fn summary(&self, trusted_publisher: Option<&str>, source: String) -> Result<Summary> {
        self.validate()?;
        let subject = self.payload.subject_hash()?;
        let applicable = |e: &&Evidence| e.subject_hash == subject;
        let failed = self
            .payload
            .evidence
            .iter()
            .filter(applicable)
            .any(|e| e.outcome == Outcome::Fail);
        let missing: Vec<String> = self
            .payload
            .specification
            .requirements
            .iter()
            .filter(|r| {
                !self.payload.evidence.iter().filter(applicable).any(|e| {
                    e.outcome == Outcome::Pass
                        && e.requirements.contains(&r.id)
                        && r.required_rigor.is_none_or(|minimum| e.rigor == minimum)
                })
            })
            .map(|r| r.id.clone())
            .collect();
        let completeness = if failed {
            Completeness::Failed
        } else if !missing.is_empty() || self.payload.procedure.is_empty() {
            Completeness::Incomplete
        } else {
            Completeness::ReportedComplete
        };
        let trusted = trusted_publisher == Some(self.payload.publisher.as_str());
        let claims = self
            .payload
            .claims
            .iter()
            .map(|claim| {
                let matching: Vec<_> = self
                    .payload
                    .evidence
                    .iter()
                    .filter(|e| claim.evidence.contains(&e.id))
                    .collect();
                let supported: Vec<_> = matching
                    .iter()
                    .filter(|e| e.subject_hash == subject && e.outcome == Outcome::Pass)
                    .collect();
                let status = if matching
                    .iter()
                    .any(|e| e.subject_hash == subject && e.outcome == Outcome::Fail)
                {
                    "failed"
                } else if supported.is_empty() {
                    "inferred"
                } else {
                    "publisher_reported_support"
                };
                ClaimSummary {
                    kind: claim.kind,
                    text: claim.text.clone(),
                    status,
                    evidence: claim.evidence.clone(),
                    rigor: supported.iter().map(|e| e.rigor).collect(),
                }
            })
            .collect();
        Ok(Summary {
            address: self.address()?,
            publisher: self.payload.publisher.clone(),
            title: self.payload.specification.title.clone(),
            reusable: trusted && completeness == Completeness::ReportedComplete && self.payload.sources.is_empty(),
            backing_check: if self.payload.sources.is_empty() { "inline_content_checked" } else { "external_content_unchecked; realize_before_use" },
            completeness,
            missing_requirements: missing,
            claims,
            subject: self.payload.subject.clone(),
            signature_valid: true,
            checking: if trusted {
                "explicit_publisher_trust; trace_structure_checked; tests_not_rerun"
            } else {
                "trace_structure_checked; claims_not_independently_checked"
            },
            source,
        })
    }
}
