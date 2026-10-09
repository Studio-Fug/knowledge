//! Derived, content-committed search data; never publisher verification evidence.
use crate::{canonical, model::Artifact, query::{self, Mode, Query, Search}, store::Cache, Error, Result};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs::{self, File}, io::{Read, Write}, path::PathBuf, time::Duration};

const CHUNK_BYTES: usize = 1024;
const MAX_DIMENSIONS: usize = 4096;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Recipe {
    pub endpoint: String,
    pub model: String,
    pub revision: String,
    pub format: String,
}
impl Recipe {
    pub fn new(endpoint: String, model: String, revision: String) -> Self {
        Self { endpoint, model, revision, format: "canonical-json-or-query-utf8;1024-byte-chunks;unit-mean-unit-f32:v1".into() }
    }
}

pub trait Embedder {
    fn recipe(&self) -> &Recipe;
    fn embed(&self, inputs: &[String]) -> Result<Vec<Vec<f32>>>;
}

pub struct Ollama {
    recipe: Recipe,
    agent: ureq::Agent,
}
impl Ollama {
    pub fn connect(endpoint: &str, model: &str) -> Result<Self> {
        let url = url::Url::parse(endpoint).map_err(|_| Error::new("invalid_embedding_config", "embedding endpoint must be an HTTP(S) origin"))?;
        if !["http", "https"].contains(&url.scheme()) || url.host_str().is_none() || url.path() != "/" || !url.username().is_empty() || url.password().is_some() || url.query().is_some() || url.fragment().is_some() || model.is_empty() || model.len() > 256 {
            return Err(Error::new("invalid_embedding_config", "embedding endpoint must be a credential-free HTTP(S) origin and model must be specified"));
        }
        let agent = ureq::AgentBuilder::new().timeout(Duration::from_secs(60)).redirects(0).build();
        let mut backend = Self {recipe: Recipe::new(endpoint.trim_end_matches('/').into(),model.into(),String::new()),agent};
        let (name, revision) = backend.identity()?;
        backend.recipe.model = name;
        backend.recipe.revision = revision;
        Ok(backend)
    }
    fn read(response: ureq::Response) -> Result<serde_json::Value> {
        if response.status() != 200 { return Err(Error::new("embedding_unavailable", "embedding service returned a non-200 response")); }
        let mut bytes = Vec::new();
        response.into_reader().take((canonical::MAX_BYTES+1) as u64).read_to_end(&mut bytes)?;
        canonical::parse_request(&bytes)
    }
    fn identity(&self) -> Result<(String,String)> {
        let value = Self::read(self.agent.get(&format!("{}/api/tags",self.recipe.endpoint)).call().map_err(|_| Error::new("embedding_unavailable", "embedding model service is unavailable"))?)?;
        let models = value["models"].as_array().ok_or_else(|| Error::new("invalid_embedding", "model service returned no model identities"))?;
        let requested = &self.recipe.model;
        for model in models {
            let name = model["name"].as_str().unwrap_or("");
            if name == requested || (!requested.contains(':') && name == format!("{requested}:latest")) {
                let digest = model["digest"].as_str().unwrap_or("");
                if !canonical::is_hex(digest,32) { return Err(Error::new("invalid_embedding", "model service did not provide a SHA-256 model revision")); }
                return Ok((name.into(),digest.into()));
            }
        }
        Err(Error::new("embedding_unavailable", "configured embedding model is not installed"))
    }
}
impl Embedder for Ollama {
    fn recipe(&self) -> &Recipe { &self.recipe }
    fn embed(&self, inputs: &[String]) -> Result<Vec<Vec<f32>>> {
        if self.identity()?.1 != self.recipe.revision { return Err(Error::new("embedding_model_changed", "model revision changed; restart/reindex to use the new revision")); }
        let body = serde_json::to_string(&serde_json::json!({"model":self.recipe.model,"input":inputs,"truncate":false}))?;
        let response = self.agent.post(&format!("{}/api/embed",self.recipe.endpoint)).set("Content-Type","application/json").send_string(&body).map_err(|_| Error::new("embedding_unavailable", "embedding request failed; input was not silently truncated"))?;
        let value = Self::read(response)?;
        let returned=value["model"].as_str();
        let matching=returned.is_none_or(|name| name==self.recipe.model || self.recipe.model.strip_suffix(":latest")==Some(name));
        if !matching || self.identity()?.1 != self.recipe.revision { return Err(Error::new("embedding_model_changed", "embedding response does not match the pinned model")); }
        serde_json::from_value(value["embeddings"].clone()).map_err(|_| Error::new("invalid_embedding", "embedding service returned malformed vectors"))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Record {
    pub recipe: Recipe,
    pub input_sha256: String,
    pub input_kind: String,
    pub chunks: usize,
    /// Exact IEEE-754 f32 bits keep hashed records integer-only.
    pub vector_bits: Vec<u32>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt { pub sha256: String, pub record: Record }
impl Receipt {
    pub fn validate(&self) -> Result<()> {
        if canonical::digest(&canonical::encode(&self.record)?) != self.sha256 || !canonical::is_hex(&self.record.input_sha256,32) || self.record.chunks == 0 || self.record.chunks > 4096 {
            return Err(Error::new("invalid_embedding", "embedding record commitment is invalid"));
        }
        let vector = self.vector();
        let magnitude = norm(&vector)?;
        if (magnitude-1.0).abs() > 0.0001 { return Err(Error::new("invalid_embedding", "stored embedding is not normalized")); }
        Ok(())
    }
    fn vector(&self) -> Vec<f32> { self.record.vector_bits.iter().map(|b| f32::from_bits(*b)).collect() }
}
fn norm(vector: &[f32]) -> Result<f64> {
    if vector.is_empty() || vector.len() > MAX_DIMENSIONS || vector.iter().any(|v| !v.is_finite()) { return Err(Error::new("invalid_embedding", "vector is empty, non-finite or oversized")); }
    let magnitude = vector.iter().map(|v| f64::from(*v).powi(2)).sum::<f64>().sqrt();
    if !magnitude.is_finite() || magnitude == 0.0 { return Err(Error::new("invalid_embedding", "zero or invalid vector norm")); }
    Ok(magnitude)
}
fn chunks(text: &str) -> Vec<String> {
    let mut remaining = text;
    let mut inputs = Vec::new();
    while !remaining.is_empty() {
        let mut end = remaining.len().min(CHUNK_BYTES);
        while !remaining.is_char_boundary(end) { end-=1; }
        inputs.push(remaining[..end].to_owned());
        remaining=&remaining[end..];
    }
    inputs
}
fn produce(provider: &dyn Embedder, text: &str, kind: &str) -> Result<Receipt> {
    if text.is_empty() || text.len()>canonical::MAX_BYTES { return Err(Error::new("too_large", "embedding input is empty or exceeds 4 MiB")); }
    let inputs = chunks(text);
    let mut sum = Vec::<f64>::new();
    for batch in inputs.chunks(16) {
        let vectors = provider.embed(batch)?;
        if vectors.len()!=batch.len() { return Err(Error::new("invalid_embedding", "embedding service dropped input chunks")); }
        for vector in vectors {
            let magnitude = norm(&vector)?;
            if sum.is_empty() { sum.resize(vector.len(),0.0); }
            if vector.len()!=sum.len() { return Err(Error::new("invalid_embedding", "embedding dimensions changed")); }
            for (total,value) in sum.iter_mut().zip(vector) { *total+=f64::from(value)/magnitude; }
        }
    }
    let vector: Vec<f32> = sum.iter().map(|v| (*v/inputs.len() as f64) as f32).collect();
    let magnitude=norm(&vector)?;
    let record=Record {recipe:provider.recipe().clone(),input_sha256:canonical::digest(text.as_bytes()),input_kind:kind.into(),chunks:inputs.len(),vector_bits:vector.iter().map(|v| ((f64::from(*v)/magnitude) as f32).to_bits()).collect()};
    let receipt=Receipt {sha256:canonical::digest(&canonical::encode(&record)?),record};
    receipt.validate()?;
    Ok(receipt)
}

#[derive(Serialize)]
pub struct IndexStatus {
    pub enabled: bool,
    pub indexed: usize,
    pub total: usize,
    pub failures: BTreeMap<String,String>,
    pub recipe: Option<Recipe>,
}
#[derive(Serialize)]
pub struct SearchEvidence {
    pub sha256: String,
    #[serde(flatten)]
    pub record: SearchRecord,
}
#[derive(Serialize)]
pub struct SearchRecord {
    pub query: String,
    pub filters: serde_json::Value,
    pub scope_artifacts: Vec<String>,
    pub threshold_bits: u64,
    pub query_embedding: Receipt,
    pub object_embeddings: BTreeMap<String,Receipt>,
    pub skipped: BTreeMap<String,String>,
    pub qualifying_claim_embeddings: BTreeMap<String,Receipt>,
    pub considered_embeddings: BTreeMap<String,String>,
    pub scored_claim_embeddings: BTreeMap<String,String>,
    pub scores_bits: BTreeMap<String,u64>,
    pub scoring: &'static str,
}

pub struct Index {
    provider: Box<dyn Embedder>,
    directory: PathBuf,
    records: BTreeMap<String,Receipt>,
    failures: BTreeMap<String,String>,
}
fn regular(path: &std::path::Path) -> Result<()> {
    if fs::symlink_metadata(path)?.file_type().is_symlink() { return Err(Error::new("unsafe_path", "embedding paths cannot be symbolic links")); }
    Ok(())
}
impl Index {
    pub fn open(cache: &Cache, provider: Box<dyn Embedder>) -> Result<Self> {
        let root=cache.root().join("embeddings");
        fs::create_dir_all(&root)?; regular(&root)?;
        let directory=root.join(canonical::digest(&canonical::encode(provider.recipe())?));
        fs::create_dir_all(&directory)?; regular(&directory)?;
        Ok(Self {provider,directory,records:BTreeMap::new(),failures:BTreeMap::new()})
    }
    pub fn configured(cache: &Cache) -> Result<Option<Self>> {Self::configure(cache,false)}
    pub fn configure(cache: &Cache, rebuild: bool) -> Result<Option<Self>> {
        let endpoint=std::env::var("KNOWLEDGE_EMBEDDING_ENDPOINT").unwrap_or_default();
        let model=std::env::var("KNOWLEDGE_EMBEDDING_MODEL").unwrap_or_default();
        if endpoint.is_empty() && model.is_empty() { return Ok(None); }
        let provider=Ollama::connect(&endpoint,&model)?;
        let mut index=Self::open(cache,Box::new(provider))?;
        if rebuild {
            for entry in fs::read_dir(&index.directory)? {
                let path=entry?.path();regular(&path)?;
                if path.is_file() {fs::remove_file(path)?;}
            }
        }
        index.backfill(cache)?;
        Ok(Some(index))
    }
    pub fn backfill(&mut self, cache: &Cache) -> Result<()> {
        for address in cache.addresses()? { self.ingest(&cache.get(&address)?); }
        Ok(())
    }
    pub fn ingest(&mut self, artifact: &Artifact) -> bool {
        let address=match artifact.validate().and_then(|_| artifact.address()) {Ok(address)=>address,Err(_)=>return false};
        let attempt=(|| -> Result<Receipt> {
            let path=self.directory.join(format!("{address}.json"));
            if path.try_exists()? {
                regular(&path)?;
                let mut bytes=Vec::new(); File::open(&path)?.take((canonical::MAX_BYTES+1) as u64).read_to_end(&mut bytes)?;
                let receipt: Receipt=canonical::parse(&bytes)?;
                receipt.validate()?;
                if receipt.record.recipe!=*self.provider.recipe() || receipt.record.input_sha256!=address || receipt.record.input_kind!="artifact" { return Err(Error::new("invalid_embedding", "embedding belongs to a different artifact or model")); }
                return Ok(receipt);
            }
            let bytes=canonical::encode(artifact)?;
            let text=std::str::from_utf8(&bytes).map_err(|_| Error::new("invalid_embedding", "canonical artifact is not UTF-8"))?;
            let receipt=produce(self.provider.as_ref(),text,"artifact")?;
            self.install(&path,&receipt)?;
            Ok(receipt)
        })();
        match attempt {
            Ok(receipt)=>{self.failures.remove(&address);self.records.insert(address,receipt);true}
            Err(error)=>{self.records.remove(&address);self.failures.insert(address,error.code.into());false}
        }
    }
    fn install(&self,path: &std::path::Path,receipt: &Receipt) -> Result<()> {
        let bytes=canonical::encode(receipt)?;
        let mut size=0u64;
        for recipe in fs::read_dir(self.directory.parent().ok_or_else(|| Error::new("unsafe_path","missing embedding parent"))?)? {
            let directory=recipe?.path();regular(&directory)?;
            if !directory.is_dir() {continue;}
            for entry in fs::read_dir(directory)? {
                let path=entry?.path();regular(&path)?;
                size=size.checked_add(fs::metadata(path)?.len()).ok_or_else(|| Error::new("index_full","embedding storage size overflow"))?;
            }
        }
        if size+bytes.len() as u64>128*1024*1024 {return Err(Error::new("index_full","embedding index exceeds 128 MiB across model revisions"));}
        let mut temp=tempfile::NamedTempFile::new_in(&self.directory)?;
        temp.write_all(&bytes)?;temp.as_file().sync_all()?;
        temp.persist_noclobber(path).map_err(|_| Error::new("io","embedding record installation failed"))?;
        #[cfg(unix)] File::open(&self.directory)?.sync_all()?;
        Ok(())
    }
    fn claim_embedding(&self, address: &str, position: usize, claim: &crate::model::Claim) -> Result<Receipt> {
        let bytes=canonical::encode(&serde_json::json!({"artifact_sha256":address,"claim_index":position,"claim":claim}))?;
        let input_sha256=canonical::digest(&bytes);
        let path=self.directory.join(format!("{address}-claim-{position}.json"));
        if path.try_exists()? {
            regular(&path)?;
            let mut bytes=Vec::new(); File::open(path)?.take((canonical::MAX_BYTES+1) as u64).read_to_end(&mut bytes)?;
            let receipt: Receipt=canonical::parse(&bytes)?; receipt.validate()?;
            if receipt.record.recipe!=*self.provider.recipe() || receipt.record.input_sha256!=input_sha256 || receipt.record.input_kind!="claim" { return Err(Error::new("invalid_embedding","claim embedding commitment is stale")); }
            return Ok(receipt);
        }
        let text=std::str::from_utf8(&bytes).map_err(|_| Error::new("invalid_embedding","claim is not UTF-8"))?;
        let receipt=produce(self.provider.as_ref(),text,"claim")?;
        self.install(&path,&receipt)?;
        Ok(receipt)
    }
    pub fn status(&self, total: usize) -> IndexStatus {
        IndexStatus {enabled:true,indexed:self.records.len(),total,failures:self.failures.clone(),recipe:Some(self.provider.recipe().clone())}
    }
    pub fn search(&self, cache: &Cache, query: &Query) -> Result<Search> {
        if query.text.trim().is_empty() { return Err(Error::new("invalid_query", "semantic search requires nonempty query text")); }
        let (candidates,scanned)=query::candidates(cache,query,false)?;
        let embedding=produce(self.provider.as_ref(),&query.text,"query")?;
        let query_vector=embedding.vector();
        let threshold=query.threshold.unwrap_or(0.2);
        let mut eligible=Vec::new(); let mut skipped=BTreeMap::new();
        let mut qualifying=BTreeMap::new();
        let mut considered=BTreeMap::new();
        let mut scored_claims=BTreeMap::new();
        let mut scores=BTreeMap::new();
        for (artifact,mut summary) in candidates {
            let Some(object)=self.records.get(&summary.address) else {skipped.insert(summary.address,self.failures.get(&artifact.address()?).cloned().unwrap_or_else(|| "not_indexed".into()));continue;};
            considered.insert(summary.address.clone(),object.sha256.clone());
            let vector=object.vector();
            if vector.len()!=query_vector.len() { return Err(Error::new("invalid_embedding", "query and object embedding dimensions differ")); }
            let mut score=cosine(&query_vector,&vector)?;
            if query.supported_claims_only || query.rigor.is_some() {
                let mut best=None::<(f64,Receipt)>;
                for (position,(claim,status)) in artifact.payload.claims.iter().zip(&summary.claims).enumerate() {
                    if (query.supported_claims_only && status.status!="publisher_reported_support") || query.rigor.is_some_and(|rigor| !status.rigor.contains(&rigor)) {continue;}
                    let receipt=self.claim_embedding(&summary.address,position,claim)?;
                    let similarity=cosine(&query_vector,&receipt.vector())?;
                    if best.as_ref().is_none_or(|(old,_)| similarity>*old) {best=Some((similarity,receipt));}
                }
                let Some((similarity,receipt))=best else {continue;};
                score=similarity;
                scored_claims.insert(summary.address.clone(),receipt.sha256.clone());
                qualifying.insert(summary.address.clone(),receipt);
            }
            scores.insert(summary.address.clone(),score.to_bits());
            if score>=threshold {summary.similarity=Some(score);eligible.push((artifact,summary));}
        }
        eligible.sort_by(|(_,a),(_,b)| b.similarity.partial_cmp(&a.similarity).unwrap_or(std::cmp::Ordering::Equal).then_with(|| a.address.cmp(&b.address)));
        let matches=query::select(eligible,query);
        qualifying.retain(|address,_| matches.iter().any(|summary| &summary.address==address));
        let records=matches.iter().map(|s|(s.address.clone(),self.records[&s.address].clone())).collect();
        let mut filters=serde_json::to_value(query)?;
        filters.as_object_mut().ok_or_else(|| Error::new("invalid_query","query is not an object"))?.remove("threshold");
        let record=SearchRecord {query:query.text.clone(),filters,scope_artifacts:cache.addresses()?,threshold_bits:threshold.to_bits(),query_embedding:embedding,object_embeddings:records,skipped,qualifying_claim_embeddings:qualifying,considered_embeddings:considered,scored_claim_embeddings:scored_claims,scores_bits:scores,scoring:"inclusive cosine threshold; qualifying claim vectors for rigor/support filters, whole-object vector otherwise; descending similarity; address tie-break"};
        let sha256=canonical::digest(&canonical::encode(&record)?);
        Ok(Search {matches,scanned,scope:"local_cache; semantic_similarity; publisher_assertions; eligible_revisions_only",semantic:Some(SearchEvidence {sha256,record})})
    }
}

pub fn search(cache: &Cache, query: &Query, index: Option<&Index>) -> Result<Search> {
    match query.mode {
        Some(Mode::Lexical)=>query::search(cache,query),
        Some(Mode::Semantic)=>index.ok_or_else(|| Error::new("embedding_unavailable", "semantic embeddings are not configured"))?.search(cache,query),
        None if index.is_some() && !query.text.trim().is_empty()=>index.unwrap().search(cache,query),
        None if query.threshold.is_some()=>Err(Error::new("embedding_unavailable", "a similarity threshold requires configured semantic embeddings")),
        None=>query::search(cache,query),
    }
}

fn cosine(a: &[f32], b: &[f32]) -> Result<f64> {
    if a.len()!=b.len() {return Err(Error::new("invalid_embedding","embedding dimensions differ"));}
    Ok((a.iter().zip(b).map(|(a,b)|f64::from(*a)*f64::from(*b)).sum::<f64>()/(norm(a)?*norm(b)?)).clamp(-1.0,1.0))
}
