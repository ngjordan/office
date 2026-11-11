# Office suite

One window, one egui context, five apps: **Sheets**, **Docs**, **Cad**, **Decks**, **PDF**.

This repository holds the Office host only. The five apps stay in their own repositories
and are pulled in as path dependencies, so the standalone apps keep shipping on their own.

| App | Repository | Branch | Path (relative to this repo) |
|---|---|---|---|
| Sheets | `ngjordan/sheets` | `feature/sheets-shell-library` | `../gc-gridcraft` |
| Docs | `ngjordan/docs` | `feature/docs-shell-library` | `../wc-wordcraft` |
| Cad | `ngjordan/cad` | `feature/cad-shell-library` | `../cc-cadcraft` |
| Decks | `ngjordan/decks` | `feature/decks-shell-library` | `../dc-deckcraft` |
| PDF | `ngjordan/pdf` | `feature/pdf-shell-library` | `../pdfcraft` |

Cloning this repo alone does **not** build: clone the five app repositories as siblings
of this directory, on the branches above.

## Build and test

```sh
cd office
export CARGO_BUILD_JOBS=4                      # this box OOMs on unbounded parallelism
export CRAFT_FONTS_DIR=/tmp/craft-fonts
export CRAFT_I18N_DIR=/tmp/craft-i18n
export PKG_CONFIG_PATH=/opt/hermes-agent/workspace/alsa-stub/pkgconfig
export LIBRARY_PATH=/opt/hermes-agent/workspace/alsa-stub/lib
umask 022
cargo test -p iridium-office -j 1
```

`-j 1` matters: the test binaries are ~2.8 GB with debug info, and `cargo test` links up
to `CARGO_BUILD_JOBS` of them at once — concurrent links get the build OOM-killed
(exit -9, no error message).

## Tests

| Test | What it holds down |
|---|---|
- `tests/five_in_one.rs` — Phase 0 regression gate: all five apps construct, paint and share one egui context.
- `tests/suite.rs` — Phase 2b: the switcher, file routing, and the look rules across a switch.
- `src/suite.rs` unit tests — the per-module look (style, zoom, fonts), using a fake module that owns its look.

## Release

`.github/workflows/release.yml` builds the macOS universal app and the Windows x64 and
arm64 installers. It runs the `release` **environment**, which only the `main` and
`release` branches may deploy to, so the signing credentials never reach a run from
somewhere else.

Secrets (all in the `release` environment, *Settings > Environments > release*):

| Secret | What it is |
|---|---|
| `APPLE_CERTIFICATE` | the Developer ID `.p12`, base64 |
| `APPLE_CERTIFICATE_PASSWORD` | the `.p12` password |
| `KEYCHAIN_PASSWORD` | the temporary keychain's password |
| `APPLE_API_KEY` | the App Store Connect `.p8`, base64, no newlines |
| `APPLE_API_KEY_ID` | the key id |
| `APPLE_API_ISSUER_ID` | the issuer id |
| `AZURE_TENANT_ID` | the Entra ID tenant |
| `AZURE_CLIENT_ID` | the app registration's client id |
| `AZURE_CLIENT_SECRET` | the app registration's client secret |
| `AZURE_SIGNING_ENDPOINT` | e.g. `https://eus.codesigning.azure.net` |
| `AZURE_SIGNING_ACCOUNT` | the Trusted Signing account name |
| `AZURE_CERT_PROFILE` | the certificate profile name |

The Apple notarization works with either the API key (`APPLE_API_*`) or an Apple ID
with an app-specific password (`APPLE_ID` + `APPLE_PASSWORD` + `APPLE_TEAM_ID`); the
API key is preferred, because it does not expire and needs no interactive 2FA. With
neither, the app is still signed, but not notarized, and the release notes say so.

Windows signing uses Azure Artifact Signing (formerly Trusted Signing) when the `AZURE_*`
secrets are set, and a CA-issued `.pfx` (`WINDOWS_CERTIFICATE` +
`WINDOWS_CERTIFICATE_PASSWORD`) otherwise. With neither, the installer ships unsigned.
`packaging/windows/set_signing_secrets.py` stores the `.pfx` pair in the environment.

Buying a certificate from a CA rather than using Azure:

- **SSL.com** still documents the plain `.pfx` route - `signtool sign /fd sha256 /f cert.pfx
  /p password` - which is exactly what the signing script does. Their `eSigner` cloud HSM
  issues the `.pfx` for CI use.
- **DigiCert** (Keypair) and **Sectigo** keep the key in a FIPS token or their own cloud
  HSM and sign through a provider DLL, not a `.pfx`. The signing script would need that
  provider's DLL and metadata format, like the Azure path.
- Since **1 March 2026** a publicly trusted code-signing certificate may not be issued for
  more than **460 days**, so the certificate is renewed every year or so and the secret has
  to be replaced when it is.
- An OV certificate is issued to an organisation, so the publisher name comes from the
  business registration. An individual code-signing certificate is much harder to get.

Two things to know about Artifact Signing:

- **Use the Public Trust model.** A Private Trust certificate is not in any root program, so
  Windows treats it like a self-signed one. Public Trust is issued under the Microsoft Root
  Certificate Program and is what a public download needs.
- **The certificate names the publisher, not the product.** The subject is the validated
  identity - the legal entity, or the individual's legal name - and Azure does not let you
  set a custom one. The product name comes from `signtool /d` and from the binary's
  VERSIONINFO (`ProductName`, `CompanyName`), both set in `build.rs`.

SmartScreen still warns on a brand new binary until the publisher has reputation, which
builds over weeks of clean downloads. Signing means the prompt names the publisher instead
of saying "Unknown Publisher", and later releases inherit that reputation; an unsigned file
has to rebuild it from zero every time.


## CI and the dependency policy

`.github/workflows/ci.yml` runs the same gate locally: format, clippy, dependency policy
(`deny.toml`), and the tests. It checks the five apps out as siblings, so it needs a PAT
with `repo` scope in the repository secret **`OFFICE_TOKEN`** — the default `GITHUB_TOKEN`
cannot read the app repositories.

`deny.toml` is the PDF repo's policy with the merged graph's differences:

- `MPL-2.0` is allowed: CAD's DWG reader (`acadrust`) and Decks' media decoder
  (`symphonia`) already ship it in their standalone builds.
- `storytold/winprint.rs` is allowed: the merged graph still resolves the upstream
  spooler at four revisions, because redirecting them to the mirror makes Cargo report
  `specification 'winprint' is ambiguous`.
- The wildcard-version check warns instead of failing: the four repos that are not PDF
  write their internal path dependencies as `version = "*"`.


## How the look works

The five apps share ONE egui context, and each replaces that context's fonts, style and
zoom with its own. Each app decides whether to re-install its look against **its own**
memory, not against the context — so an app that has already installed its look will not
install it again, and would otherwise draw in whatever look the previous app left behind.

The suite therefore keeps a **live** look per module:

1. background modules run their logic first, the active module last;
2. each module's look is restored **before** its own logic and captured **after** it;
3. the active module's look is restored again before it draws.

A construction-time snapshot is deliberately avoided: it would revert the user's runtime
theme or interface-size change and would drop faces an app added while running (PDF's
document fonts).

egui 0.37 details that shape this: styles and theme apply immediately, while
`set_fonts` and `set_zoom_factor` land a pass later and their pending values are private —
so only applied values can be read back. The zoom is written straight into
`Options::zoom_factor`, because egui's `set_zoom_factor` requests a repaint on every call
and the suite re-issues it every frame.
