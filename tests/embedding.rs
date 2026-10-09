use ed25519_dalek::SigningKey;
use knowledge::{canonical,embedding::{self,Embedder,Index,Recipe},model::{Artifact,Payload},query::{Mode,Query},store::Cache,Result};
use std::sync::{Arc,atomic::{AtomicUsize,Ordering}};
struct Mock {recipe:Recipe,calls:Arc<AtomicUsize>,bad:bool}
impl Embedder for Mock {
 fn recipe(&self)->&Recipe {&self.recipe}
 fn embed(&self,inputs:&[String])->Result<Vec<Vec<f32>>> {
  self.calls.fetch_add(inputs.len(),Ordering::SeqCst);
  Ok(inputs.iter().map(|text| if self.bad {vec![0.0,0.0]} else if text.contains("bucket") || text.contains("moving water") {vec![1.0,0.0]} else {vec![0.0,1.0]}).collect())
 }
}
fn provider(calls:Arc<AtomicUsize>,revision:&str,bad:bool)->Box<dyn Embedder> {Box::new(Mock {recipe:Recipe::new("mock://local".into(),"test".into(),revision.into()),calls,bad})}
fn artifact(title:&str)->Artifact {
 let mut p:Payload=canonical::parse(include_bytes!("../examples/payload.json")).unwrap();
 p.specification.title=title.into();p.claims.clear();p.subject.identifier="test-subject".into();p.specification.requirements[0].text="Carry liquids".into();
 Artifact::seal(p,&SigningKey::from_bytes(&[7;32])).unwrap()
}
fn query()->Query {Query {text:"moving water".into(),mode:Some(Mode::Semantic),threshold:Some(0.9),include_incomplete:true,..Query::default()}}
#[test]
fn nonlexical_similarity_is_thresholded_and_survives_restart_without_reembedding_objects() {
 let root=tempfile::tempdir().unwrap();let cache=Cache::open(root.path()).unwrap();
 let bucket=artifact("bucket");let lamp=artifact("lamp");let address=cache.put(&bucket).unwrap();cache.put(&lamp).unwrap();
 let calls=Arc::new(AtomicUsize::new(0));let mut index=Index::open(&cache,provider(calls.clone(),"a",false)).unwrap();index.backfill(&cache).unwrap();
 assert_eq!(calls.load(Ordering::SeqCst),2);
 let result=index.search(&cache,&query()).unwrap();assert_eq!(result.matches.len(),1);assert_eq!(result.matches[0].address,address);assert!(!result.matches[0].reusable);
 let evidence=result.semantic.unwrap();evidence.query_embedding.validate().unwrap();evidence.object_embeddings[&address].validate().unwrap();
 assert!(knowledge::query::search(&cache,&query()).unwrap().matches.is_empty());
 let before=calls.load(Ordering::SeqCst);drop(index);
 let mut index=Index::open(&cache,provider(calls.clone(),"a",false)).unwrap();index.backfill(&cache).unwrap();assert_eq!(calls.load(Ordering::SeqCst),before);
 let filtered=Query {include_incomplete:false,..query()};assert!(index.search(&cache,&filtered).unwrap().matches.is_empty());
 let mut changed=Index::open(&cache,provider(calls.clone(),"b",false)).unwrap();changed.backfill(&cache).unwrap();assert_eq!(calls.load(Ordering::SeqCst),before+3);
 let invalid=Query {threshold:Some(1.1),..query()};assert!(index.search(&cache,&invalid).is_err());
 assert!(embedding::search(&cache,&query(),None).is_err());
}
#[test]
fn malformed_vectors_and_tampered_records_are_visible_and_never_used() {
 let root=tempfile::tempdir().unwrap();let cache=Cache::open(root.path()).unwrap();let address=cache.put(&artifact("bucket")).unwrap();
 let calls=Arc::new(AtomicUsize::new(0));let mut index=Index::open(&cache,provider(calls.clone(),"a",true)).unwrap();index.backfill(&cache).unwrap();assert_eq!(index.status(1).indexed,0);assert_eq!(index.status(1).failures[&address],"invalid_embedding");
 let mut index=Index::open(&cache,provider(calls.clone(),"a",false)).unwrap();index.backfill(&cache).unwrap();
 let recipe=canonical::digest(&canonical::encode(&provider(calls.clone(),"a",false).recipe()).unwrap());
 let path=root.path().join("embeddings").join(recipe).join(format!("{address}.json"));let mut record:serde_json::Value=serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();record["record"]["vector_bits"][0]=0.into();std::fs::write(&path,serde_json::to_vec(&record).unwrap()).unwrap();
 let mut restarted=Index::open(&cache,provider(calls,"a",false)).unwrap();restarted.backfill(&cache).unwrap();let search=restarted.search(&cache,&query()).unwrap();assert!(search.matches.is_empty());assert_eq!(search.semantic.unwrap().skipped[&address],"invalid_embedding");assert_eq!(cache.get(&address).unwrap().address().unwrap(),address);
}
#[test]
fn api_decimals_do_not_change_artifact_canonicalization_or_allow_duplicate_keys() {
 let query:Query=canonical::parse_request(br#"{"threshold":0.7}"#).unwrap();assert_eq!(query.threshold,Some(0.7));
 assert!(canonical::parse::<serde_json::Value>(br#"{"threshold":0.7}"#).is_err());
 assert!(canonical::parse_request::<Query>(br#"{"threshold":0.7,"threshold":0.8}"#).is_err());
}

#[test]
fn supported_and_rigor_queries_match_the_qualifying_claim_not_an_unverified_neighbor() {
 use knowledge::model::{Claim,ClaimKind,Evidence,EvidenceOrigin,Outcome,Rigor,Step,Blob};
 let root=tempfile::tempdir().unwrap();let cache=Cache::open(root.path()).unwrap();
 let mut p=artifact("lamp").payload;
 p.procedure.push(Step {id:"TEST".into(),instructions:"Synthetic fixture".into(),acceptance:"Light emitted".into()});
 p.records.push(Blob {path:"report.txt".into(),data:hex::encode("synthetic passing report")});
 p.claims=vec![Claim {kind:ClaimKind::Capability,text:"bucket".into(),aliases:vec![],evidence:vec![]},Claim {kind:ClaimKind::Capability,text:"emits light".into(),aliases:vec![],evidence:vec!["REPORT".into()]}];
 p.evidence.push(Evidence {id:"REPORT".into(),subject_hash:p.subject_hash().unwrap(),requirements:vec!["WATER".into()],step:"TEST".into(),records:vec!["report.txt".into()],outcome:Outcome::Pass,rigor:Rigor::Physical,reporter:p.publisher.clone(),origin:EvidenceOrigin::Publisher,reproduction_notes:String::new()});
 cache.put(&Artifact::seal(p,&SigningKey::from_bytes(&[7;32])).unwrap()).unwrap();
 let mut index=Index::open(&cache,provider(Arc::new(AtomicUsize::new(0)),"a",false)).unwrap();index.backfill(&cache).unwrap();
 assert_eq!(index.search(&cache,&query()).unwrap().matches.len(),1);
 assert!(index.search(&cache,&Query {supported_claims_only:true,..query()}).unwrap().matches.is_empty());
 assert!(index.search(&cache,&Query {rigor:Some(Rigor::Physical),..query()}).unwrap().matches.is_empty());
}

#[test]
fn every_signed_field_reaches_the_model_without_utf8_truncation() {
 use std::sync::Mutex;
 struct Capture {recipe:Recipe,inputs:Arc<Mutex<Vec<String>>>}
 impl Embedder for Capture {
  fn recipe(&self)->&Recipe {&self.recipe}
  fn embed(&self,input:&[String])->Result<Vec<Vec<f32>>> {self.inputs.lock().unwrap().extend_from_slice(input);Ok(input.iter().map(|_|vec![1.0,0.0]).collect())}
 }
 let root=tempfile::tempdir().unwrap();let cache=Cache::open(root.path()).unwrap();
 let object=artifact(&"桶".repeat(2000));cache.put(&object).unwrap();
 let inputs=Arc::new(Mutex::new(Vec::new()));
 let mut index=Index::open(&cache,Box::new(Capture {recipe:Recipe::new("mock://local".into(),"capture".into(),"a".into()),inputs:inputs.clone()})).unwrap();index.backfill(&cache).unwrap();
 let captured=inputs.lock().unwrap();assert!(captured.len()>1);assert!(captured.iter().all(|chunk|chunk.len()<=2048));
 assert_eq!(captured.concat().as_bytes(),canonical::encode(&object).unwrap());
}

#[test]
#[ignore="requires a real Ollama embedding model"]
fn real_model_discovers_a_container_for_carrying_water_without_lexical_overlap() {
 let endpoint=std::env::var("KNOWLEDGE_EMBEDDING_ENDPOINT").unwrap();
 let model=std::env::var("KNOWLEDGE_EMBEDDING_MODEL").unwrap();
 let root=tempfile::tempdir().unwrap();let cache=Cache::open(root.path()).unwrap();
 let mut bucket=artifact("Bucket with a handle").payload;
 bucket.specification.requirements[0].text="A portable open vessel for carrying liquids".into();bucket.design[0].data=hex::encode("A cylindrical vessel with a curved handle");bucket.subject.conditions="Manual transport".into();
 let bucket=Artifact::seal(bucket,&SigningKey::from_bytes(&[7;32])).unwrap();let address=cache.put(&bucket).unwrap();
 let mut lamp=artifact("Electric lamp").payload;lamp.specification.requirements[0].text="Illuminate a room with electric light".into();lamp.design[0].data=hex::encode("Light bulb with an electric circuit");lamp.subject.conditions="Indoor illumination".into();cache.put(&Artifact::seal(lamp,&SigningKey::from_bytes(&[7;32])).unwrap()).unwrap();
 let mut index=Index::open(&cache,Box::new(embedding::Ollama::connect(&endpoint,&model).unwrap())).unwrap();index.backfill(&cache).unwrap();assert_eq!(index.status(2).indexed,2,"{:?}",index.status(2).failures);
 let q=Query {text:"moving water".into(),threshold:Some(-1.0),..query()};
 assert!(knowledge::query::search(&cache,&q).unwrap().matches.is_empty());
 let result=index.search(&cache,&q).unwrap();assert_eq!(result.matches.len(),2);assert_eq!(result.matches[0].address,address);
 let high=result.matches[0].similarity.unwrap();let low=result.matches[1].similarity.unwrap();assert!(high>low);
 let selected=index.search(&cache,&Query {threshold:Some((high+low)/2.0),..q}).unwrap();assert_eq!(selected.matches.len(),1);assert_eq!(selected.matches[0].address,address);
 println!("REAL_EMBEDDING_MODEL:{}",model);println!("BUCKET_SIMILARITY:{high}; LAMP_SIMILARITY:{low}");
}
