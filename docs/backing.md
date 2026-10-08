# Git-backed content

An artifact can commit to external design or report content while a cache stores
only its signed manifest. The artifact address hashes that manifest, including
each source descriptor and its SHA-256 realization commitment. Fetching checks
the realized content against that commitment. Changing a URL, revision, selection,
format or commitment produces a new artifact; optional local realization does not.

Version 2 adds `sources` alongside inline `design` and `records`. A descriptor is:

```json
{
  "path": "source",
  "role": "design",
  "repository": "https://github.com/owner/repository",
  "revision": "<full lowercase 40- or 64-digit Git commit ID>",
  "archive": "https://codeload.github.com/owner/repository/tar.gz/<commit ID>",
  "subdirectory": "src",
  "format": "git_tar_gzip_v1",
  "sha256": "<64 lowercase hexadecimal digits>"
}
```

`role` is `design` or `record`; `path` is a relative export prefix and, for a
record source, the identifier referenced by evidence. An empty `subdirectory`
selects the whole tree. HTTPS archive endpoints from other Git hosts also work.
The endpoint path must include the pinned commit ID. Repository/revision provenance
is a publisher assertion: the checked tree hash establishes content identity,
not cryptographic proof that a forge associated those bytes with that commit.

Design descriptors enter the verification subject hash. Report descriptors enter
the enclosing artifact hash but not that subject, avoiding circular evidence.
Signatures and subject hashes have version-2 domains. Version-1 serialization,
signature domains and addresses remain unchanged; sources require version 2.

## Author and consume

Download a pinned source archive and compute its commitment:

```sh
curl --fail --location 'https://codeload.github.com/owner/repository/tar.gz/FULL_COMMIT_ID' --output source.tar.gz
knowledge source-hash --input source.tar.gz --subdirectory src
```

Put the returned hash and source selection in a version-2 payload. Compute its
subject, attach relevant evidence, sign and insert it using the ordinary commands
in [implementation.md](implementation.md). The cache checks the signed manifest;
it does not fetch sources during insertion, search or ordinary retrieval.

Consumers explicitly select network origins and a new export directory:

```sh
knowledge realize ARTIFACT_SHA256 --output realized --allow-origin https://codeload.github.com
```

Add `--substituter CACHE_URL` to obtain the exact manifest from another cache.
Files export under `design/<path>` or `records/<path>`. All sources must pass
before the output directory is published. Existing output is refused. Downloads
use HTTPS, no redirects, a 30-second timeout and bounded responses. No source
network access occurs without explicit origin permission. Allowlisted origins
are trusted network destinations; this is not a comprehensive SSRF firewall.

Search and inspect say external content is unchecked and keep `reusable: false`,
including publisher-reported complete artifacts from trusted publishers. Realize
reports checked content commitments and signatures, not rerun tests or physical
truth. Failed fetching or hash checking fails the command without rewriting the
artifact. Clients retain the realization result only for that exact artifact.

## Deterministic realization

`git_tar_gzip_v1` accepts one gzip-compressed tar tree with one wrapper directory.
It selects the requested subdirectory, strips the wrapper and selection prefix,
then sorts file paths by UTF-8 bytes. The hash frame is:

1. Domain bytes `knowledge:git-tree:v1` followed by a zero byte.
2. File count as an unsigned 64-bit big-endian integer.
3. For each file: path byte length (u64 BE), path bytes, one executable byte
   (0 or 1), content length (u64 BE), and content bytes.

Tar ordering, wrapper name, timestamps, ownership, non-executable permission bits,
and empty directories do not affect identity. Content, paths and executability do.
Paths must be UTF-8, relative and normalized. Links, special files, duplicate
paths, and files that collide with directories are rejected. Archives are read
without unpacking or executing them. Export creates fresh files, with owner-only
permissions. This format is intentionally distinct from Nix NAR hashing.

Limits: 32 MiB compressed archive, 64 MiB tree, 16 MiB per file, 10,000 archive
entries, and 64 MiB combined export. Sources are constituent data of artifacts;
they are not separately indexed database objects. Realizations stay outside cache
storage and may be discarded without changing identity.

## Initial limits and abuse risks

Public Git source archives are the first transport. Direct Git/SSH cloning,
private-source credentials, symlinks, submodule resolution and Git LFS object
materialization are unsupported. Pointer files are ordinary committed bytes;
Knowledge never silently follows them. Publishers must define what their selected
tree represents. Availability still depends on the source host, so retain bytes
elsewhere when long-term availability matters.

A malicious publisher can commit to convincing false reports. Hash checking proves
byte consistency; it does not authenticate measurements. Resource limits and
path validation constrain hostile archives. Origin permission controls network
contact; operators must restrict it for their environment. Fetching grants no
right to execute designs. Paid/confidential access remains outside this public
prototype. Requirement links and specific tests are in [traceability.md](traceability.md).
