# Releasing MyLore

MISSION-099 pipeline. Everything version-related is single-sourced by
`scripts/release.mjs`; installers are built per-platform by GitHub Actions on
tag push (`.github/workflows/release.yml`).

## Cut a release

```bash
# 1. Bump the version everywhere (package.json, tauri.conf.json, Cargo.toml).
node scripts/release.mjs 0.2.0        # SemVer; pre-releases: 0.2.0-beta.1

# 2. Scaffold the CHANGELOG entry from commits since the last v* tag,
#    then edit it into user-facing notes.
node scripts/release.mjs changelog 0.2.0

# 3. Refresh Cargo.lock with the new local version and run the gates.
cargo check
npm test && npm run e2e && node scripts/release.mjs check

# 4. Commit, tag, push — CI builds the installers and drafts the release.
git add -A && git commit -m "chore(release): v0.2.0"
git tag v0.2.0 && git push origin main v0.2.0
```

## What CI builds

| Platform | Artifacts |
|----------|-----------|
| Windows  | NSIS `.exe` setup + MSI |
| macOS    | `.app` / `.dmg` (universal) |
| Linux    | `.deb`, `.rpm`, AppImage |

Artifacts attach to a **draft** GitHub Release (`prerelease` when the tag has
a pre-release segment like `-alpha.1`). Review, publish.

## Signing

Provisioning is env/secrets-gated — without secrets everything builds unsigned
(fine for alpha/beta):

- **Windows:** import a code-signing PFX via the `WINDOWS_PFX` /
  `WINDOWS_PFX_PASSWORD` secrets; reference the certificate in
  `tauri.conf.json → bundle.windows` (`certificateThumbprint` or
  `certificateSubjectName`).
- **macOS:** `APPLE_CERTIFICATE` (+ password) and `APPLE_SIGNING_IDENTITY`
  secrets enable codesign inside the workflow's keychain step; notarization
  can be added later via `APPLE_API_KEY` trio with `tauri.conf.json →
  bundle.macOS.notarize`.

## Gates before tagging

- `node scripts/release.mjs check` passes in CI as a hard gate (version sync).
- Full test matrix runs first in `.github/workflows/ci.yml` on main; release
  builds additionally re-run `npm ci` + build from a clean checkout.