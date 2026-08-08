# Manifest example

This is an illustrative shape only; do not copy the URL or hash verbatim into `packages/`.

```toml
schema = 1
name = "example"
version = "1.2.3"
description = "Example CLI"
homepage = "https://example.com"
license = "MIT"
aliases = ["example-cli"]

[[dependencies]]
name = "runtime"
version = ">=1.0"

[[artifacts]]
target = "x86_64-pc-windows-msvc"
url = "https://example.com/example-1.2.3-windows.zip"
sha256 = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
archive = "zip"
strip_components = 1

[[artifacts.binaries]]
source = "example.exe"
name = "example"
```
