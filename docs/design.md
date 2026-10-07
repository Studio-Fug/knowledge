# Design and decisions

A simple system for exchanging requirement specifications, concrete implementations,
and traceable verification. A deployed instance is a **CACHE**.

The sole first-class database object is a **requirements → implementation artifact**.
It contains its specification, design or physical-subject description, verification
report and evidence, SOP, semantic decomposition, dependencies, and provenance.
Incomplete artifacts are allowed; remaining work must be explicit.

This repository currently contains requirements and an initial risk analysis.
The [initial Rust prototype](implementation.md) implements a public-artifact
subset. Its tests establish specific software behavior, not compliance with the
complete product requirements or truth of publishers' evidence.

## Requirements hierarchy

The model follows [rules_requirements](https://github.com/Studio-Fug/rules_requirements),
using `refines` for a single-parent requirement tree. Existing REQ-001–078 remain
stable. Short titles support navigation; descriptions contain the requirements.

| Parent | Area | Model |
| --- | --- | --- |
| REQ-100 | System objective and user needs | [model.yaml](../requirements/model.yaml) |
| REQ-101 | Artifact identity, signatures, and provenance | [artifacts.yaml](../requirements/artifacts.yaml) |
| REQ-102 | Verification, SOPs, scope, and reproduction | [verification.yaml](../requirements/verification.yaml) |
| REQ-103 | Semantic capabilities, constraints, and search | [discovery.yaml](../requirements/discovery.yaml) |
| REQ-104 | Local storage and CACHE federation | [caches.yaml](../requirements/caches.yaml) |
| REQ-105 | Identity, access, and encrypted syndication | [access.yaml](../requirements/access.yaml) |
| REQ-106 | Physical offerings and paid access | [commerce.yaml](../requirements/commerce.yaml) |
| REQ-107 | Clients and Bazel/Nix integration | [interfaces.yaml](../requirements/interfaces.yaml) |
| RISK → MIT → REQ | Proposed abuse and integrity controls | [risks.yaml](../requirements/risks.yaml) |

Risk-control requirements REQ-079–094 have mitigation parents rather than a second
functional parent, preserving the toolkit's single evidence-rollup chain.
Their functional links and intended adversarial checks are in the
[risk analysis](risk-analysis.md).

## Core rules

- **Immutable identity.** All artifact content is committed by SHA-256, including
  labels and display-only explanations. Hash the payload, sign its hash, then
  hash the complete signed artifact. Internal commitments are not separate objects.
- **Traceable evolution.** Multiple predecessor hashes permit conceptual branching
  and merging. Contribution notes are non-authoritative. Reproduction reports and
  conceptual predecessors do not automatically supersede an original.
- **Specification-relative verification.** Full verification requires passing,
  applicable evidence covering every requirement and explicit rigor demand, plus
  a sufficient SOP. Simulation can suffice when rigor is unspecified. Assembly
  evidence can cover obligations of previously unverified components.
- **Visible claim scope.** Capabilities and constraints distinguish inference from
  evidence-supported claims; evidence identifies design, batch, or unit scope.
  Reported completeness, signature validity, and requester-checked evidence remain
  distinct. A signed report does not establish that a physical test occurred.
- **Controlled disclosure.** Specifications can be public; evidence and know-how
  have separate access and encryption scopes. Grants name one exact hash and scope.
  Permissions, payment state, and private keys are CACHE operational state.
- **Distinct offerings.** Buying a physical faucet does not buy its manufacturing
  know-how or component data. Integrators sell assembly-validation value. Hosts
  provide payments behind a uniform request/status/wait workflow and may grant
  upgrades without payment.
- **Federated delivery.** Search can use local and configured remote caches.
  Ordered substitution validates commitments. Distributors may serve ciphertext;
  the controlling CACHE authorizes key release.

## Delivery phases

| Phase | Scope |
| --- | --- |
| Initial | One CACHE process, local disk, agent API/CLI, external semantic decompositions, signed reports, public-key authentication, access/payment hooks, encrypted scopes and federation, Bazel/Nix |
| Subsequent | Human browser in this repository using the same API and use cases |
| Later | Scalable storage/execution and ZKP implementations behind initial extension hooks |
| Deferred decisions | Reputation, consensus, dispute arbitration, fulfillment and billing infrastructure |

Controls are proposed requirements for the relevant phase, not evidence that the
product already provides them. No risk has been accepted as resolved.

## Decision rationale and validation

The [consolidation record](consolidation.md) explains resolved contradictions
and remaining design questions. The [risk analysis](risk-analysis.md) covers
cheating, manipulation, confidentiality, malicious content, payments, and failures.

Validate the complete directory with an installed `rules_requirements` CLI:

```sh
rr validate requirements/
```

Pass the directory, not only `model.yaml`. Structural validation does not establish
product compliance or the truth of verification evidence.

## Why these choices

**Requirements and verification are the product's organizing principle.** Search
finds ways to satisfy needs, rather than matching product names alone. A pump,
aqueduct, and bucket can share a capability while differing in power, throughput,
or operating constraints. Each design pairs with its exact specification and
traceable report. Other specifications or improved labels produce new artifacts.

**Assembly validation is valuable in its own right.** An integrator establishes
that the whole meets the promised requirements. Previously untested components
can be screened and failures discarded. The report must identify the tested
scope; neither component secrecy nor a passing demonstration waives assembly
requirements. Component suppliers' contributions are not attributed to the integrator.

**Physical delivery and know-how are separate.** A faucet buyer may receive the
faucet and its report without the manufacturing recipe. Restricted evidence and
know-how have separate keys. Sellers can offer knowledge at a different price;
pricing and physical fulfillment are host responsibilities.

**Identity should preserve every decision-relevant bit.** Display-only text can
still influence an agent, so it is hashed. Reports, semantic labels, provenance
notes, ciphertext, and public encryption data cannot be silently replaced.
Operational policies and secrets are separate; access audit identifies the policy
used instead of pretending the artifact hash explains the commercial interaction.

**History records epistemology, not consensus.** Multiple predecessors record
conceptual contributions. Notes help readers but do not prove a causal story.
Independent reproductions, including failures, are new artifacts. A purported
reproduction can be unfaithful; preserve its conditions and authorship without
automatically changing the original's report or declaring a winner.

**Evidence has several distinct dimensions.** Specification coverage, per-claim
support, achieved rigor, physical scope, authenticity, and requester checking must
not collapse into one confidence score. Simulation can cover a specification
fully; that does not mean an individual product was physically tested.

**The initial economic boundary is access.** Hosts bring payments and issue
exact-hash grants. Sellers may recognize prior purchasers and grant upgrades.
Ciphertext may be syndicated while a controlling cache releases keys. Key release
cannot prevent a buyer from retaining or sharing plaintext.

**Start with a small core.** One process and local disk avoid an external database.
The API and CLI come first; the browser lives here and uses the same API.
Semantic decompositions come from external agents. Bazel and Nix are initial
integrations. Initial payment and proof hooks leave host processing and future
cryptography outside the core implementation.

## Lessons informing the abuse analysis

Marketplace reports resemble reviews, but add reproducible procedures and
requirement traces. Research on manipulation and biased feedback motivates
keeping report attribution and scope separate from popularity:

- Mayzlin, Dover, and Chevalier compare purchase-restricted and open review
  platforms and document patterns consistent with promotional manipulation:
  [Promotional Reviews](https://pubs.aeaweb.org/doi/abs/10.1257/aer.104.8.2421).
- Luca and Zervas find suspicious reviews associated with reputation and
  competitive incentives:
  [Fake It Till You Make It](https://people.bu.edu/zg/publications/fakereviews.pdf).
- Nosko and Tadelis examine biased feedback and platform-wide reputational effects:
  [The Limits of Reputation](https://www.nber.org/system/files/working_papers/w20830/w20830.pdf).

Our design inference is to retain evidence, reporter identity, reproduction
conditions, disclosed incentives, and conflicting outcomes before choosing a
reputation algorithm. Purchase history or high-effort video alone is not a durable
authenticity guarantee. These studies do not establish an optimal mechanism here.

Cryptographic proof can establish a defined statement over committed data, while
signatures authenticate origin and integrity; neither alone establishes physical
truth: [ZKP overview](https://ethereum.org/zero-knowledge-proofs),
[NIST signature definition](https://csrc.nist.gov/glossary/term/digital_signature).
Specific confidential commitments and future proof guarantees remain design work.
