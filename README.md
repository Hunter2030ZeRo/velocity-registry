# velocity-registry

Package metadata registry for **Velocity**, kept intentionally separate from the Velocity client/runtime source tree.

The registry does **not** mirror third-party application binaries. Human-maintained manifests point at upstream/vendor HTTPS artifacts and pin their SHA-256 hashes. CI validates those manifests and compiles them into a compact client-facing index.

## Repository model

```text
packages/*.toml
      │
      │ pull request / review
      ▼
GitHub Actions
      │
      ├─ manifest validation
      ├─ cross-package dependency validation
      ├─ stable PackageId resolution
      ├─ dependency-cycle detection
      └─ binary index compilation
      │
      ▼
registry branch
├── registry.json
├── velocity.idx.zst
└── velocity.idx.zst.sha256
      │
      ▼
Velocity client
```

The Velocity client should consume the generated registry branch rather than clone this source repository or parse thousands of TOML files itself.

## Why a compiled index?

Velocity is intended to minimize time-to-first-fetch and time-to-installed, not only raw download time. Registry compilation moves work out of the client hot path:

- package names and aliases are resolved ahead of time;
- dependencies are converted to stable integer `PackageId`s;
- manifest structure is validated in CI;
- the complete client database is serialized once and zstd-compressed;
- clients can cache one compact index instead of traversing a Git tree of manifests.

The v1 index uses a versioned bincode payload. The format can later move to a zero-copy representation without changing the human-facing TOML manifest format.

## Layout

```text
.
├── packages/                     # package manifests (source of truth)
├── schema/package.schema.json    # documented schema
├── crates/
│   ├── velocity-manifest/        # TOML model + manifest validation
│   └── velocity-index/           # repository validation + index compiler
├── docs/
│   ├── FORMAT.md
│   └── MANIFEST_EXAMPLE.md
└── .github/workflows/
    ├── validate.yml
    └── publish.yml
```

## Local commands

```sh
cargo run -p velocity-index -- validate --packages packages
cargo run -p velocity-index -- build --packages packages
```

The build command writes `generated/velocity.idx.zst` and a matching `.sha256` file by default.

## Package manifests

Start from `packages/_template.toml.example`. A logical package can contain target-specific artifacts for Windows and Linux. Initial target triples are:

- `x86_64-pc-windows-msvc`
- `aarch64-pc-windows-msvc`
- `x86_64-unknown-linux-gnu`
- `aarch64-unknown-linux-gnu`
- `x86_64-unknown-linux-musl`
- `aarch64-unknown-linux-musl`

No arbitrary install scripts are part of schema v1. The first registry format stays declarative so the client can validate, plan, cache, and eventually transact installs predictably. A constrained recipe layer can be added later if real packages require it.

## Publication

On every relevant push to `main`, `publish.yml` builds a fresh registry and force-publishes only generated files to the `registry` branch. This keeps reviewable package source and machine distribution separate while requiring no dedicated package-registry server.

## Current state

This is the initial registry scaffold. `packages/` is intentionally empty until the first manifests are populated with verified upstream URLs and checksums.
