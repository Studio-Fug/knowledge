# knowledge

A simple database mapping requirement specifications to concrete designs and
their verification reports, with content-addressed references, distributed
caches, and semantic discovery.

The project starts with requirements. The initial draft is in
[`requirements/model.yaml`](requirements/model.yaml), following the YAML model
used by [rules_requirements](https://github.com/Studio-Fug/rules_requirements).
REQ-001 through REQ-009 correspond to the nine starting statements. The three
user needs are proposed groupings for their traceability.

No implementation or verification evidence exists yet. These requirements are
a draft for discussion, not a claim that the system satisfies them.

## Working definitions

- **REQUIREMENTS-TO-IMPLEMENTATION ARTIFACT**: the sole first-class database
  object, pairing a requirement specification with an implementation. Its
  specification, design, report, evidence, semantic decomposition, and provenance
  are constituent data. Content addresses identify whole artifacts; dependency
  and predecessor links refer to whole artifacts.
- **REQUIREMENT SPECIFICATION**: the description of what a design must satisfy.
- **IMPLEMENTATION**: a concrete design together with its verification report.
- **VERIFICATION REPORT**: an artifact tracing a concrete design to its
  requirement specification.
- **CONTENT ADDRESS**: a SHA-256 identifier for a database entry.
- **CACHE**: an instance of the system, addressable by internet address and port.
- **SEMANTIC DECOMPOSITION**: a representation used to discover requirement
  specifications and implementations by capabilities and constraints extracted
  from the specification and concrete design. Its encoding remains to be defined.

## Agreed verification behavior

The database may hold incomplete verification evidence to make remaining work
visible. A machine-readable status must distinguish those implementations from
fully verified implementations. Fully verified means passing evidence covers
every requirement for the concrete design and meets any explicitly specified
evidence or rigor demands; missing, failed, or stale evidence
prevents that classification.

Selection, retrieval for reuse, and dependency resolution require fully verified
implementations by default. An explicit opt-in permits other implementations,
whose verification status must remain visible, including across caches. These
behaviors are captured in REQ-003 and REQ-010 through REQ-013.

Full verification requires coverage of dependency-related obligations in the
assembly specification, using applicable component evidence or verification
performed for the assembly. Previously unverified components do not prevent
full verification when assembly evidence covers every requirement and explicit
rigor demand. Missing required evidence still prevents full verification.
This replaces the earlier blanket requirement for fully verified dependencies.

## Agreed content addressing and history

A requirements-to-implementation artifact's content address is the SHA-256 of a deterministic manifest
covering all of its data, including its specification, design files, verification
report and evidence, semantic decomposition, dependencies, predecessors, and
display-only explanations. Referenced artifacts are identified by content address. The precise
manifest format and byte encoding remain to be defined.

Objects are immutable. New evidence, features, requirements, or changes in
composition produce a distinct object. A derived version records one or more
predecessor objects that contributed conceptually to its production. All of
those hashes are included in its own hashed content, making the provenance
traversable and binding the version to its ancestry. An initial object records
an empty predecessor set. History can branch and merge, preserving the origins
of ideas as well as the sequence of versions.
These behaviors are captured in REQ-014 through REQ-016.

Predecessors record conceptual provenance; they are not implementation dependencies. A fully
verified version may therefore follow an incompletely verified version.


Each predecessor link includes a human-readable explanation of its conceptual
contribution. This explanation is display-only and non-authoritative: it does
not establish verification status, dependencies, or lineage conclusions.
Agents deriving a provenance account must examine the linked specifications,
implementations, and semantic decompositions and draw their own conclusions.
These behaviors are captured in REQ-017 and REQ-018.

Every bit of an object's stored data is covered by its content address, directly
or through content-addressed references. This includes display-only and
non-authoritative metadata: editing a predecessor explanation creates a new
object and hash. No object metadata sits outside this coverage (REQ-019).
This preserves the exact data available to an agent for reproducing an
inference chain. Hash coverage preserves inputs; it does not make explanations
authoritative or guarantee that an agent will reproduce the same conclusions.

## Agreed database scope

Requirements-to-implementation artifacts are the only first-class objects
(REQ-020). Reports, evidence, semantic decompositions, and provenance metadata
belong to these artifacts. An agent's inference is included only as constituent
data of a new requirements-to-implementation artifact, and only when it
contributes to that artifact's traceability. Inference that does not contribute
to traceability is omitted. Producing a new artifact does not by itself justify
including the inference; standalone reasoning or provenance accounts are not
database objects (REQ-021).

The same requirement specification may appear in multiple artifacts with
different implementations. Each alternative retains its own verification
report, content address, and provenance history (REQ-022).

The same concrete design may also satisfy different specifications, represented
as separate artifacts. Each artifact must provide verification tracing the
design to its particular specification (REQ-023).

Evidence cached in an existing artifact may be reused directly when it applies
to the concrete design and covers the new specification's full traceability
requirements. The new artifact contains the reused evidence and its trace to
the new specification. Its verification status is determined against that
specification rather than inherited from the source artifact (REQ-024).

## Agreed verification rigor and discovery

Specifications may demand particular evidence or rigor levels, but are not
required to do so. Explicit demands must be met (REQ-025).

Reports record verification methods and rigor actually supported by evidence,
with traceability to the requirements and design verified. Omission of a rigor
demand does not establish achieved rigor (REQ-026).

Achieved rigor is queryable alongside semantic requirements. For example,
"Show me smart faucet designs that have actually been built and tested" must
distinguish physical construction and testing evidence from simulation alone.
Results expose the scope covered by the qualifying evidence (REQ-027).

When the specification demands no particular rigor, passing simulation evidence
may establish full verification if it covers every requirement for the concrete
design and meets the other full-verification conditions, including coverage of
dependency-related obligations. Fully verified does not imply physically built or tested; those
remain separate evidence-based query dimensions (REQ-028).

## Agreed remote cache retrieval

Each cache uses an explicitly configured ordered list of remote cache internet
addresses and ports. When an artifact is absent locally, retrieval tries those
caches in order until a valid matching artifact is obtained or the list is
exhausted (REQ-029).

Retrieved content must be validated against the requested SHA-256 content
address before it is accepted for local storage or use. Mismatched content is
rejected and does not satisfy the request (REQ-030).

## Agreed semantic decomposition and revision

Semantic decompositions describe capabilities and constraints extractable from
both the specification and implementation. For example, "moves water" may group
pumps, buckets, and aqueducts, while "requires electrical power" or "supports
continuous flow" distinguishes designs. Labels may be inferred by examining
the concrete design, beyond the terminology in its specification (REQ-031).

As model capability or understanding improves, semantic labels may be revised
by creating a new artifact containing the updated decomposition and a
predecessor link to the previous artifact's hash, then hashing the new artifact.
The previous artifact remains unchanged; the revision introduces no additional
first-class object type (REQ-032).

Each semantic capability and constraint exposes a machine-readable verification
status. Verified capabilities and constraints trace to passing, applicable report evidence covering
the claim and its stated conditions, with the method and achieved rigor exposed.
Inference without sufficient evidence is distinguished from verification
(REQ-033).

Browsing and search summaries expose these statuses and evidence references,
and searches can filter by verified capabilities and constraints. An agent can identify what is
actually verified without reading the entire artifact. Overall full verification
against the specification does not automatically verify additional capabilities or constraints
inferred from the design (REQ-034).

## Agreed search scope and commercial access

Semantic search supports both local and configured remote caches, with a
selectable scope. Results identify their source cache and artifact address
(REQ-035). No default search scope has been selected.

Publishers may expose queryable specifications and semantic discovery data,
present verification evidence or proofs on request, and sell access to concrete
implementations. These disclosures have separate access controls. Knowing an
artifact address does not authorize retrieval of restricted content, and remote
substitution must respect those restrictions (REQ-036 and REQ-037).

Evidence or proofs identify the exact artifact, specification, claims, and
rigor they concern. Checking a claim must not require unauthorized disclosure
of implementation content. Publisher claims and verification independently
checked by a requester must remain distinguishable (REQ-038).

The economic aim is to make costly verification reusable through inexpensive
checking. Zero-knowledge proofs derived from reports and artifacts are the
preferred direction to investigate; the proof statement, mechanism, and cost
targets remain open. Specifications, evidence, and implementations remain
constituents of the same artifact, with controlled partial disclosure rather
than additional first-class object types. Public portions and proofs must be
bound to the artifact's content address; no object data is exempt from hashing.

## Independent re-verification

An authorized recipient may rerun validation checks and contribute a report
and evidence in a new artifact linked to the original as a predecessor. The
report identifies the exact specification and design checked, checks and
conditions, results, and verifying party. The original remains unchanged
(REQ-039).

Reports distinguish publisher evidence from recipient or third-party
re-verification, with provenance and scope visible. A hash, cryptographic proof,
or claim of independence alone does not establish that physical measurements
are truthful or that parties are independent (REQ-040). Proof of authentic
evidence remains a desired direction, with source authentication and trust
assumptions unresolved. No reputation, payment arbitration, or consensus
mechanism has yet been selected.

A submission and retrieval hook supports recipient and third-party
re-verification artifacts, including failed results. Conflicting reports linked
to the original artifact are discoverable subject to access restrictions;
reports remain parts of artifacts (REQ-041).

Reports record reproduction procedures, environment and conditions, and known
deviations from the original design or validation procedure. Unknown details
remain explicit. Reproduction fidelity is not assumed, and a conflicting result
alone does not establish which report is correct (REQ-042).

The initial scope is this evidence mechanism. Reputation scoring and policies
for resolving disputes are deferred until practical experience informs them;
they are not prerequisites for submitting or retrieving re-verification.

## Initial build integration

Bazel and Nix are the initial supported integrations for expanding
content-addressed implementation dependencies into build-system relationships.
Other build systems may be added later (REQ-043).

Dependency relationships identify artifacts by content address independently
of build-system-specific integration data. Additional integrations can use the
same dependency model; any integration data stored in an artifact participates
in its hash (REQ-044).

## Reproducible verification procedures

Each artifact includes a well-written standard operating procedure for
reproducing its verification. As applicable, it covers build instructions,
tools and dependencies, equipment and setup, inputs and test conditions,
ordered steps or commands, acceptance criteria, evidence capture, and traces
to the specification. Physical procedures are included where commands alone
are insufficient. Missing necessary instructions are explicit remaining
verification work and prevent full verification (REQ-045).

The procedure can assume ordinary domain knowledge and reference established
procedures instead of restating general background. Necessary references
identify the applicable revision and are available to the authorized recipient;
artifact instructions and reference identifiers are included in the hash
(REQ-046).

## Version filtering

Agents can configure whether browsing and search include superseded artifact
versions. History remains accessible, and multiple current branches are
supported without assuming a single universally latest artifact (REQ-047).
No default for this filter has been selected.

## Publisher authentication

Published artifacts carry a cryptographic signature binding the publisher's
signing identity to the exact artifact content. The signing public key or its
identifier and signature validation result are available to recipients,
including across caches (REQ-048).

A valid signature authenticates the signing identity; it does not establish
the truth of verification claims, reproduction fidelity, independence, or a
trusted real-world identity (REQ-049).

Signed artifacts use two stages (REQ-050 and REQ-051):

1. Hash the deterministic payload and sign that payload hash.
2. Hash the complete artifact containing the payload, signature, and signing
   metadata. This final SHA-256 is the database content address.

The payload hash is an internal signing value, not another first-class object
or retrieval address. Dependency and predecessor links use the final artifact
address. Validation checks both the signature over the recomputed payload hash
and the final content address. Changes to payload, signature, or signing
metadata change the final address. Every stored bit remains covered by hashing,
without making the signature depend on a hash containing itself.

## Mutable access policy

Access grants and revocations are cache configuration associated with requester
identities and artifact addresses. They are not artifact data or additional
first-class database objects. Changing permissions leaves artifact content,
publisher identity, and content address unchanged (REQ-052). This separates
mutable access policy from the full hash coverage of artifact data.

The commercial mechanism charges for physical articles or access to artifact
content and manufacturing know-how, not ownership of the ideas. Each cache enforces applicable grants and revocations on subsequent
access requests, including evidence and implementation requests (REQ-053).
Revocation governs future access; it does not erase content already delivered.


## Physical articles and assembly value

Buying a physical article does not grant its design or manufacturing know-how.
The verification report may be disclosed separately. Know-how may be sold in a
separate transaction with its own price and permissions (REQ-054).

An integrator may offer a validated assembly with a report tracing to the
assembly specification without disclosing component designs, know-how, reports,
or other component data. Assembly purchase grants no automatic component access
(REQ-055). Assembly-validation work is a distinct contribution to value from
component-validation work, and may be compensated independently (REQ-056).
Know-how may command a higher price; no fixed pricing rule has been selected.

SOPs and reproducibility information remain included in artifacts but subject to
access restrictions. Inclusion does not require disclosure to physical-article
buyers. Dependency verification and component-data disclosure are separate.

Integrators may test previously unverified components, discard failures, and
use selected passing components in a verified assembly. Evidence and SOPs
identify the tested scope and selection or screening conditions. Verification
does not extend automatically to untested components or units (REQ-057).

## Remaining decisions to make together

1. What deterministic manifest format and byte encoding shall be hashed?
2. What integration data is required to resolve dependencies in Bazel and Nix?
3. How shall semantic capabilities and constraints be encoded and matched?

The guiding design preference is to keep the system as simple as possible.
Storage formats, protocols, search machinery, and build integration remain open
until the requirements establish what is needed.
