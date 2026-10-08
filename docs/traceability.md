# Implementation traceability

Requirements remain normative YAML. This record distinguishes implemented behavior,
the checks that exercise it, and remaining gaps; it is not a blanket product
verification report. `modules` links requirements to their implementation files.
Cargo tests are named here without inventing Bazel verification targets.

## Container slice

| Requirement | Implementation | Evidence and limits |
| --- | --- | --- |
| REQ-108 | [Dockerfile](../Dockerfile), [CLI](../src/main.rs), [server](../src/server.rs) | `tests/container.rs::image_serves_as_nonroot_and_preserves_signed_artifacts_across_recreation`: native image runs, read-only root, non-root identity, health, allowed/disallowed publishers. [Rust CI](../.github/workflows/rust.yml) runs on amd64 and arm64. |
| REQ-109 | [launcher](../src/bin/knowledge-launch.rs), [sidecar](../deploy/compose.tailnet.yaml), [forwarding](../deploy/tailscale/serve.json) | Launcher unit tests check bounded key input, secret-free arguments/errors and runtime key environment. `tests/container.rs::compose_keeps_tailnet_routing_outside_the_cache_image` checks actual Compose configuration and sidecar executables. Live enrollment, routing and tailnet ACL behavior remain operator acceptance checks requiring a key and a second client. |
| REQ-110 | [volumes](../compose.yaml), [sidecar state](../deploy/compose.tailnet.yaml), [storage](../src/store.rs) | Container test inserts a signed artifact, stops/removes the process, recreates it on the volume and retrieves the same hash. Launcher unit test excludes volume deletion on shutdown; core storage test checks exclusive locking. Live tailnet identity persistence is not tested by key-free CI. |

Run the container checks after building the image:

```sh
docker build -t knowledge:test .
cargo test --locked --test container -- --ignored --test-threads=1
```

## Git-backed content

| Requirement | Implementation | Evidence and limits |
| --- | --- | --- |
| REQ-111 | [version-2 descriptors](../src/model.rs), [source realization](../src/backing.rs), [CLI](../src/main.rs) | `tests/backing.rs::descriptors_are_pinned_hashed_and_fetches_need_explicit_host_permission` integrates external design with a signed artifact. Public HTTPS Git archives, including GitHub; direct Git transport and private credentials remain gaps. |
| REQ-112 | [descriptor validation and subject hashing](../src/model.rs), [source validation](../src/backing.rs) | Descriptor tests reject mutable revisions and embedded credentials; changed selection changes artifact and subject hashes. Record-source tests check that evidence commitments stay inside the artifact without creating a circular subject. |
| REQ-113 | [bounded fetching, deterministic realization and hash checking](../src/backing.rs), [realize command](../src/main.rs) | Realization tests cover changed content/modes, reordered archives, transport metadata, unsafe links, duplicate/colliding paths and no partial export. Network smoke test fetches a pinned GitHub source with explicit origin permission. Checked content is exported only after all commitments match; tests are not rerun. |
| REQ-114 | [manifest storage](../src/store.rs), [separate export](../src/backing.rs) | `tests/backing.rs::cache_persists_only_the_signed_manifest_and_exports_checked_content_separately` stores a small manifest backed by a larger fixture, restarts, realizes separately and removes the realization without changing identity. No automatic realization retention/eviction policy is implemented. |
| REQ-115 | [checking summaries](../src/model.rs), [fetch failures](../src/backing.rs) | Even trusted, publisher-reported complete external artifacts remain `reusable: false` until the caller realizes their content. Forbidden origins and mismatches fail explicitly without mutating stored manifests. No global availability guarantee or independently-verified status is claimed. |

[Backing-source design](backing.md) specifies the representation and normalization.
Unit fixtures are synthetic evidence of software behavior. CI results establish
which tests actually passed; this table alone is not a certification.
