# Semantic embeddings

With an embedding endpoint configured, the Rust cache embeds every stored signed
object and compares query embeddings with its indexed vectors. Nonempty text
searches use semantic matching by default; `"mode":"lexical"` selects the earlier
label/alias matcher. An explicit semantic request fails if embeddings are disabled
or the model is unavailable. Similarity does not establish verification or trust.

## Container deployment

Add the optional model overlay to your existing local or tailnet Compose command:

```sh
docker compose -p knowledge \
  -f compose.yaml -f deploy/compose.tailnet.yaml \
  -f deploy/compose.embeddings.yaml \
  up -d --build --wait --wait-timeout 1200
```

Supply `TS_AUTHKEY` through your existing launch environment. Preserve the same
Compose project and cache volume; if you use a data-volume override, include it
as well. To use a published Knowledge image, set `KNOWLEDGE_IMAGE` and replace
`--build` with `--no-build`. Model weights have their own persistent volume.
The Ollama API has no published host or tailnet port. Model download and first
indexing can take several minutes. The default model is `embeddinggemma` and
`KNOWLEDGE_EMBEDDING_MODEL` selects another installed/pullable model.

For a separately managed Ollama service, set `KNOWLEDGE_EMBEDDING_ENDPOINT` to its
HTTP(S) origin and `KNOWLEDGE_EMBEDDING_MODEL` to the model name in the cache's
environment. Both must be configured. The endpoint must be reachable from the
cache container. Configured endpoints are trusted destinations: the entire public
signed object and user query are sent there. Credentials in URLs and redirects
are rejected. The first adapter uses Ollama's [embedding API](https://docs.ollama.com/api/embed)
and [model manifest API](https://docs.ollama.com/api/tags); the Rust `Embedder` trait
allows future adapters without changing immutable artifacts.

## Query

```sh
curl -s http://knowledge:8787/v1/search \
  -H 'Content-Type: application/json' \
  -d '{"text":"combine RF signals","mode":"semantic","threshold":0.3,"include_incomplete":true}'
```

`threshold` is an inclusive cosine threshold between -1 and 1 (default 0.2).
There is no universal probability or acceptance meaning to a score. Calibrate the
threshold to the model and your catalog; a value of -1 lists all semantically
scored eligible candidates for inspection. Results contain `similarity`, ordered
highest first, with address ordering for ties. Existing completeness, publisher
trust, rigor, subject-scope, supported-claim and eligible-version filters remain.
Failed/incomplete designs are excluded unless `include_incomplete` is true.

`supported_claims_only` and `rigor` compare the query with qualifying individual
claim embeddings, rather than accepting an object because a different capability
has a passing report. Those claim embeddings are computed and cached on demand.
Their scope and evidence remain publisher assertions; embeddings do not turn
inference or incomplete external content into verified/reusable designs.

## Ingestion and checking state

The server backfills all existing objects at startup and indexes each accepted
HTTP publication. Publication returns `indexing: indexed`, `failed` or `disabled`;
an embedding failure does not undo a valid immutable artifact insertion. CLI
imports remain valid without a model and are backfilled when the configured
server next starts. Stop the server before running the offline indexing command,
because the cache has one process owner:

```sh
knowledge index-embeddings
knowledge index-embeddings --rebuild
knowledge search 'moving water' --mode semantic --threshold 0.3 --include-incomplete
```

The commands use the same embedding environment variables. Rebuild regenerates
only the current recipe's derived index; artifacts and other recipe histories are
preserved. `GET /v1/embeddings/status` reports model/recipe, total and indexed counts,
and per-artifact failures. Semantic responses expose skipped eligible objects,
not an exhaustive success claim. A startup configuration/model error prevents
startup; a later query-model failure returns HTTP 503. Lexical search remains an
explicit alternative.

## Identity and replay

The input is the complete canonical signed artifact JSON, including requirements,
design and report blobs, claims, lineage, source descriptors and signature. External
source bytes are not silently fetched: the signed descriptors are the structured
data available to the cache. This indexes declared content, not an understanding
of unavailable realized designs. Binary blob bytes remain their canonical hex
representation; model usefulness for binary-heavy data is an empirical limitation.

The canonical UTF-8 input is split at character boundaries into at most 1024-byte
chunks. Every chunk is sent with truncation disabled, in batches of up to 16.
Each returned vector is normalized, their mean is normalized, and the final f32
vector is retained. Empty, zero, non-finite, oversized or inconsistent vectors
are rejected. A provider input-limit error is visible rather than silently
discarding content. Vector dimensions are bounded at 4096, requests/responses at
4 MiB, and derived index storage at 128 MiB across recipe/model revisions.

Each derived record hashes the input commitment, input kind, recipe, model manifest
digest, chunk count and exact vector bits. The configured model digest is checked
before and after inference. Existing records are integrity/binding checked on
restart; changing the model/recipe selects a separate index. A model digest is a
provider assertion, not remote attestation of faithful model execution.

Search returns a hashed provenance record with the query vector, returned object
vectors, qualifying claim vectors, all considered vector-record commitments,
score bits, threshold bits, filters and the local artifact-address snapshot.
It permits score/filter replay using those exact committed inputs. Other considered
vectors are retained in the operator's derived index and referenced by hash.
This does not promise identical future model inference or independent evidence
authenticity. Floating-point values in the API are separate from integer-only
artifact canonicalization; hashed vectors and thresholds use exact integer bits.

These are derived index/response records, not separately published database objects
or new publisher artifacts. Updating signed semantic labels still requires a new
artifact and predecessor relationship. Requirement links and test limits are in
[traceability.md](traceability.md).
