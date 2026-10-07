# Requirements consolidation

Existing REQ-001–078 are preserved, with short titles and normative descriptions,
under REQ-100 and seven area parents. REQ-079–094 are proposed risk-control
requirements. Existing agreed product decisions are approved specification intent;
new controls and mitigations are proposed, not implemented. Parent rollups do not
supply evidence that their children are satisfied.

## Resolved contradictions

All numbered references below use the `REQ-` prefix.

| Earlier conflict or ambiguity | Consolidated interpretation | IDs |
| --- | --- | --- |
| All dependencies must be fully verified versus testing unverified components | Evidence must cover assembly-specification obligations. Applicable component evidence or assembly testing may do so; prior component certification is not universal. Default reuse still needs full verification or explicit opt-in. | 012, 013, 055, 057 |
| Everything hashed versus mutable permissions, payments, and private keys | Every bit of artifact data is committed. Policy, grants, payment state, private keys, and access audit are operational state; they do not change identity. | 019, 052, 068, 075, 094 |
| One object type versus first-class payments, reports, proofs, and unit identifiers | Only specification-to-implementation artifacts are database objects. Payments are first-class API workflows; other values are constituents or operational state. | 020, 021, 059, 066, 068, 078 |
| An embedded report/proof names the final hash that contains it | Reports bind a pre-existing verification subject; publisher signatures bind the payload. The final enclosing hash comes last and responses name it externally. | 014, 038, 050, 051, 077, 078, 093 |
| Hidden content versus complete content validation | Commit to public and encrypted sections. Partial responses expose binding data and checking limits, rather than claim unavailable whole content was validated. Hidden plaintext commitments need confidentiality-aware design. | 030, 038, 075, 080, 081, 093 |
| Fully verified means all inferred capabilities are verified | Completeness is specification-relative. Additional semantic claims need evidence. Reported completeness, signature validity, and requester checking are separate. | 011, 026, 028, 033, 034, 079 |
| Valid signatures or reports establish physical truth/independence | Cryptography authenticates commitments and signing identities. Physical truth, fidelity, and independence remain attributed claims; disagreements are retained. | 040, 042, 049, 058, 079, 088, 092 |
| Reproduction/SOP implies sale of manufacturing know-how | Procedures suffice for the declared verification scope. A physical article can be tested without revealing manufacturing instructions or component data. | 039, 045, 046, 054, 055 |
| Any predecessor is an older version to hide | Conceptual provenance and explicit revision/supersession are distinct. Alternatives and reproduction reports do not automatically hide originals. Filters are configurable and scope-aware. | 016, 047, 087 |
| Non-authoritative lineage notes are outside identity | Notes are hashed and inspectable, but not sufficient proof of contribution. Only relevant agent derivations are included. | 017, 018, 019, 021 |
| Purchases propagate to descendants, components, or distributors | One grant covers one final hash and scope. Upgrades issue new explicit grants. Ciphertext distribution does not confer key-release authority. | 069, 070, 073, 074, 076 |
| Signatures/encryption can change without changing identity | Both are artifact data. Re-signing and re-encryption change the address. Operational key delivery need not rewrite the artifact. | 014, 015, 051, 075, 093 |
| Privacy-preserving verification is already guaranteed | Signed reports allow signature, trace-structure, and disclosed-evidence checks without design disclosure. Checks needing unavailable inputs remain unchecked. Stronger ZKPs are future work. | 038, 055, 077, 078, 079 |
| Browser repository and distributed scope changed | Browser stays here after API/CLI. One-process storage is initial; multi-cache search/substitution are initial interfaces; scalable storage/execution is later. | 006, 035, 060–063 |

Artifact hashes preserve exact model inputs, not deterministic agent output.
Commercial decisions additionally depend on mutable policy, adapter versions,
requester identity, and outcomes; these require operational audit if replayed.

## Remaining design decisions

1. **Canonical representation:** versioned encoding, field types, normalization,
   section layout, ordering, and malformed-input rejection.
2. **Confidential commitments:** bind ciphertext, hidden plaintext, signed reports,
   physical subjects, and future proofs without circularity or low-entropy leaks.
   Select encryption/signature algorithms and key storage with cryptographic review.
3. **Checking policy:** precise report-completeness and requester-check statuses;
   explicit publisher-trust policies; reported versus independently checked
   physical evidence. The CACHE is not a universal judge of physical truth.
4. **Semantic matching:** provider-independent encoding, synonyms, constraints,
   ranking, query expansion, and pump/aqueduct/bucket correctness fixtures.
   Accepting external decompositions alone does not implement fuzzy search.
   Pin query and matcher inputs where replay matters.
5. **Federation and versions:** protocol, controller authentication/discovery,
   pagination, partial failure, explicit revisions, and source-aware filtering.
   A local view cannot claim globally exhaustive lineage or contrary evidence.
6. **Bazel/Nix:** dependency descriptors, materialization, required grants, and
   reproducible execution inputs. Physical purchase does not imply downloadable
   manufacturing dependencies.
7. **Commerce/identity:** callback and clearance states, grant persistence,
   confidential key delivery, key rotation/recovery authority, and host-admin APIs.
   Settlement, refunds, physical fulfillment, and commercial disputes stay with hosts.
8. **Budgets:** limits, retention, timeouts, backup procedures, supported platforms,
   and measurable performance targets before claiming scalability.

The [initial Rust slice](implementation.md) fixes a provisional version-1 public
encoding, signed artifacts, subject-bound trace checking, deterministic label
matching, local storage limits, and exact-hash substitution. Confidential and
commercial formats remain open. Runtime code and tests are entirely Rust.

These are implementation tasks, not reasons to introduce more product object types.
Reputation, physical-truth adjudication, and dispute mechanisms remain uncommitted.

## Validation

The upstream `rules_requirements` parser and validator check the complete directory:
unique IDs, known fields, reference consistency, hierarchy cycles and single-parent
rules, need coverage, mitigation links, and verification-method references.
A coverage audit checks preservation of REQ-001–078 and controls for every risk.
Product acceptance procedures remain plans. Prototype Rust tests are documented
separately; no requirement is marked `verified_by` merely because that slice passes.
