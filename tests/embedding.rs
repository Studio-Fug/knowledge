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
 let evidence=result.semantic.unwrap();evidence.record.query_embedding.validate().unwrap();evidence.record.object_embeddings[&address].validate().unwrap();
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
 let mut restarted=Index::open(&cache,provider(calls,"a",false)).unwrap();restarted.backfill(&cache).unwrap();let search=restarted.search(&cache,&query()).unwrap();assert!(search.matches.is_empty());assert_eq!(search.semantic.unwrap().record.skipped[&address],"invalid_embedding");assert_eq!(cache.get(&address).unwrap().address().unwrap(),address);
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
 let q=Query {threshold:Some(0.1),..query()};
 let mut index=Index::open(&cache,provider(Arc::new(AtomicUsize::new(0)),"a",false)).unwrap();index.backfill(&cache).unwrap();
 assert_eq!(index.search(&cache,&q).unwrap().matches.len(),1);
 assert!(index.search(&cache,&Query {supported_claims_only:true,..q.clone()}).unwrap().matches.is_empty());
 assert!(index.search(&cache,&Query {rigor:Some(Rigor::Physical),..q}).unwrap().matches.is_empty());
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

#[test]
#[ignore="requires a real Ollama model and pinned public catalog artifact"]
fn real_model_indexes_the_multichunk_core_artifact_and_its_actual_signed_bytes() {
 use std::io::Read;
 let root=tempfile::tempdir().unwrap();let cache=Cache::open(root.path()).unwrap();
 let mut bytes=Vec::new();ureq::get("https://raw.githubusercontent.com/Studio-Fug/28ghz-2way-power-divider/7688adb7f19feb475dbbd40aa60f916cde5e4a09/knowledge/artifact.json").call().unwrap().into_reader().take((canonical::MAX_BYTES+1) as u64).read_to_end(&mut bytes).unwrap();
 assert_eq!(canonical::digest(&bytes),"a462549b5c495b935125a5a705e2d238e7d6f9a29d95990c9e599a47ad0d9824");
 let artifact:Artifact=canonical::parse(&bytes).unwrap();let address=cache.put(&artifact).unwrap();
 assert_eq!(address,"9cdb703565b6e97f87064670c3646c4d566506af4e155964fe5069bd6695b88e");
 let backend=embedding::Ollama::connect(&std::env::var("KNOWLEDGE_EMBEDDING_ENDPOINT").unwrap(),&std::env::var("KNOWLEDGE_EMBEDDING_MODEL").unwrap()).unwrap();
 let mut index=Index::open(&cache,Box::new(backend)).unwrap();assert!(index.ingest(&artifact),"{:?}",index.status(1).failures);
 let result=index.search(&cache,&Query {text:"combine RF signals".into(),threshold:Some(-1.0),..query()}).unwrap();assert_eq!(result.matches[0].address,address);
 let evidence=result.semantic.unwrap();assert!(evidence.record.object_embeddings[&address].record.chunks>1);assert_eq!(evidence.record.object_embeddings[&address].record.input_sha256,address);
 assert_eq!(evidence.sha256,canonical::digest(&canonical::encode(&evidence.record).unwrap()));
 println!("DIVIDER_SIMILARITY:{:?}; CHUNKS:{}",result.matches[0].similarity,evidence.record.object_embeddings[&address].record.chunks);
}

#[test]
fn ollama_adapter_uses_all_chunks_disables_truncation_and_checks_model_identity() {
 use std::thread;
 use tiny_http::{Server,Response};
 let server=Server::http("127.0.0.1:0").unwrap();let endpoint=format!("http://{}",server.server_addr());
 let thread=thread::spawn(move || {
  for position in 0..4 {
   let mut request=server.recv_timeout(std::time::Duration::from_secs(10)).unwrap().unwrap();
   let response=if position==2 {
    assert_eq!(request.url(),"/api/embed");let mut body=String::new();request.as_reader().read_to_string(&mut body).unwrap();let value:serde_json::Value=serde_json::from_str(&body).unwrap();assert_eq!(value["truncate"],false);assert_eq!(value["input"],serde_json::json!(["bucket","lamp"]));
    // Ollama versions may omit model in the response; the explicit request and
    // before/after manifest digest still pin the inference model.
    serde_json::json!({"embeddings":[[1.0,0.0],[0.0,1.0]]})
   } else {assert_eq!(request.url(),"/api/tags");serde_json::json!({"models":[{"name":"test:latest","digest":"a".repeat(64)}]})};
   request.respond(Response::from_string(response.to_string())).unwrap();
  }
 });
 let backend=embedding::Ollama::connect(&endpoint,"test").unwrap();assert_eq!(backend.recipe().revision,"a".repeat(64));assert_eq!(backend.embed(&["bucket".into(),"lamp".into()]).unwrap().len(),2);thread.join().unwrap();
}

#[test]
fn configured_http_ingestion_indexes_new_objects_and_semantic_requests_use_the_index() {
 use std::{thread,io::Read};
 let root=tempfile::tempdir().unwrap();let server=knowledge::server::bind("127.0.0.1:0".parse().unwrap()).unwrap();let endpoint=format!("http://{}",server.server_addr());let object=artifact("bucket");let publisher=object.payload.publisher.clone();let bytes=canonical::encode(&object).unwrap();
 let thread=thread::spawn(move || {let cache=Cache::open(root.path()).unwrap();let mut index=Index::open(&cache,provider(Arc::new(AtomicUsize::new(0)),"a",false)).unwrap();for _ in 0..3 {let request=server.recv_timeout(std::time::Duration::from_secs(10)).unwrap().unwrap();knowledge::server::handle_with_index(request,&cache,std::slice::from_ref(&publisher),Some(&mut index)).unwrap();}});
 let response=ureq::post(&format!("{endpoint}/v1/artifacts")).send_bytes(&bytes).unwrap();let mut body=String::new();response.into_reader().read_to_string(&mut body).unwrap();assert_eq!(serde_json::from_str::<serde_json::Value>(&body).unwrap()["indexing"],"indexed");
 let mut body=String::new();ureq::post(&format!("{endpoint}/v1/search")).send_string(r#"{"text":"moving water","mode":"semantic","threshold":0.9,"include_incomplete":true}"#).unwrap().into_reader().read_to_string(&mut body).unwrap();let result:serde_json::Value=serde_json::from_str(&body).unwrap();assert_eq!(result["matches"].as_array().unwrap().len(),1);assert_eq!(result["matches"][0]["similarity"],1.0);
 let mut body=String::new();ureq::get(&format!("{endpoint}/v1/embeddings/status")).call().unwrap().into_reader().read_to_string(&mut body).unwrap();assert_eq!(serde_json::from_str::<serde_json::Value>(&body).unwrap()["indexed"],1);thread.join().unwrap();
}
