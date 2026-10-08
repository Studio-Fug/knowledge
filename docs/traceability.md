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

| Requirement | Current implementation | Required evidence |
| --- | --- | --- |
| REQ-111 | **Gap:** version 1 supports inline design/evidence blobs only. | Git and GitHub realization fixtures integrated with a signed artifact. |
| REQ-112 | **Gap:** external descriptors and format versions need a model extension. Existing canonicalization/signature tests cover inline fields only. | Pinned-revision validation; changed descriptor/commitment changes identity; mutable-ref rejection. |
| REQ-113 | **Gap:** no external source fetch/realize/check path yet. Existing substitution verifies complete inline artifact hashes. | Matching and tampered realizations, deterministic tree encoding, bounded fetching and rejection before use. |
| REQ-114 | **Gap:** local disk cache retains complete inline representations. | Restart with manifest-only storage; fetch and optional eviction preserve identity. |
| REQ-115 | **Gap:** no external-content checking state yet. | Missing, inaccessible and mismatched sources remain explicit and cannot be treated as checked. |

The proposed identity model keeps the signed artifact manifest's content address,
with external constituents committed by SHA-256 references checked against their
realizations. Computing the final address over a fully expanded artifact is an
alternative still under discussion. The requirements deliberately specify the
integrity properties without silently choosing between those representations.
Nix's source-tree/NAR hash is not assumed to equal our canonical artifact hash.
