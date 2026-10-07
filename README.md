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
  specifications by meaning. Its structure is still to be defined.

## Agreed verification behavior

The database may hold incomplete verification evidence to make remaining work
visible. A machine-readable status must distinguish those implementations from
fully verified implementations. Fully verified means passing evidence covers
every requirement for the concrete design; missing, failed, or stale evidence
prevents that classification.

Selection, retrieval for reuse, and dependency resolution require fully verified
implementations by default. An explicit opt-in permits other implementations,
whose verification status must remain visible, including across caches. These
behaviors are captured in REQ-003 and REQ-010 through REQ-013.

Full verification also requires every implementation dependency, directly or
transitively, to be fully verified. Missing, incomplete, failed, stale, or unknown
verification of a dependency prevents the containing implementation from being
classified as fully verified.

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

## Remaining decisions to make together

1. What evidence and verification rigor does each requirement demand?
2. What deterministic manifest format and byte encoding shall be hashed?
3. Can one implementation satisfy multiple specifications?
4. When must implementation references become build dependencies? Is Bazel the
   first supported build system or just an example?
5. How are remote caches selected, and must retrieved content be checked
   against the requested SHA-256 before use?
6. What is a semantic decomposition, and what search results count as correct
   for the pump/aqueduct/bucket example?

The guiding design preference is to keep the system as simple as possible.
Storage formats, protocols, search machinery, and build integration remain open
until the requirements establish what is needed.
