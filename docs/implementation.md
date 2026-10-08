# Initial Rust implementation

The first slice is a single Rust library and executable. It stores **public**
specification-to-implementation artifacts on disk, signs them, checks structural
traceability, searches supplied semantic labels, and fetches exact hashes from
other caches. It does not execute designs or test procedures.

## Run

Install Rust through rustup; the repository pins toolchain 1.99.0. Then:

```sh
cargo build --locked
cargo test --locked --all-targets
cargo run --locked -- --help
cargo run --locked -- keygen --output publisher.secret
cargo run --locked -- subject --input examples/payload.json
cargo run --locked -- seal --input examples/payload.json --key publisher.secret > artifact.json
cargo run --locked -- put --input artifact.json
cargo run --locked -- search 'moving water' --include-incomplete
cargo run --locked -- serve
```

The example is deliberately unverified. For real reports, use your own payload
and evidence; `search 'moving water' --physical` filters physical capability reports.
The default cache is `.knowledge`; `--cache PATH` selects another. CLI success
and error output is JSON, on stdout and stderr respectively. Key generation
creates a new file, refuses to overwrite it, and uses mode 0600 on Unix. Keep
private keys outside artifacts and source control.

`Payload` in [model.rs](../src/model.rs) defines the draft input shape. Every
field is explicit: version, publisher, specification, design, procedure, subject,
evidence, records, claims, predecessors, dependencies. Optional rigor is `null`;
empty collections are `[]`. Blob contents are lowercase hex bytes. `seal` sets
the publisher key but never rewrites evidence. First compute the subject hash,
then put it into each applicable evidence record, then sign the payload.

[Git-backed content](backing.md) extends the payload with optional `sources` in
version 2. Existing version-1 artifact addresses and signatures remain valid.

## Agent API

The server defaults to `127.0.0.1:8787`; a non-loopback listener requires explicit
`--allow-network`. The [container setup](containers.md) configures this for ordinary
containers, or keeps the server on loopback behind a private-network sidecar. It is
read-only unless `--allow-publisher PUBLIC_KEY` is configured; publication then
requires a valid signature from an allowed key. There is no requester identity,
confidential-content authorization, CORS, or payment protocol yet. Everything
inserted is public to anyone who can reach this server.

| Request | Response |
| --- | --- |
| `GET /health` | Mode and status |
| `GET /v1/artifacts/<sha256>` | Complete signed public artifact |
| `POST /v1/artifacts` with artifact JSON | Validated artifact address |
| `POST /v1/search` with query JSON | Summaries and checking limits |

An empty query `{}` lists up to 20 publisher-reported complete artifacts. Query
fields are `text`, `trust_publisher`, `include_incomplete`, `rigor`, `scope`,
`supported_claims_only`, `exclude_superseded`, and `limit` (maximum 100).
Rigor categories are `analysis`, `simulation`, `software`, and `physical`;
they are exact categories, not a universal ordering. Scope is `design`, `batch`,
or `unit`. `--physical` is a CLI shorthand for physical rigor.

Exact-hash substitution uses configured origins in order:

```sh
knowledge get <sha256> --substituter http://127.0.0.1:8787
```

Only HTTP(S) endpoints configured by the caller are contacted. Redirects are
disabled; requests have a ten-second timeout and bounded responses. Every
substituted artifact is signature-checked and hash-checked before insertion.
Dependencies and predecessors are references, never instructions to contact a
host or execute a build.

## Identity and verification

Versions 1 and 2 use canonical JSON that sorts object keys, preserves array order and Unicode
code points, emits compact UTF-8, and permits only integer numbers. Duplicate
keys and unknown fields are rejected. There is no implicit Unicode normalization.
The SHA-256 address covers the entire signed artifact, including descriptions,
labels, evidence, ancestry, and signature. Re-signing changes identity.

To avoid circularity, evidence binds a separate subject digest covering the
specification, concrete design, procedure, physical scope, and dependency hashes,
with an explicit versioned domain. Evidence, record bytes, semantic labels, and
ancestry remain inside the enclosing artifact hash. Changing only labels/history
can reuse evidence; changing the verification subject makes old evidence stale.
Publisher signatures use Ed25519 over a domain-prefixed canonical payload digest.

`reported_complete` means each specification requirement has an applicable
passing report, referenced records, and a procedure; any requested rigor must
match. Applicable failing reports produce `failed`; missing or stale reports
produce `incomplete`. Default search excludes both. Capability summaries separately
show `inferred`, `publisher_reported_support`, or `failed`.

Signatures authenticate the publisher, **not physical truth or adequacy of the
tests**. Reports and reporter independence are publisher assertions. `reusable`
is false unless the requester explicitly supplies the exact trusted publisher
key, the artifact is reported complete, and its content is inline. External content
always remains unchecked in a search/inspect summary; [realize](backing.md) checks
its commitments before exporting it. Even then, the output explicitly says
tests were not rerun. This prototype has no independently-checked status.

Search is a deterministic scan using labels, aliases, token stemming, and one
character fuzzy matching. Supply common capability labels such as “moves water”
for pumps, buckets, and aqueducts. It does not yet infer decompositions from
arbitrary designs or use embeddings. Query filtering is per supported claim;
unrelated physical evidence does not satisfy a physical capability query.

An explicit `revision` predecessor may hide an older artifact only when the new
artifact passes the same query filters and uses the same publisher key.
Conceptual predecessors and another publisher's assertions do not hide it.
Results describe the local view; they cannot establish global latest versions or
the absence of contrary evidence elsewhere.

## Storage and limits

One process owns the cache through an exclusive file lock. Writes use atomic
non-overwriting temporary-file installation and filesystem synchronization.
Reads revalidate signatures and content addresses; corruption fails closed.
Artifact paths are derived only from validated hashes. No design is executed. Explicit realization exports checked files to a new
directory; ordinary cache reads do not fetch or extract external sources.

Limits: 4 MiB per artifact/request, 256 entries per artifact section, 1000 cached
artifacts, and 256 MiB cached content. Search scans the cache and has no persistent
index. The sequential server is a development interface, not a hardened
internet service. Linux and macOS are the initial target platforms.

## Remaining work

This slice contributes to artifact identity, traceability, lineage, discovery,
local cache, substitution, and API requirements; it does not certify them all.
The YAML requirements remain the normative product scope, with no blanket
verification claim made by these tests.

Next slices: schema evolution and hierarchical artifact specifications; richer
semantic matching and remote search; Bazel/Nix dependency materialization;
confidential sections and controller authorization; payment adapters and
hash-specific grants; independently authenticated reproduction reports; then
the Rust human browser in this repository. Encryption, ZKPs, payments, and paid
access are unsupported in this public prototype. Do not put private designs in it.

CI compiles the actual Rust code and runs formatting, tests, and Clippy. The
integration tests cover stale evidence, tampering, immutable storage, revisions,
query filtering, loopback restrictions, and public-cache substitution. Synthetic
fixture reports are test data, not verification of a physical product.
