# Security Notes — secrets policy (open-source contract)

MyLore is fully open source. This document is the binding rule for how any
credential is handled, so the answer to "could this leak?" is always
structural, never hopeful.

## 1. Classification

| Credential | Class | Storage | In git? |
|---|---|---|---|
| AniList **OAuth client secret** | App identity — public-knowledge for desktop apps (RFC 8252 §8.4) | `{app_data_dir}/anilist.client-secret` (one line) or `ANILIST_CLIENT_SECRET` env. Never in source. | ❌ never |
| User **access tokens** (AniList etc.) | User data — highly sensitive | OS-level secret pipeline (`FileSecretStore` under the app-data dir / keyring). Never sent to the frontend, never logged. | ❌ never |
| `MYLORE_DB_KEY` (optional at-rest passphrase) | Unlocks the whole library | Provided via environment at launch; never persisted, never logged. Lose it = lose the data. | ❌ never (env only) |
| TMDB / Google Books keys | Per-user, user-entered in Settings | Same secret pipeline as above | ❌ never |

## 2. The rules (enforced)

1. **No credential literals in source.** A unit guard test fails the build if
   a `CLIENT_SECRET` literal reappears (`oauth.rs` tests).
2. Runtime loading order: app-data file → environment variable → feature
   degrades with a clear error message (manual paste path keeps working).
3. Tokens are write-only from the UI's perspective: settings rows expose
   `has_key`, never the value.
4. Logs must never carry tokens or secrets; coordinator/bridge logs carry
   lengths and booleans only.

## 3. Why the client secret being public is acceptable — and still rotated

For native/OSS apps the client secret only authenticates *the application*
to the provider, not the user. Worst case of exposure is impersonating the
app's registration (phishing look-alikes, rate-limit abuse on its quota).
The user's account stays protected by their own token + AniList auth.

Rotation procedure (maintainers): delete & recreate the API client on
anilist.co/settings/developer → update your local
`anilist.client-secret` file → done. Users are unaffected.

## 4. If you find a leaked credential

Open a minimal issue referencing this file (no value), or contact the
maintainer privately; rotate first, commit explanations after.
