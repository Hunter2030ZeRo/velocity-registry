# Additional CLI packages

These manifests pin upstream release assets. SHA-256 values were checked against the official GitHub release API `assets[].digest` fields. Archive layouts were checked against the tagged upstream packaging sources linked below. The archives were not downloaded or rehashed in the authoring environment because release-asset network access was unavailable; this is metadata/source verification, not an installation test.

| Package | Release | Packaging source | Targets |
| --- | --- | --- | --- |
| fzf | [v0.74.3](https://github.com/junegunn/fzf/releases/tag/v0.74.3) | [Source](https://github.com/junegunn/fzf/blob/v0.74.3/.goreleaser.yml) | 4 |
| zoxide | [v0.10.0](https://github.com/ajeetdsouza/zoxide/releases/tag/v0.10.0) | [Source](https://github.com/ajeetdsouza/zoxide/blob/v0.10.0/.github/workflows/release.yml) | 4 |
| just | [1.58.0](https://github.com/casey/just/releases/tag/1.58.0) | [Source](https://github.com/casey/just/blob/1.58.0/bin/package) | 4 |
| dust | [v1.2.5](https://github.com/bootandy/dust/releases/tag/v1.2.5) | [Source](https://github.com/bootandy/dust/blob/v1.2.5/.github/workflows/CICD.yml) | 6 |
| bottom | [0.14.9](https://github.com/ClementTsang/bottom/releases/tag/0.14.9) | [Source](https://github.com/ClementTsang/bottom/blob/0.14.9/.github/workflows/build_releases.yml) | 6 |
| hexyl | [v0.17.0](https://github.com/sharkdp/hexyl/releases/tag/v0.17.0) | [Source](https://github.com/sharkdp/hexyl/blob/v0.17.0/.github/workflows/CICD.yml) | 4 |
| uv | [0.12.10](https://github.com/astral-sh/uv/releases/tag/0.12.10) | [Source](https://github.com/astral-sh/uv/blob/0.12.10/.github/workflows/build-release-binaries.yml) | 6 |
| gh | [v2.100.0](https://github.com/cli/cli/releases/tag/v2.100.0) | [Source](https://github.com/cli/cli/blob/v2.100.0/.goreleaser.yml) | 4 |
| duf | [v0.9.1](https://github.com/muesli/duf/releases/tag/v0.9.1) | [Source](https://github.com/muesli/duf/blob/v0.9.1/.goreleaser.yml) | 4 |
| sd | [v1.1.0](https://github.com/chmln/sd/releases/tag/v1.1.0) | [Source](https://github.com/chmln/sd/blob/v1.1.0/.github/workflows/publish.yml) | 4 |

## Usage and target notes

- `bottom` exposes `btm`; `uv` exposes `uv` and `uvx`, plus `uvw.exe` on Windows.
- `gh` can use GitHub APIs without Git, but repository/Git operations require Git already installed on the system. This registry does not yet contain a Git package and cannot express external system prerequisites; no unresolved registry dependency is added.
- `zoxide` requires the user to enable shell integration with `zoxide init`; interactive selection additionally uses `fzf`. No shell startup files are changed by these manifests.
- `just` recipes may need a shell and programs chosen by the recipe author. This manifest installs the task runner only.
- Only upstream artifacts matching the registry's supported targets are included. In particular, musl-only Rust releases are not relabeled as GNU artifacts. Linux Go archives are conservatively registered for GNU targets only.
- `lazygit` is deferred until its required Git executable can be represented as a registry dependency. `tokei` is deferred because its latest release has no binary assets.

## Verification before publication

Run the existing `velocity-index validate` and `velocity-index build` commands (also run by PR CI). Before publication, download each new artifact, verify its SHA-256, and confirm the configured binary paths after stripping archive components. Platform execution has not been tested by this change.
