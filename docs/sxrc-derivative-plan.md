# HXNU SXRC-Derived Memory Compression Plan (Phase 4.5 Track)

## Goal

Build a kernel-safe, `no_std` SXRC-derived compressed-page subsystem for HXNU.

Target outcome:
- page-level compression for memory pressure handling (zram-like behavior)
- deterministic decode path with strict validation
- bounded metadata and bounded workspace in kernel hot paths

## Non-Goals (Initial Phase)

- direct import of userspace `sxrc` crate into kernel
- YAML parsing in kernel runtime
- dynamic/adaptive dictionary learning inside kernel hot paths
- full swap-to-disk implementation in the first iteration

## Why Derived, Not Direct Import

`sxrc` is strong as a profile/tooling and validation harness, but current crate design is userspace-oriented (`std`, `serde_yaml`, dynamic collections). Kernel integration must be a reduced and bounded derivative core.

Practical split:
- Keep `sxrc` repo as profile generator + benchmark + reference behavior.
- Build HXNU-side `no_std` codec/profile runtime with fixed tables and bounded memory.

## Proposed Architecture

### A. Userspace Profile Pipeline (outside kernel hot path)

Use `sxrc` tooling to generate static profile artifacts:
- compression unit
- endian mode
- static dictionary entries
- static short pattern table

Output format:
- generated Rust constants file committed into HXNU tree (or generated during build and vendored)

Suggested path:
- `tools/sxrc-profile-gen/` (host tool)
- output: `kernel/src/mm/compress/profile_generated.rs`

### B. Kernel Codec Core (`no_std`)

Add new modules:
- `kernel/src/mm/compress/mod.rs`
- `kernel/src/mm/compress/codec.rs`
- `kernel/src/mm/compress/header.rs`
- `kernel/src/mm/compress/checksum.rs`
- `kernel/src/mm/compress/profile_generated.rs`

Core page classes:
- `Zero` (all-zero page)
- `Same` (single-byte fill page)
- `Sxrc` (compressed payload using fixed profile)
- `Raw` (fallback)

Kernel invariants:
- fixed page size: `4096`
- no unbounded allocation in encode/decode fast path
- strict header validation + checksum verify + decoded length verify
- fail-safe fallback to `Raw` page when compression is not profitable or validation fails

### C. Compressed Page Store + Reclaim Hook

Add storage/policy module:
- `kernel/src/mm/compress/store.rs`

Responsibilities:
- hold compressed page records
- map page-id/frame-id to compressed payload metadata
- track counts (`zero/same/sxrc/raw`) and bytes saved

Then integrate with memory pressure/reclaim path:
- `kernel/src/mm/pager.rs` (new)
- `kernel/src/mm/mod.rs` exports: `frame`, `heap`, `compress`, `pager`

## Runtime Safety Contract

Must-have checks:
- header magic/version check
- checksum check before mapping decoded data as valid
- decoded size exactly equals page size
- reject malformed payload with deterministic error
- never panic in decode on untrusted/malformed payload

## Observability

Expose compact stats via `procfs`:
- `/proc/mmstat` or `/proc/compress`
- total pages processed
- compressed pages by class (`zero/same/sxrc/raw`)
- compression ratio (input/output bytes)
- decode failures, checksum failures, fallback count

## Acceptance Criteria

1. Correctness
- roundtrip encode/decode for 4 KiB pages across deterministic corpora
- malformed payload tests fail safely (error, no corruption)
- checksum mismatch is detected and counted

2. Boundedness
- no YAML, no `std`, no unbounded metadata growth in kernel path
- fixed worst-case workspace per operation

3. Integration
- reclaim path can store and restore at least one compressed page class end-to-end
- fallback to raw page preserves behavior under incompressible pages

4. Operational visibility
- procfs counters report meaningful transitions during pressure simulation

## Commit Plan (No Push)

1. `docs`: add SXRC-derived integration contract and MM boundary notes
2. `tools`: add profile generator skeleton and generated static profile artifact
3. `mm/compress`: add header/checksum/page-class structures + unit tests
4. `mm/compress`: add bounded encode/decode core with `Raw` fallback
5. `mm/store`: add compressed-page store and accounting
6. `mm/pager`: connect one reclaim/restore path using compressed store
7. `procfs`: add compression stats endpoint + smoke checks

## Risk Notes

- Early false confidence risk: strong synthetic ratio does not guarantee good mixed workload behavior.
- Fragmentation/perf risk: compression wins can be offset by decode latency without careful reclaim policy.
- Corruption risk: checksum and strict bounds checks are mandatory before marking restored pages valid.

## Immediate Next Step

Start with the minimal kernel profile:
- fixed 16-bit unit
- fixed endian per target
- static dictionary only
- `Zero`/`Same`/`Raw` + minimal `Sxrc` path

Then layer patterns and optimization after correctness + boundedness are stable.
