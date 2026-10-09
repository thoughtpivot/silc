# Releasing Silc

Silc releases are cut by [semantic-release](https://github.com/semantic-release/semantic-release) after CI succeeds on `main` or, when the branch exists, `dev`. Merging a release-worthy commit to `main` tags `vX.Y.Z`, updates [CHANGELOG.md](../CHANGELOG.md), bumps the published crate manifests, and opens a GitHub release. No release pull request is required.

`thoughtpivot/vran` is private. This repo follows the same release shape with standard semantic-release: conventional commits, stable tags on the default branch, prerelease tags on `dev`, a changelog, and a back-merge into `dev`.

## Version rules

| Commit | Release |
| --- | --- |
| `fix:` | patch `vX.Y.(Z+1)` |
| `feat:` | minor `vX.(Y+1).0` |
| `feat!:` or a `BREAKING CHANGE:` footer | major `v(X+1).0.0` |
| `perf:` | patch |
| `chore:` | no release |
| `ci:`, `docs:`, `style:`, `refactor:`, `test:`, `build:` | no release |

A breaking change is a major bump on 0.x as well. `feat!:` on 0.7.0 publishes 1.0.0.

The release commit is `chore(release): X.Y.Z`, so it does not cut another release. It does not contain `[skip ci]`: the tag push has to start the crates.io job.

Release commits and the baseline tag are written as **Dan Stephenson &lt;ispyhumanfly@gmail.com&gt;** (`GIT_AUTHOR_NAME`, `GIT_AUTHOR_EMAIL`, `GIT_COMMITTER_NAME`, `GIT_COMMITTER_EMAIL` on the release job). They must not use a bot name or an AI trailer.

## Branches

- `main` publishes stable `vX.Y.Z` tags and GitHub releases.
- `dev`, if it exists, publishes prerelease tags `vX.Y.Z-dev.N` and GitHub prereleases. There is no `dev` branch today; the workflow checks for `origin/dev` and skips prereleases and the back-merge until one exists.
- After a stable release, `main` is merged into `dev` with `chore: back-merge main into dev after vX.Y.Z` when `dev` exists.

The first release run on `main` finds no `vX.Y.Z` tag (existing tags are per-crate, such as `sil-core-v0.7.0`). It creates an annotated tag `v<workspace version>` on that tested commit so later releases start from the published line instead of replaying older history. The tag push still runs the crates.io job. Versions already on crates.io are skipped. `sil-rlm` 0.7.0 and `silc` 0.7.0 are not on crates.io yet (both are still 0.6.0 there), so that first tag publishes those two crates.

## What is versioned

The release updates:

- `[workspace.package] version` in `Cargo.toml`
- the matching `version` fields under `[workspace.dependencies]` (path crates)
- those path packages in `Cargo.lock`
- `CHANGELOG.md`

`sil-ide` and `sil-lsp` stay `publish = false`. Every other workspace crate is published to crates.io from the stable tag, in dependency order. The editor extension (`editors/vscode-silc/package.json`) is not part of this release.

`@version("...")` in `.silc` sources is the language pin, not the crate version. A patch release does not rewrite examples or that pin.

Per-crate `CHANGELOG.md` files under `crates/` are the older release-plz notes. New notes go in the repository [CHANGELOG.md](../CHANGELOG.md).

## Tokens

The default `GITHUB_TOKEN` cannot start other workflows when it pushes a tag. Crates.io publishing runs on the tag push, so the release job needs a credential whose pushes do trigger workflows.

### GitHub App (preferred)

Create a GitHub App installed only on `thoughtpivot/silc`.

| Setting | Value |
| --- | --- |
| Repository permission | Contents: Read and write |
| Installation | `thoughtpivot/silc` only |
| Where metadata is not a secret | Client ID |

Store:

| Name | Kind | Value |
| --- | --- | --- |
| `SILC_RELEASE_APP_CLIENT_ID` | repository variable | GitHub App client ID |
| `SILC_RELEASE_APP_PRIVATE_KEY` | repository secret | PEM private key, including the BEGIN and END lines |

The release workflow mints a short-lived installation token with `actions/create-github-app-token`. That token pushes the release commit and the `vX.Y.Z` tag. Git author and committer stay Dan Stephenson even though the token belongs to the app. The GitHub release entry is opened by the app.

If `main` or `dev` requires a pull request before pushing, allow this app to bypass that rule. semantic-release pushes the release commit straight to the branch.

### Personal access token (alternative)

If the app is not set up, set repository secret `SILC_RELEASE_TOKEN` to a fine-grained PAT from Dan Stephenson (`ispyhumanfly`) with Contents read and write on `thoughtpivot/silc`. The workflow uses it when `SILC_RELEASE_APP_CLIENT_ID` is empty. A PAT push also triggers the tag workflow, and the GitHub release is opened by that user.

Leave `SILC_RELEASE_TOKEN` unset when the GitHub App is configured. The app token wins when both are present.

Do not put either value in the repository, the changelog, or commit messages.

## crates.io

Stable tags publish from `.github/workflows/ci.yml`, job id `release-plz-release` (the name shown in the Actions UI is "publish crates.io"). Trusted Publishing matches the workflow filename and the environment. Keep each published crate's trusted publisher pointed at:

| Field | Value |
| --- | --- |
| Repository owner | `thoughtpivot` |
| Repository name | `silc` |
| Workflow filename | `ci.yml` |
| Environment | empty |

No long-lived `CARGO_REGISTRY_TOKEN` secret. The job exchanges the GitHub OIDC token with `rust-lang/crates-io-auth-action`. A crate's first publish still needs an API token once; trusted publishing can only update a crate that already exists. `sil-ide` and `sil-lsp` are not published.

Prerelease tags do not publish to crates.io. A stable tag whose versions are already on crates.io (the `v0.7.0` baseline) does not publish again.

## Public notes

Silc is public and Apache-2.0. Release notes are the Conventional Commit subjects since the previous `v*` tag. Do not put credentials, customer data, or private plans in commit messages; they are copied into `CHANGELOG.md` and the GitHub release.
