# Contributing

Add package manifests under `packages/` using `packages/_template.toml.example` as the starting point.

Before submitting a change, run:

```sh
cargo run -p velocity-index -- validate --packages packages
cargo run -p velocity-index -- build --packages packages
```

Use canonical lowercase package names. Package aliases must not collide with other package names or aliases. Each artifact must use HTTPS and include a SHA-256 checksum. Windows and Linux artifacts for the same upstream project should share one logical package manifest.
