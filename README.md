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

- **REQUIREMENT SPECIFICATION**: the description of what a design must satisfy.
- **IMPLEMENTATION**: a concrete design together with its verification report.
- **VERIFICATION REPORT**: an artifact tracing a concrete design to its
  requirement specification.
- **CONTENT ADDRESS**: a SHA-256 identifier for a database entry.
- **CACHE**: an instance of the system, addressable by internet address and port.
- **SEMANTIC DECOMPOSITION**: a representation used to discover requirement
  specifications by meaning. Its structure is still to be defined.

## Decisions to make together

## Agreed verification behavior

The database may hold incomplete verification evidence to make remaining work
visible. A machine-readable status must distinguish those implementations from
fully verified implementations. Fully verified means passing evidence covers
every requirement for the concrete design; missing, failed, or stale evidence
prevents that classification.

Selection, retrieval for reuse, and dependency resolution require fully verified
implementations by default. An explicit opt-in permits other implementations,
whose verification status must remain visible, including across caches. These
behaviors are captured in REQ-003 and REQ-010 through REQ-012.

## Remaining decisions to make together

1. What evidence and verification rigor does each requirement demand?
2. What exact bytes does a content address hash? How are multi-file designs and
   references represented?
3. Can one specification map to multiple implementations, and can one
   implementation satisfy multiple specifications?
4. When must implementation references become build dependencies? Is Bazel the
   first supported build system or just an example?
5. How are remote caches selected, and must retrieved content be checked
   against the requested SHA-256 before use?
6. What is a semantic decomposition, and what search results count as correct
   for the pump/aqueduct/bucket example?

The guiding design preference is to keep the system as simple as possible.
Storage formats, protocols, search machinery, and build integration remain open
until the requirements establish what is needed.
