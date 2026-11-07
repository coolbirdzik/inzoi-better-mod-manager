# Building and releasing

[Back to README](../README.md)

The release flow follows the same pattern as DebridDock (`realdebrid`): synchronize versions, create a release commit and annotated tag, push `main` and the tag together, then let GitHub Actions test, bundle and publish.

## GitHub Actions

The single [Build and release workflow](../.github/workflows/release.yml) handles:

| Event | Checks | Desktop artifacts | Public release |
| --- | --- | --- | --- |
| Push to `main` | Windows + macOS | Windows x64, macOS ARM64 + Intel | No |
| Pull request | Windows + macOS | Windows x64, macOS ARM64 + Intel | No |
| Manual workflow run | Windows + macOS | Windows x64, macOS ARM64 + Intel | No |
| Push `vX.Y.Z` tag | Windows + macOS | Same packages, with signed updater files | After all checks pass |

The `Release vX.Y.Z` commit on `main` skips its duplicate build because the release script pushes that commit and its tag atomically. The tag run performs the full pipeline. Ordinary branch/PR runs can be cancelled when superseded; tag runs are not cancelled automatically.

1. **Test:** check versions, release tooling, dev server behavior, all translation catalogs, frontend build, Playwright UI tests, Rust tests and the release verifier.
2. **Bundle:** build the Windows NSIS installer and portable ZIP; build separate macOS Apple Silicon and Intel apps/DMGs. A portable package reuses the installer build's executable.
3. **Collect:** give each architecture a unique filename and retain its artifact for 14 days. No architecture can overwrite another's update archive.
4. **Verify and publish (tag push only):** download artifacts from the same workflow run, generate the four-platform `latest.json` and SHA-256 checksums, verify every updater signature and signed version using the public key in the tagged source, upload to a draft, then mark it public and Latest.

The publisher rejects missing platforms, missing installers/signatures, mismatched tags or versions, invalid signatures, untrusted release URLs and already-public releases. If upload fails after a draft was created, re-run the failed job to finish that draft. Create a new version to replace an already-public release.

`contents: write` is granted only to the publishing job. Branch/PR builds do not need signing secrets. The old separate verify/publish workflows have been consolidated into this pipeline.

## Configure signing once

In the repository's **Settings → Secrets and variables → Actions**, configure:

| Secret | Purpose |
| --- | --- |
| `TAURI_SIGNING_PRIVATE_KEY` | Tauri updater private key matching `plugins.updater.pubkey` in `src-tauri/tauri.conf.json` |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Password for that key, if it has one |

Keep the current updater key for continuity with installed versions. Do not generate a replacement just to make CI pass. Never commit a private key. `GITHUB_TOKEN` is supplied by Actions and needs no personal access token.

Updater signatures authenticate update packages. They are separate from Windows publisher certificates and Apple Developer signing/notarization. This workflow does not configure either publisher certificate; the macOS packages are not notarized. See the official [Tauri updater](https://v2.tauri.app/plugin/updater/) and [macOS signing](https://v2.tauri.app/distribute/sign/macos/) guides for those mechanisms.

## Cut a release

Commit the intended feature/documentation changes first. Run from a clean `main` branch:

```sh
pnpm run release patch --dry-run
pnpm run release patch
```

Use `minor`, `major` or an exact stable version such as `0.2.0` instead of `patch`. The dry run makes no changes and no network requests, and can preview a dirty checkout. A real release:

1. Checks that `main` is up to date, the tag is unused and version files agree.
2. Runs the local release checks.
3. Asks for terminal confirmation.
4. Updates `package.json`, `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml` and the app entry in `src-tauri/Cargo.lock`.
5. Creates `Release vX.Y.Z`, an annotated `vX.Y.Z` tag, and atomically pushes `main` plus the tag.

The pnpm lockfile does not store the root package version and is left unchanged by the version bump. Pre-release/build-metadata version suffixes are currently rejected.

Other commands:

```sh
pnpm run release 0.2.0 --no-push  # Commit/tag locally, without pushing
pnpm run release:check          # Run the local checks, without releasing
pnpm run build:release          # Build the current version on this OS
```

`--yes` permits noninteractive release execution. `--skip-checks` skips local tests; CI still tests the tagged source. If the atomic push fails, the script keeps its local release commit/tag and prints the retry command. A failure before the commit restores only the script's version edits.

The same commands are available under **Tasks: Run Task → Release…** in VS Code.

## Release assets

Every name starts with `inZOI-Better-Mod-Manager_X.Y.Z_`:

| Suffix | Use |
| --- | --- |
| `windows-x64-setup.exe` | Windows x64 installer and installed-app updater |
| `windows-x64-portable.zip` | Windows x64 portable app and portable updater |
| `macos-arm64.dmg` | Apple Silicon installer |
| `macos-x64.dmg` | Intel Mac installer |
| `macos-arm64.app.tar.gz` / `macos-x64.app.tar.gz` | macOS updater payloads |
| `.sig` beside each updater payload | Tauri signature with the signed app version |

`latest.json` includes `windows-x86_64`, `windows-x86_64-portable`, `darwin-aarch64` and `darwin-x86_64`. The existing app update endpoint remains `/releases/latest/download/latest.json`. `SHA256SUMS.txt` covers every distributed package/signature and the manifest.

The artifact collector and manifest generator live in [scripts/release-artifacts.mjs](../scripts/release-artifacts.mjs). The independent Rust signature verifier lives in [tools/verify-release](../tools/verify-release). Verification never executes a downloaded installer.

## Local builds and validation

```sh
pnpm install --frozen-lockfile
pnpm run test:release
pnpm run test:dev
pnpm run test:i18n
pnpm run build
pnpm exec playwright install chromium
pnpm run test:ui
cargo test --locked --manifest-path src-tauri/Cargo.toml
cargo test --locked --manifest-path tools/verify-release/Cargo.toml
```

On Windows, `INZOIMM_TEST_BROWSER=msedge` selects an installed Edge browser instead of Playwright Chromium. The release helper selects Edge automatically when available.

Build an NSIS installer and portable ZIP on Windows with `pnpm run build:release`. On macOS, that command produces a native app and DMG; CI produces both macOS architectures. A Windows build cannot verify a macOS bundle or its runtime behavior. GitHub-hosted macOS jobs must run to validate those builds.
