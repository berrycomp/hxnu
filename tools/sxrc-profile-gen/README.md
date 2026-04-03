# sxrc-profile-gen

Host-side skeleton generator for HXNU SXRC-derived static kernel profiles.

Current behavior (skeleton):
- emits a deterministic Rust constants file for kernel integration
- does not parse YAML manifests yet
- supports a single built-in profile (`minimal-v1`)

Usage:

```bash
HOST_TRIPLE="$(rustc -vV | awk '/host:/ {print $2}')"
cargo run --manifest-path tools/sxrc-profile-gen/Cargo.toml --target "$HOST_TRIPLE" -- \
  --out kernel/src/mm/compress/profile_generated.rs \
  --profile minimal-v1
```
