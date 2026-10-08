# RF combiner public proof of concept

This version-2 Knowledge object represents the [experimental 28 GHz divider](https://github.com/Studio-Fug/28ghz-2way-power-divider) at commit `66c6e169a4a4493971753f8e23b1d327d4e11174`. It demonstrates requirements, evidence and Git-backed design records; it is not a passing or independently certified RF design.

The object contains 15 requirements and 10 publisher-reported evidence entries: seven pass, three fail, and five requirements have no completed evidence. DUT return loss and insertion/combining loss fail acceptance. Whole populated assembly simulation and physical/power qualification remain outstanding. Existing simulation results apply to the DUT window, not the connector mating planes. Inspection evidence uses the conservative `analysis` rigor category.

Three immutable Git sources commit the fabrication board, validation reports, and simulation results using Knowledge tree hashes. The reports reference the retained Touchstone/NPZ datasets. YAPNR remains an external dependency. Its missing whole-assembly SSoT/adapters are tracked in [YAPNR #97](https://github.com/Studio-Fug/yapnr/issues/97).

The signature uses a prototype publisher identity. It authenticates this translation, not an independently authenticated original simulation operator or the Studio-Fug organization. No private key is distributed. Knowledge verifies content, signatures and trace structure; realization does not rerun simulations.

Artifact address: `9cdb703565b6e97f87064670c3646c4d566506af4e155964fe5069bd6695b88e`.
Subject hash: `17c7899537adb3e669f6a502e5c591e4fbd789ab6886cb34bdcc026a1f7f00b6`.

Use `knowledge inspect --help`, `knowledge put --help`, and `knowledge realize --help` for CLI options. Inspect the artifact, insert it into a local cache, and search with incomplete results enabled. Default search excludes it. Realization requires explicitly allowing `https://codeload.github.com`; all three sources were successfully materialized and checked (1,731,964 bytes). The resulting status is `failed` and `reusable: false`.

The divider repository provides `scripts/export_knowledge_object.py` and `knowledge/source-commitments.json` to reproduce the payload. Generate a private prototype key locally and seal the payload with Knowledge; a new key changes the artifact address. Do not commit private keys. Regeneration should use the pinned design revision and its matching source commitments.
