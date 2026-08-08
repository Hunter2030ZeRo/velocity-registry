# Velocity Registry Format v1

Velocity keeps human-maintained package manifests separate from the machine-facing registry index.

## Source format

Each package lives in `packages/<name>.toml`. The source format is intentionally declarative: package metadata, version constraints, target-specific artifacts, hashes, and binary exposure rules are data rather than arbitrary install scripts.

The initial target set is:

- `x86_64-pc-windows-msvc`
- `aarch64-pc-windows-msvc`
- `x86_64-unknown-linux-gnu`
- `aarch64-unknown-linux-gnu`
- `x86_64-unknown-linux-musl`
- `aarch64-unknown-linux-musl`

## Package identity

Canonical package names and aliases are normalized at registry-build time. The compiler derives a stable 64-bit FNV-1a ID from the canonical package name and rejects collisions.

Dependencies are resolved from names/aliases to IDs before publication. This moves repeated string-heavy identity resolution out of the client hot path.

## Compiled index

`velocity-index build` emits:

- `velocity.idx.zst` — zstd-compressed binary registry
- `velocity.idx.zst.sha256` — integrity checksum for the compiled registry

The uncompressed stream begins with:

1. `VLTIDX1\0` magic (8 bytes)
2. little-endian index format version (`u32`)
3. bincode-encoded index payload

Bincode is a pragmatic v1 representation. The client-facing format is explicitly versioned so a later zero-copy layout can replace it without changing the human manifest format.

## Publication model

The `main` branch contains reviewable source manifests and tooling. CI publishes compiled artifacts to a generated `registry` branch. Velocity clients should consume the compiled branch rather than clone or parse the manifest repository.
