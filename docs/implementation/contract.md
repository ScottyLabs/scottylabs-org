# Implementation contract

This is the working agreement for building the site content platform described in `docs/site-content-portal-spec.md`. The spec says what to build and why. This file pins the shapes and rules every part of the code shares, so pieces built in parallel fit together.

If something here conflicts with the spec, this file wins for code. Record any change you make to it in the "Changes" section at the end.

## Architecture

One Rust service, `services/site`, serves everything on one origin:

| Path | What |
| -- | -- |
| `/api/...` | JSON API |
| `/media/<uuid>?w=480\|960\|1920` | Uploaded images |
| everything else | The SvelteKit build in `STATIC_DIR` |

This replaces the static `sites.web` deployment. Reasons, recorded in the spec's section 6: project pages at `/projects/<slug>/` need a server fallback; one origin keeps the session cookie first-party and needs no CORS; and each preview deploys as one URL that works end to end.

`sites/web` is one SvelteKit app containing the public site and the portal:

- Public pages live in the `(site)` route group and are prerendered. Pages that show managed content fetch it from `/api/public/...` in the browser.
- `/projects/[slug]/` is not prerendered. The service answers it with the SPA fallback `200.html`, with the project's title and description injected, and status 404 when no published project has that slug.
- `/committees/[slug]/` is prerendered for the seven slugs.
- The portal lives under `/portal/` with `ssr = false` and `prerender = false`, so the service answers every portal path with `200.html`.

Local development runs Vite on port 5173 and the service on 127.0.0.1:8080. Vite proxies `/api` and `/media` to the service, so the browser sees one origin, as in production.

## Environment

| Variable | Where it comes from | Required |
| -- | -- | -- |
| `DATABASE_URL`, `PGUSER` | kennel, or the dev shell | yes. The service starts without the database and connects in the background (see Changes, "2026-11-25, security audit fixes: availability and resilience") |
| `DATABASE_POOL_SIZE` | anywhere | optional. Request pool connections, 1 to 50, default 10 |
| `APP_URL` | kennel; the dev shell sets `http://localhost:5173` | yes. The service refuses to start without it |
| `PORT` | kennel. Binds `127.0.0.1:$PORT`; kennel's Caddy and health probe are on the same host | in deploys |
| `SITE_BIND_ADDR` | dev shell, `127.0.0.1:8080`. Wins over `PORT` when set, so binding all interfaces is always an explicit choice | local |
| `STATIC_DIR` | flake `env`. Unset locally, where Vite serves pages | in deploys |
| `S3_ENDPOINT`, `S3_BUCKET`, and the key as `AWS_ACCESS_KEY_ID` and `AWS_SECRET_ACCESS_KEY` | kennel `garage` provisioner in deploys; the dev shell names the key `S3_ACCESS_KEY` and `S3_SECRET_KEY`, which win when both are set. `S3_REGION` defaults to `garage` | optional. Without them media is stored in Postgres; a partial set logs which are missing, and startup logs where uploads go |
| `OIDC_CLIENT_ID`, `OIDC_CLIENT_SECRET`, `KEYCLOAK_URL`, `KEYCLOAK_REALM`, `OAUTH_RELAY_URL`, `PROJECT_GROUP`, `PROJECT_ADMIN_GROUP` | governance `oidc_client` | optional. Sign-in through Keycloak is on only when all seven are set, and on a production host only when `OAUTH_RELAY_URL` is the site's own callback (see "Ricochet relay") |
| `ANDREW_ID_CLAIM` | a Keycloak mapper, once there is one | optional. Names an ID token claim that holds the Andrew ID; when set it decides alone (see "Keycloak sign-in") |
| `PORTAL_DEV_LOGIN` | dev shell sets `1`; the preview profile defaults it to `1` | optional |
| `GOVERNANCE_SYNC` | `1` or `0`; declared in `secretspec.toml` so a deployment can turn it off | optional. Unset, the governance sync runs everywhere except local development with open dev sign-in |
| `PORTAL_DEV_LOGIN_KEY` | OpenBao, preview profile only | optional |
| `SEED_DEMO_CONTENT` | dev shell and preview profile | optional |
| `RUST_LOG` | anywhere | optional |
| `PUBLIC_POSTHOG_KEY`, `PUBLIC_POSTHOG_HOST` | governance `posthog` feature, prod profile; kennel serves them to the browser in `/_app/env.js` and passes them to the service | optional. Page views on public pages are sent only when both are set (see Changes, "security audit fixes: client-side security and privacy"); the service then lets public pages connect to the host |

Dev login rules, enforced in the service:

1. Off unless `PORTAL_DEV_LOGIN=1`.
1. Always off when the `APP_URL` host is `scottylabs.org` or `www.scottylabs.org`.
1. When `APP_URL` is not `localhost` or `127.0.0.1`, `PORTAL_DEV_LOGIN_KEY` must be set and the request must send the same key. Compare in constant time. There, `GET /api/auth/config` lists no personas; `POST /api/auth/dev-personas` with the key does.
1. Only the seeded personas below can be used. Away from `localhost` and `127.0.0.1`, never one whose row is linked to a Keycloak account (`keycloak_sub` set): someone signed in there with their real account, and the key is shared.
1. A dev session lasts 8 hours at most, and works only while dev sign-in is on and, on previews, with the key it was made with: turning dev sign-in off or changing the key ends it.

## Seed data

Seeding runs at startup inside one transaction and only inserts what is missing, so it is safe on every restart and never overwrites edits. The ticker is seeded once per database (recorded in `seed_markers`), so deleting its announcements sticks.

Always seeded:

- The seven committees with colors from the Team page and their tints.
- Each committee page, published, with the content from the Figma prototype.
- Published projects from the current home page and the Figma prototype, with their screenshots.
- The CollegeCart announcement from `Hero.svelte`, live for 30 days from first seed.

Seeded only when `SEED_DEMO_CONTENT=1` (dev shell, previews):

- The other sample announcements from the Figma ticker.
- Personas for dev login, with roles:

| Andrew ID | Name | Role |
| -- | -- | -- |
| `tkanz` | Thomas Kanz | admin |
| `evanfost` | Evan Foster | director, events |
| `apallati` | Anish Pallati | director, tech |
| `spchoksh` | Sara Chokshi | lead, events, can post to the ticker, edits Nova |
| `newmember` | New Member | no role |

Bundled seed images live in `sites/web/static/seed/` and are referenced as `/seed/...`. Each seed photo also has a `-480.jpg` copy, and a `-960.jpg` one where that is at most 80% of its width, written by `sites/web/scripts/seed-sizes.ts` (see Changes, 2026-11-25).

## Conventions

### Rust

- Edition 2024, nightly toolchain from kennel. Format with `cargo fmt`. `cargo clippy` clean, on the dev shell's nightly and on the stable toolchain the Nix checks (`site-clippy`, `site-test`) use in CI. Files the tests read with `include_str!` must be listed in `extraSources` in `flake.nix`.
- `sqlx` with runtime-checked queries (`query`, `query_as`, `query_scalar`). Do not use the `query!` macros, which need a database at compile time. `sqlx::migrate!` is fine.
- IDs are `Uuid::now_v7()`.
- Handlers return `Result<T, AppError>` (`src/error.rs`). Errors serialize as `{ "error": { "code", "message", "fields"? } }` with the matching status: `bad_request` 400, `validation_failed` 422, `unauthenticated` 401, `forbidden` 403, `not_found` 404, `conflict` 409, `method_not_allowed` 405, `payload_too_large` 413, `unsupported_media_type` 415, `rate_limited` 429, `upstream_failed` 502, `unavailable` 503 (with `Retry-After: 5`; the database is unreachable or too slow, the service is starting, or it is busy), `auth_not_configured` 503, `internal` 500. Internal errors log the cause and never return it.
- Messages are for people: plain sentences that say what to do. "Write a message of 90 characters or fewer." not "message: invalid length".
- JSON structs use `#[serde(rename_all = "camelCase")]`. Enums serialize as their snake_case names (`"in_development"`), matching `sites/web/src/lib/api/types.ts`.
- Every write that changes content or roles writes an audit entry with `audit::record` in the same transaction.
- Authorization lives in `src/permissions.rs` as pure functions over `Viewer`, unit tested. A handler must call them; hiding a button in the UI is never the check.
- No `unwrap` or `expect` outside startup and tests.
- No non-ASCII punctuation in source files (a commit hook rejects em dashes, smart quotes, ellipses and arrows). In Rust strings use escapes such as `\u{2019}`.

### Web

- Svelte 5 runes, SvelteKit 3, TypeScript strict. Check the installed packages in `node_modules` when unsure of an API; do not rely on memory of older versions.
- Use the tokens in `src/app.css` and the existing components. The portal UI kit lives in `src/lib/portal/ui/`. Satoshi is the site font; JetBrains Mono for inspector labels.
- Call the API through `src/lib/api/client.ts`. It adds the CSRF header and turns error bodies into `ApiError`.
- Every interactive element works with the keyboard and has a visible focus style. Honour `prefers-reduced-motion`. Text contrast meets WCAG AA.
- In Svelte markup use HTML entities (`&rsquo;`, `&mdash;`) rather than typographic characters.

## Auth

### Keycloak sign-in

- `GET /api/auth/login?returnTo=/portal/...`
  - `returnTo` must start with `/portal`; anything else becomes `/portal/`.
  - Without OIDC configured, redirect to `/portal/sign-in/?error=not_configured`.
  - Create a PKCE S256 verifier, a nonce and a 32-byte random `csrf`. Store them with `returnTo` in `auth_attempts`.
  - `state` is base64url (no padding) of `{"return_to": "<APP_URL>/api/auth/callback", "csrf": "<csrf>"}`. Ricochet reads only `return_to` (other fields are ignored) and forwards the browser there with every provider parameter unchanged: `code`, `state`, `iss`, `session_state`, `error`, `error_description`.
  - `redirect_uri` is `OAUTH_RELAY_URL`: the shared relay on previews and staging, the site's own callback in production (see "Ricochet relay"). Scopes: exactly `openid email profile`. Do not add `microprofile-jwt`: the realm offers it, and its mapper writes realm roles into a claim also named `groups`, replacing the group paths below.
  - Redirect (302) to Keycloak's authorization endpoint from discovery at `<KEYCLOAK_URL>/realms/<KEYCLOAK_REALM>`.
  - Each call to Keycloak gets 5 seconds: discovery (configuration and keys together), and each token request. A 5xx or 429 answer counts as no answer. One discovery runs at a time, without holding a lock while it runs, and a failed one is remembered for 30 seconds, during which the last good result is used, or the sign-in fails at once. Keycloak not answering ends the sign-in with `unavailable`.
- `GET /api/auth/callback?code&state` (or `?error=...`)
  - If `iss` is present it must equal `<KEYCLOAK_URL>/realms/<KEYCLOAK_REALM>` exactly (RFC 9207), on errors too; otherwise `failed`. A callback without `iss` is accepted.
  - On `error`, use up the attempt and go to `cancelled` for `access_denied`, `failed` for anything else.
  - Decode `state`, load and delete the attempt by `csrf`. Reject attempts older than 10 minutes.
  - Exchange the code with the verifier and `redirect_uri = OAUTH_RELAY_URL`. Keycloak's `invalid_grant` gives `expired`; other token errors give `failed`. Verify the ID token (signature, issuer, audience, expiry, nonce). Only RS256 is accepted, the algorithm of the realm's signing key, so a token signed with the client secret (HS256, which discovery also lists) is refused. Keys are picked by `kid`; the realm's encryption key (`use: enc`) is skipped.
  - Read `sub`, `name`, `email`, `email_verified`, the `groups` claim and the Andrew ID. The Andrew ID needs proof, because a username alone can come from an identity provider where people pick their own:
    - With `ANDREW_ID_CLAIM` set, that claim is the Andrew ID, and nothing else counts.
    - Otherwise `preferred_username` is the Andrew ID only when the token's `email`, ignoring case, is exactly `<preferred_username>@andrew.cmu.edu` or `<preferred_username>@cmu.edu`, and `email_verified` is not `false` (a missing claim is accepted).
    - Without the proof the sign-in is refused with `no_andrew_id`, and nothing is linked or created. One info line per sign-in logs whether `email_verified` was `true`, `false` or missing, without personal data.
  - Upsert the user: match `keycloak_sub`, else an existing row with that `andrew_id` (people can be added before they first sign in; linking such a row is audited), else insert. An Andrew ID whose row is linked to another `keycloak_sub` is refused with `linked_elsewhere`; an admin can reset the link (see "Sessions" and `sign-in-recovery.md`).
  - If `groups` contains `PROJECT_ADMIN_GROUP`, ensure an `admin` assignment with source `keycloak_group`.
  - End the session the browser already had, create a session that keeps Keycloak's refresh token and ID token on the server, and redirect to the attempt's `returnTo`. On any failure redirect to `/portal/sign-in/?error=<code>` with codes `cancelled`, `expired`, `failed`, `unavailable` (Keycloak did not answer), `no_andrew_id` and `linked_elsewhere`. `/api/auth/login` also sends `not_configured`, `unavailable` and, past its rate limit, `rate_limited`.
  - Every unsuccessful callback logs why, never with a code, token or secret: a provider `error` (warn, with `error` and `error_description`), a refused code exchange (warn, with Keycloak's `error` and `error_description`), a missing sign-in cookie or an unknown, used or stale attempt (info), and every other failure (warn). Text taken from the URL is cut to 200 characters.
  - The audit log records sign-ins (`auth.sign_in`, Keycloak and dev), sign-outs (`auth.sign_out`), first links of people added before they signed in (`auth.link`), refused identities (`auth.identity_refused`) and failed sign-ins with a reason code (`auth.sign_in_failed`), never a code or token. These entries have no actor and the person as subject, so only admins see them in "Recent activity". A failed callback is recorded only once it used up a stored attempt, which the rate limit on `/api/auth/login` bounds; one that matches no attempt is only logged.

### Ricochet relay

- Staging and previews share `https://oauth.scottylabs.org/oauth2/callback`. Its allowlist accepts `https://scottylabs.org` and every `*.scottylabs.net` deployment host (and every custom domain kennel publishes), and it forwards a code to whichever of them `state.return_to` names, which anyone can edit. It refuses `www.scottylabs.org` and plain http.
- So production must not use it: a production code relayed to a host someone else controls could be redeemed at the production callback by whoever started that sign-in. In production `OAUTH_RELAY_URL` is the site's own callback, `https://scottylabs.org/api/auth/callback`, also the prod client's only redirect URI in Keycloak, and Keycloak redirects straight back with the same `code`, `state`, `session_state` and `iss`. The callback works the same either way; `state.return_to` is not used.
- The service enforces this: when the `APP_URL` host is not `localhost`, a loopback address or a `*.scottylabs.net` host, and `OAUTH_RELAY_URL` is not exactly `<APP_URL>/api/auth/callback`, Keycloak sign-in stays off, startup logs one error naming the governance change, and `/api/auth/config` says `oidc: false`. Governance must change the prod client's redirect URI and the prod profile's `OAUTH_RELAY_URL` together; one without the other breaks sign-in.
- Development: kennel runs Ricochet as the pitchfork process `ricochet` on `127.0.0.1:8090` with `RICOCHET_DEV=1`, which accepts any http loopback `return_to`. The `scottylabs-org-dev` client allows only `http://localhost:8090/oauth2/callback`.
- Open the portal at `http://localhost:5173` in development, not `http://127.0.0.1:5173`. Sign-in returns to `APP_URL` (`http://localhost:5173`), and the sign-in cookie set on another host is not sent there, so the callback ends with `error=expired`.
- The pitchfork `site` process loads the dev profile's secrets with `secretspec export` before `cargo run`, as the dev shell does, because kennel starts processes without them. Without an OpenBao token it starts anyway, with dev sign-in only. Log in once with `nix run git+https://git.cmu.dev/ScottyLabs/kennel#login`.

### Where roles come from

Authorization reads only `role_assignments`. Three things write to it:

- **Admins from `PROJECT_ADMIN_GROUP`** (`/projects/scottylabs-org/admins`) in the token's `groups` claim, at every sign-in and every check of a session with Keycloak (see "Sessions"), with source `keycloak_group`. Governance fills that group with the UIA team leads, plus the Tech director because governance adds the Tech director to every team's leads. A `groups` list without the group, including an empty list, removes an admin role that came from the group (never the last admin). A token without the claim changes nothing. `PROJECT_GROUP` grants nothing.
- **Directors and heads from governance's `data/committees.yaml`**, with source `governance`: each committee's `director` becomes its director, and the members of `committees.<c>.roles.*` and `committees.<c>.teams.*.roles.*` become leads of that committee (can post to the ticker, which still needs approval; no projects). Tech is left out of the head sync, since its leads live in governance's team files. The organization director is never made an admin.
  - A head who already has a role in the committee is left alone. A governance lead whom governance stops listing is removed with their projects. Editing a lead on the People page makes it `portal`, so later syncs keep it; when governance stops listing such a lead, the sync says so once in `messages` and in the activity log ("Kept ... as a lead in ...").
  - The People page has the last word. Removing a director or lead role there, moving it to another committee, or changing a director to a lead writes a row to `governance_opt_outs` in the same transaction, and the sync then gives that person no role in that committee (director or lead) while governance still lists them there; each run says so in `messages`. The row is deleted once governance lists the person in that committee neither as director nor as a head. To give the role back, add it on the People page.
- **Everything else in the portal**: admins, directors and leads added on the People page, with source `portal`.

Committee roles are not in the Keycloak token: governance creates Keycloak groups only for tech teams and projects. The portal does not read Grouper, which is generated from the same `committees.yaml` and syncs only to Google Groups.

The governance sync runs from the admin Sync button, at startup (in the background, after seeding, with a one minute limit) and every six hours while the service runs. A failed run logs a warning and leaves roles as they were. `GOVERNANCE_SYNC` turns it on or off; see Environment. Runs by the service itself are audited with no actor.

### Sessions

- Cookie `__Host-sl_session` when `APP_URL` is https, otherwise `sl_session`. `HttpOnly`, `Path=/`, `SameSite=Lax`, `Secure` on https. Value is 32 random bytes, base64url. The database stores only its SHA-256.

- Expires after 12 hours without a request, and 7 days after creation at most (dev sessions: 8 hours). Refresh `last_seen_at` at most once a minute.

- **Checked with Keycloak.** A Keycloak session keeps Keycloak's refresh token and latest ID token in its row (never sent to a browser). At most every 15 minutes, a request on it starts a refresh-token grant in the background, 5 seconds at most, one at a time per session across processes (`validation_retry_at`). `invalid_grant` (account disabled, signed out of Keycloak, or Keycloak's own session over) or a token for another `sub` ends the session at once (`auth.session_end`, no actor). Otherwise the new ID token's `groups` add or remove the admin role from `PROJECT_ADMIN_GROUP`, as at sign-in. When Keycloak cannot be reached the session is kept and asked again 5 minutes later, but an admin role that came only from the group stops counting 24 hours after the last confirmation (sign-in or refresh) until one succeeds; `/api/me` still lists the role while `permissions.isAdmin` is false. Dev sessions are never checked. Sessions from before this, without a refresh token, are never checked, so their group admin lapses 24 hours after sign-in. In practice a Keycloak session also ends when Keycloak's own session ends (its idle or maximum time), at the first check after that.

- Residual risk: the refresh token is stored in plain text. Using it also needs the client secret, which is not in the database; whoever has both the database and the secret can act as a signed-in person with Keycloak until the token expires.

- Signing in (Keycloak or dev) ends the session the browser already had.

- `POST /api/auth/logout` deletes the session and clears the cookie. It answers 204, or 200 `LogoutResponse` `{ "endSessionUrl" }` when the session came from Keycloak and Keycloak's discovery lists an `end_session_endpoint` on its own origin: the URL carries `id_token_hint` and `client_id` and no return address (none is registered), so the browser can end the Keycloak session too.

- Admins can sign a person out everywhere (`DELETE /api/people/:id/sessions`, `auth.sessions_end`) and reset the link between their Andrew ID and a Keycloak account (`DELETE /api/people/:id/keycloak-link`, `auth.link_reset`, which also ends their sessions). Removing someone's last role on the People page ends their sessions too. Recovery steps are in `sign-in-recovery.md`.

- Expires after 12 hours without a request, and 7 days after creation at most. Refresh `last_seen_at` at most once a minute.

- `POST /api/auth/logout` deletes the session and clears the cookie (204). Signing out also clears work kept in the tab, and when the answer is `200` with `{ "endSessionUrl": "https://..." }` the browser goes there to end the Keycloak session too (see Changes, "security audit fixes: client-side security and privacy").

- When a portal call gets a 401, the web sends the person to sign in and back to the page, and editors keep work the service does not have yet in the tab until then (see Changes, "2026-11-25, follow-ups after the final review").

### CSRF and origin checks

Every `POST`, `PUT`, `PATCH` and `DELETE` under `/api/` must:

1. carry `X-CSRF-Token` equal to the session's CSRF token (from `GET /api/me`), except `/api/auth/dev-login` and `/api/auth/dev-personas`, which run before a session exists. The token is base64url of SHA-256 over a fixed prefix and the session cookie, so the database stores nothing that passes this check (the `csrf_token` column holds an unused random value, kept for rollbacks); and
1. if an `Origin` header is present, match the origin of `APP_URL`.

Authenticated endpoints send no CORS headers. Public `GET /api/public/*` endpoints send `Access-Control-Allow-Origin: *`.

### Viewer and permissions

`src/auth/extract.rs` provides two extractors:

- `Viewer`: the signed-in user with their role assignments and permissions. Rejects with 401 when there is no valid session.
- `PortalViewer`: a `Viewer` that has at least one role. Rejects with 403 otherwise. Use this on every portal endpoint except `/api/me`.

`src/permissions.rs` is the single place for these rules:

| Question | Admin | Director of C | Lead in C |
| -- | -- | -- | -- |
| Use the portal | yes | yes | yes |
| Post to the ticker as source S | any S, including `club` | S = C | S = C if `can_post_ticker`; needs approval |
| Edit, extend, end announcements from S | any | S = C | own pending or sent back ones only |
| Reorder live announcements | yes | yes | no |
| Edit and publish committee page C | yes | yes | no |
| Create, publish, archive projects owned by C | yes | yes | no |
| Edit a project's draft | yes | owned by C | if listed in `project_editors`; publishing needs approval |
| Set a project as featured, change its slug | yes | no | no |
| Approve or send back for C | yes | yes | no |
| See people | all | people with a role in C | no |
| Add, change, remove leads in C | yes | yes | no |
| Add, change, remove directors and admins | yes | no | no |
| Sync directors and heads from governance | yes | no | no |

Extra rules:

- Nobody can remove their own admin role, and the last admin cannot be removed.
- A director appointing a lead may only give them projects owned by the same committee.
- A person may hold roles in several committees, but only an admin can give a second committee.

## API

All bodies are JSON unless stated. Types are in `sites/web/src/lib/api/types.ts`.

### Public (no auth, `Cache-Control: public, max-age=60, stale-while-revalidate=600, stale-if-error=86400`)

| Method and path | Response |
| -- | -- |
| GET `/api/health` | `200 ok` (plain text) once migrations and seeding are done and the database answers within 1.5 s; otherwise `503` with `starting` or `unavailable` and `Retry-After` |
| GET `/api/public/ticker` | `PublicTickerResponse`: live, unexpired, ordered by `position` then `starts_at` |
| GET `/api/public/projects?committee=&q=` | `PublicProjectsResponse`: published, featured first, then by name. `committee` keeps one committee's projects. `q` keeps those where every word appears, ignoring case and accents, in the name, summary, whole description, committee name, kind or status (the words on the card); it is trimmed, cut to 100 characters, and control characters are dropped. Blank means no search. Searches count 60 a minute per visitor (`429` with `Retry-After`), and at most 4 run at once (`503` after waiting 2 s) |
| GET `/api/public/projects/:slug` | `PublicProjectResponse`; 404 when unknown or not published |
| GET `/api/public/committees` | `PublicCommitteesResponse`, in `sort_order` |
| GET `/api/public/committees/:slug` | `PublicCommitteePageResponse`; 404 for unknown slugs |

### Auth and session

| Method and path | Response |
| -- | -- |
| GET `/api/auth/config` | `AuthConfigResponse` |
| GET `/api/auth/login?returnTo=` | 302 |
| GET `/api/auth/callback` | 302 |
| POST `/api/auth/dev-login` | `DevLoginInput` to 204 with cookie |
| POST `/api/auth/dev-personas` | `DevPersonasInput` to `DevPersonasResponse`; 403 when dev sign-in is off or the key is wrong |
| POST `/api/auth/logout` | 204, or 200 `LogoutResponse` (see "Sessions") |
| GET `/api/me` | `MeResponse`, or 401 |

### Ticker

| Method and path | Who | Notes |
| -- | -- | -- |
| GET `/api/announcements?tab=live\|waiting\|expired&before=` | portal | `waiting` holds pending and sent back. Leads see their own plus live ones. `expired` keeps 90 days and comes 50 at a time, newest first by when each left the ticker (`coalesce(ended_at, expires_at)`, then id): `nextBefore` is the `before` for the next page, null on the last page and on the other tabs, which ignore `before`. `counts.expired` is the total. A `before` that is not one answers `422` at `before` |
| GET `/api/announcements/:id` | portal | `AnnouncementResponse`. A waiting one only for its author and those who manage its source; 404 otherwise |
| POST `/api/announcements` | may post | Live at once for admins and directors. For leads, creates a `pending_approval` announcement and an approval |
| PATCH `/api/announcements/:id` | may edit | Partial `AnnouncementInput`. A lead editing a sent back one resubmits it |
| POST `/api/announcements/:id/extend` | may manage | Body `{ "days": 7 }`. Never beyond 30 days from now |
| POST `/api/announcements/:id/end` | may manage | Sets `state = ended`, `ended_at = now` |
| POST `/api/announcements/:id/repost` | may post from its source | Body `{ "expiresAt" }`. New announcement copied from an expired or ended one |
| PUT `/api/announcements/order` | reorder | Body `{ "ids": [...] }`, the live ones in their new order |
| DELETE `/api/announcements/:id` | creator of a pending or sent back one, or manager | 204 |

Rules: message 1 to 90 characters on one line, trimmed, without control characters, line separators or bidirectional overrides (true of every one-line field). `linkUrl` optional, `https` only, at most 2048 characters. `expiresAt` in the future and at most 30 days after it goes live. At most 6 live at once; posting or approving a seventh fails with `conflict` and says to end one first. Approving keeps the lead's chosen `expiresAt`; if it is less than 5 minutes away, approval fails with `validation_failed` and asks the approver to send it back for a later time.

### Committee pages

| Method and path | Who |
| -- | -- |
| GET `/api/committee-pages` | portal |
| GET `/api/committee-pages/:slug` | portal (read only unless `canEdit`) |
| POST `/api/committee-pages/:slug/lock` | editors: take or refresh the 5 minute edit lock. 409 with the holder when someone else holds it |
| DELETE `/api/committee-pages/:slug/lock` | the holder |
| PUT `/api/committee-pages/:slug/draft` | holder of the lock. 409 when `baseVersion` is stale |
| GET `/api/committee-pages/:slug/changes` | editors. Plain sentences such as "Stats moved above What we do" |
| POST `/api/committee-pages/:slug/publish` | editors. Body `{ "draftVersion" }`. Validates for publishing (alt text) |
| POST `/api/committee-pages/:slug/discard` | editors. Draft becomes the published version |

Draft validation (saving): block types and widths per `BLOCK_WIDTHS`, the cover first and only once, at most 12 blocks, unique block ids, stats 2 to 4 items, gallery 3 to 6 images, text fields within limits, URLs `https` (or `mailto:` for buttons), images only `/media/<uuid>` or `/seed/...`, video URLs that play one YouTube or Vimeo video, rich text sanitized with `ammonia`. Publishing additionally requires alt text on every image and an uploaded cover at least 1200 pixels wide.

### Projects

| Method and path | Who |
| -- | -- |
| GET `/api/projects?committee=&state=&q=` | portal; leads see their projects, directors and admins see all with `canEdit` set per row |
| POST `/api/projects` | body `{ "name", "committee" }`; creates a draft, slug from the name, unique with `-2`, `-3` |
| GET `/api/projects/:id` | `ProjectEditorResponse` |
| PUT `/api/projects/:id/draft` | body `{ "doc": ProjectDoc }` |
| POST `/api/projects/:id/publish` | publishes, or for leads creates an approval (response still `ProjectEditorResponse`) |
| POST `/api/projects/:id/archive`, `/unarchive` | admin, director |
| POST `/api/projects/:id/discard` | draft becomes the published version |
| PATCH `/api/projects/:id/settings` | admin: `{ "slug"?, "featured"? }`. A slug change records a redirect |
| PUT `/api/projects/:id/editors` | `{ "userIds": [...] }`, directors of the owner and admins |

Rules: name 1 to 60 characters; summary 1 to 48; description up to 4000 characters of text after sanitizing; links `https`; `repoUrl` any https host; `videoUrl` a link to one YouTube or Vimeo video (`blocks::video_id`, as committee video blocks); at most 8 media; `builtWith` excludes the owner. Publishing needs a cover with alt text and a summary; an uploaded cover must be at least 1200 pixels wide (`/seed/` images are exempt).

### Media

`POST /api/media`, multipart with `file` and optional `alt`, any portal user.

- At most 5 MB. JPEG, PNG or WebP, detected from the bytes, not the name or header.
- Decode with limits: at most 12000 pixels on each side, 40 megapixels in all and 160 MB of decoded pixels (both checked from the header before decoding, so 16-bit images are refused above 20 megapixels with `413`, "That image holds too much data to process..."), and 256 MB of allocation. Two uploads decode at once and four more may wait; past that an upload answers `503 unavailable` at once. The EXIF orientation is applied to the widest variant, after the first downscale.
- Re-encode, which strips metadata: JPEG at quality 82 for opaque images, PNG when the image has transparency. Make widths 480, 960 and 1920, never wider than the original.
- Store under `media/<uuid>/<width>` in S3 when configured, otherwise in `media_blobs`.
- Respond `201 UploadResponse`.

`GET /media/:id?w=`: the smallest stored variant at least `w` wide, or the largest. `Cache-Control: public, max-age=31536000, immutable`, `X-Content-Type-Options: nosniff`. Media rows are kept in memory once looked up (the 4,096 most recently used), so images in S3 keep being served while the database is briefly unreachable. Logs and errors name the bucket, key and status of a failed S3 call, never the presigned URL.

### Approvals

| Method and path | Who |
| -- | -- |
| GET `/api/approvals?status=waiting\|decided&mine=` | approvers see their committees; leads see their own (`mine=true`) |
| POST `/api/approvals/:id/approve` | approvers |
| POST `/api/approvals/:id/send-back` | approvers. Body `{ "note" }`, 1 to 500 characters |

Approving calls `announcements::apply_approval` or `projects::apply_approval` in the same transaction. Withdrawn approvals happen when the lead edits or deletes the subject.

### People

| Method and path | Who |
| -- | -- |
| GET `/api/people?committee=` | admins, directors (their committees) |
| GET `/api/people/search?q=` | admins, directors. Matches name or Andrew ID among known users, 10 results |
| POST `/api/people` | `AddPersonInput` |
| PATCH `/api/people/assignments/:id` | `UpdateAssignmentInput` |
| DELETE `/api/people/assignments/:id` | 204 |
| DELETE `/api/people/:id/sessions` | admins. Ends every session of the person; `EndSessionsResponse` `{ "ended" }`. Clears the cookie when admins end their own |
| DELETE `/api/people/:id/keycloak-link` | admins. Clears `keycloak_sub` and ends the person's sessions; 204. Nothing changes for someone not linked |
| POST `/api/people/sync-governance` | admins. Reads `data/committees.yaml` from the governance repository, makes each listed director a director of their committee and each listed head a lead in it (see "Where roles come from"). `GovernanceSyncResponse` |

### Overview and activity

| Method and path | Who |
| -- | -- |
| GET `/api/overview` | portal, scoped to the viewer |
| GET `/api/activity?limit=` | portal, scoped: admins see all; others see their own actions, every entry for committees they direct, and entries for committees they only lead in that name nothing they cannot open (see Changes, 2026-11-25) |

Attention items, in this order: approvals waiting for the viewer; the viewer's own sent back items; announcements the viewer can manage that expire within 3 days; committee pages they can edit with unpublished changes; projects they can edit missing a cover, summary or links; pages they can edit not published for 90 days.

## Static serving

For a request outside `/api` and `/media`, with `STATIC_DIR` set:

1. Reject paths with `..` or NUL. Decode percent-encoding once.
1. Serve the file if it exists. A directory with `index.html` and no trailing slash redirects (308) to the slash form.
1. Otherwise serve `200.html`:
   - `/projects/<slug>/`: inject the project's name, summary and cover as `<title>`, the meta description and Open Graph tags, each marked `data-sl-injected`. The app removes the marked tags before any page writes its head (`src/lib/site/head.ts`, called from the root layout), so crawlers that run no scripts read them and visitors never get two descriptions. Status 404 when no published project has that slug.
   - `/portal/` and below: status 200, `Cache-Control: no-store`.
   - Anything else: status 404 (SvelteKit renders its error page).

Headers on every response: `X-Content-Type-Options: nosniff`, `Referrer-Policy: strict-origin-when-cross-origin`, `X-Frame-Options: DENY`, `Permissions-Policy: camera=(), microphone=(), geolocation=()`, a `Content-Security-Policy` (HTML pages get theirs with the hashes of their own inline scripts; see Changes, "security audit fixes: client-side security and privacy"), and HSTS when `APP_URL` is https. `/_app/immutable/*`: `public, max-age=31536000, immutable`. HTML: `no-cache`. Other static files: `public, max-age=3600`.

## Limits on requests

- JSON bodies at most 256 KB, uploads 6 MB. A NUL anywhere in a JSON string is refused with `validation_failed`, since Postgres cannot store it.

- Rich text sources over 64,000 bytes or 4,000 tags are refused before sanitizing.

- Per IP: sign-in and dev login 20 a minute; writes 120 a minute; uploads 30 a minute; public project searches 60 a minute. Behind kennel's proxy (a loopback peer) the client is `CF-Connecting-IP`, then the right-most `X-Forwarded-For`; headers from other peers are ignored.

- Time: a request has 30 seconds to produce its response (uploads 120), headers must arrive within 30 seconds (also the idle limit of a kept-alive connection), and a body may pause at most 30 seconds. Request connections stop a statement after 5 seconds and a lock wait after 2, and end a transaction idle for 30. Shutdown waits at most 8 seconds for open requests.

- Per IP: starting a sign-in 60 a minute (a campus network often shares one address), dev sign-in and its persona list 20 a minute, writes 120 a minute, uploads 30 a minute. A refused `GET /api/auth/login` redirects to `/portal/sign-in/?error=rate_limited` instead of answering JSON. When the service listens on a loopback address only, a loopback peer is kennel's proxy and the client is `CF-Connecting-IP`, then the right-most `X-Forwarded-For`; listening on every interface, and from any other peer, those headers are ignored.

## Ownership of files

Each area keeps the files below; change another area's files only together with its owner, and record shared changes here. Rust paths are under `services/site/src/`, web paths under `sites/web/`.

| Area | Files |
| -- | -- |
| Scaffold | `flake.nix`, `Cargo.toml`, `services/site/Cargo.toml`, `secretspec.toml`, `{main,state,error,db,config,models,approvals_core}.rs`, `routes/mod.rs`, migration `0001_init.sql`, `src/lib/api/types.ts`, this file |
| Platform | `auth/`, `permissions.rs`, `audit.rs`, `ratelimit.rs`, `json.rs`, `testing.rs`, `static_files.rs` and `static_files/`, `media/`, `routes/{auth,me,people,media,activity}.rs` and `routes/{auth,people}/`, `seed/{mod,people}.rs`; web: `src/lib/api/client.ts`, `src/lib/portal/` (except `approvals/` and `overview/`), `src/lib/site/`, `src/routes/+layout.*`, `src/routes/+error.svelte`, `src/routes/(site)/+layout.svelte`, `src/routes/portal/{+layout.*,+error.svelte,sign-in,no-access}`, `src/lib/sections/home/Header.svelte`, `vite.config.ts` |
| Shared components | `src/lib/editor/`, `src/lib/media/`, `src/lib/portal/ui/PersonPicker.svelte`, `src/lib/site/SelectionBox.svelte`; `src/lib/projects/ProjectCard.svelte` belongs to Projects |
| Ticker | `routes/announcements.rs` and `routes/announcements/`, `seed/ticker.rs`; web: `src/lib/ticker/`, `src/routes/portal/ticker/`, the strip in `src/lib/sections/home/Hero.svelte` |
| Projects | `routes/projects/`, `seed/projects.rs`, migrations `02*`; web: `src/lib/projects/`, `src/routes/(site)/projects/`, `src/routes/portal/projects/`, the project links in `src/lib/sections/home/Projects.svelte` |
| Committees | `routes/committee_pages/`, `blocks.rs`, `richtext.rs`, `seed/committees.rs`, migrations `03*`; web: `src/lib/committees/`, `src/routes/(site)/committees/`, `src/routes/portal/committees/` |
| Overview | `routes/overview.rs` and `routes/overview/`, `routes/approvals.rs` and `routes/approvals/`; web: `src/routes/portal/+page.svelte`, `src/lib/portal/overview/`, `src/lib/portal/approvals/`, `src/routes/portal/people/` |
| Sign-in and governance | `auth/`, `routes/auth.rs`, `routes/auth/`, `routes/people/governance*`, `seed/people.rs`, `migrations/05*` and `07*`, `.gitleaks.toml`, `docs/implementation/sign-in-recovery.md` |

## Changes

Record changes to this contract here, newest first, with the reason.

### 2026-11-25, security audit fixes: availability and resilience

From the security audit (AVAIL-1, AVAIL-2, AVAIL-4, AVAIL-5, AVAIL-6, AVAIL-8, AUTHN-6, UPLOAD-1, LOG-1, CRIT-6). The service used to need Postgres to start: a restart during a database outage crash-looped, and kennel holds a unit that fails five times in five minutes stopped until the next deploy, static pages included.

- **Starting without the database.** `main` binds and serves at once. The request pool (`db::request_pool`) connects lazily; migrations and seeding run in the background on a one-connection pool of their own without the request timeouts (`db::prepare_until_ready`), retrying after 1 s, then twice as long each time up to a minute, and never giving up. `AppState.database` (`db::Readiness`) records `starting`, `unavailable` (could not connect), `failed` (a migration or the seed failed) or `ready`. Until it is ready, every `/api` call but the health check answers `503 unavailable` with `Retry-After: 5`; the static site and `/media` are served as usual. The governance sync starts once the database is ready. `AppState::new` (tests) is ready at once; `AppState::starting` is what `main` uses.
- **Health is readiness.** `GET /api/health` answers `200 ok` only when the database is ready and `SELECT 1` answers within 1.5 s (kennel's probe waits 2 s), otherwise `503` with the plain text `starting` or `unavailable` and `Retry-After`. It names no cause; the log does. Kennel's deploy check (200 within 60 s) still stops a release whose migrations fail; deploys need Postgres for provisioning anyway, so a database outage never meets that check.
- **Outages answer 503, not 500.** `AppError::Unavailable` (`unavailable`, 503, `Retry-After: 5`, in `ApiErrorBody`). Database errors that mean the database is down, unreachable, overloaded or too slow (pool timeout or closed, I/O, TLS, SQLSTATE classes 08, 53, 57 and 58, and 55P03) become it, wherever they are in an error's chain (`db::is_unavailable`); other database errors stay `internal`.
- **Timeouts.** Request connections set `statement_timeout` 5s, `lock_timeout` 2s and `idle_in_transaction_session_timeout` 30s as connection options, and wait at most 3 s for a connection (was 10 s). `DATABASE_POOL_SIZE` sets the pool, 1 to 50, default 10. `server::serve` replaces `axum::serve`: HTTP/1.1 only (kennel's Caddy, its probe and Vite's proxy speak it, and HTTP/2 detection would wait on a silent connection without a timeout), hyper's header timer of 30 s, which is also the idle limit of kept-alive connections, a request body that pauses 30 s fails, `ConnectInfo` as before, and a graceful shutdown that waits at most 8 s, under kennel's 10 s stop wait. `server::limit_duration` ends a request that has no response after 30 s (uploads 120 s) with `503 unavailable`.
- **Loopback by default.** With `PORT` the service binds `127.0.0.1:$PORT` (was `0.0.0.0`); `SITE_BIND_ADDR` wins when set. Kennel's Caddy dials `localhost:<port>` and its probe `127.0.0.1:<port>`, so both still reach it.
- **Last good public answers.** `last_good::keep` keeps the latest `200` of every `GET /api/public/*` in memory, keyed by path and, for the project list, `committee` (searches are not kept; at most 256 copies, 1 MB each, 16 MB in all). When a later request answers a 5xx, the kept copy is sent with `200`, `Cache-Control: no-store`, `Access-Control-Allow-Origin: *` and `X-Content-Stale: true`. The public `Cache-Control` adds `stale-if-error=86400` (`routes::common::PUBLIC_CACHE`), so a CDN that caches these answers may keep serving them for a day while the service fails.
- **Public pages through an outage (web).** The committee pages, the Committees index, the Projects page and project pages keep the last content each visitor loaded in `localStorage` under `sl-last-good:<key>` (`committee:<slug>`, `committees`, `projects`, `project:<slug>`) as `{ value, savedAt }`, at most 512 KB each, checked for shape and dropped after a week or when the page answers 404 (`src/lib/site/last-good.ts`). When the content cannot load, a page shows what it showed before in this visit, else the saved copy, with `LastGoodNote` above it ("We couldn't reach the site, so this is how the page looked when you visited earlier today." and Try again), and its error message only when it has neither. The ticker already kept what it loaded and hides when it has nothing; it is unchanged, since its public shape has no expiry to check a saved copy against.
- **Search cost.** `search::read` keeps each published project's excerpt and folded search text in memory, keyed by id and a per-process keyed hash of the fields they come from, so neither the list, committee pages nor a search parses a description already read; cards are built on the blocking pool. Searches take a visitor's `ratelimit::Class::Search` allowance (60 a minute) in the list handler, and wait at most 2 s for one of 4 search slots (`AppState.search_slots`).
- **Uploads.** `process::MAX_DECODED_BYTES` (160 MB, 40 megapixels at 4 bytes) is checked from the header with the pixel count, and the decoder may allocate 256 MB (was 512 MB): a 16-bit image above 20 megapixels answers `413` with "That image holds too much data to process. Export it as a JPEG and try again." The EXIF orientation is applied after shrinking to the widest variant, and the widths follow the shown orientation. Two uploads decode at once and four more may wait; any further upload answers `503 unavailable` with "Many images are being processed right now. Try again in a few seconds." at once.
- **Media.** `media::lookups` keeps the 4,096 most recently used media rows (content type, variants, storage), which never change after upload, so images in S3 keep being served while the database is briefly unreachable. A failed S3 call logs and returns `<action> <key> in bucket <bucket>: could not connect | timed out | failed` with reqwest's error without its URL, since the presigned URL's query string is a credential; download stream errors drop the URL too. `/media` already sends `public, max-age=31536000, immutable`.
- **Production answers only on its own host** (`host_redirect::canonical_host`). When the `APP_URL` host is `scottylabs.org` or `www.scottylabs.org`, a request naming another host, such as kennel's generated `scottylabs-org-site-main.scottylabs.net`, is answered `308` to the same path and query on `APP_URL` (`Cache-Control: public, max-age=86400`) for `GET` and `HEAD`, and `403 forbidden` ("Changes can only be made at https://scottylabs.org...") otherwise. `/api/health` is exempt for kennel's probe on `127.0.0.1`; requests without a host pass; previews and local development are unaffected.
- **Owned elsewhere, recorded for ops and kennel.** Cloudflare caches by file extension, so `/api/public/*` (without `q`) and the extension-less `/media/<uuid>` need a Cache Rule that makes them eligible and respects origin TTLs; kennel forces `no-store` on HTML; kennel leaves a unit that hit its start limit stopped until a redeploy, restarts units without secrets when OpenBao is down, sets no `MemoryMax`, and routes the generated host of a production deployment instead of redirecting it.

### 2026-11-25, security audit fixes: sign-in, sessions and identity

From the security audit (findings AUTHN-1 to 5, AUTHZ-1, 4 and 7, GOV-1, CRIT-2 and 3, RELAY-1, AVAIL-1, 2, 9 and 10, NET-1, PREVIEW-1, PREV-1, DP-4, AUD-2). Migration `0701_session_validation.sql` adds `sessions.refresh_token`, `id_token`, `validated_at`, `validation_retry_at` and `dev_key_fingerprint`.

- **Production does not use the shared relay** (see "Ricochet relay"). A production host whose `OAUTH_RELAY_URL` is not its own callback keeps Keycloak sign-in off with one startup error. Governance must give the prod client `https://scottylabs.org/api/auth/callback` as its redirect URI and the same `OAUTH_RELAY_URL`.
- **Andrew IDs need proof** (see "Keycloak sign-in"): `ANDREW_ID_CLAIM`, or a `preferred_username` whose CMU email matches and `email_verified` is not false. Otherwise `error=no_andrew_id`, and nothing is linked or created. A refused relink is `error=linked_elsewhere`.
- **Sessions are checked with Keycloak** every 15 minutes at most, so a disabled account or leaving the admin group takes effect without a new sign-in; a Keycloak outage keeps sessions but lets the group's admin role lapse after 24 hours (see "Sessions"). Admins can sign people out everywhere and reset a Keycloak link; removing someone's last role signs them out. Recovery steps: `sign-in-recovery.md`.
- **Fail fast while Keycloak is down**: 5 seconds per call, one discovery at a time with no lock held, failures remembered for 30 seconds, and `error=unavailable` on the sign-in page.
- **Signing in ends the browser's earlier session**, for Keycloak as for dev sign-in.
- **Previews**: dev sessions last 8 hours, end when dev sign-in turns off or the key changes, and never act as a person linked to Keycloak away from localhost. `GET /api/auth/config` lists no personas there; `POST /api/auth/dev-personas` with the key does, and the sign-in page asks for the key first.
- **CSRF tokens are derived from the session cookie**, so the database holds none that work. `/api/me` and the header are unchanged.
- **Rate limits**: sign-in starts 60 a minute per IP, dev sign-in 20; a refused sign-in start redirects to `error=rate_limited`. `CF-Connecting-IP` counts only when the service listens on a loopback address.
- **Audit**: sign-ins, sign-outs, failed and refused sign-ins (with a reason, after an attempt was used up), ended sessions and link resets (see "Keycloak sign-in" and "Sessions"). `audit::record` is unchanged.
- **Sign-out** answers 200 with `endSessionUrl` when Keycloak can end its own session too; otherwise 204 as before. The web does not use it yet (spec Q5).
- **Types**: `DevPersona`, `DevPersonasInput`, `DevPersonasResponse`, `LogoutResponse` and `EndSessionsResponse` in types.ts; `AuthConfigResponse.devPersonas` is empty when `devLoginNeedsKey`.

### 2026-11-25, security audit fixes: authorization, governance and data protection

Fixes for the audit's authorization, governance and data protection findings. Migration `0801_audit_retention_and_review.sql` (additive). Where an earlier statement in this file says otherwise, this entry wins.

- **The audit log only grows** (AUD-1). A trigger on `audit_log` refuses `DELETE` and `TRUNCATE` and skips `UPDATE` (with a warning), except that a user's deletion may still clear `actor_id`. The only way out is the database function `expire_audit_log()`, which deletes entries more than two years old. `audit_log.details` (jsonb) keeps before and after values, written by the new `audit::record_details(conn, entry, details)` (`audit::record` is unchanged; `audit::change(before, after)` builds the usual `{ "before", "after" }`). Details are kept for role changes (`role.add`, `role.update`, `role.remove`, `role.sync`: Andrew ID, role, committee, ticker permission, projects), project settings (`project.slug`, `project.feature`, `project.move`, `project.archive`, `project.unarchive`, `project.editors`), every publish and approval (`project.publish`, `project.approve`, `committee_page.publish`: the published version before and after; `announcement.approve`: what went live), discards (the draft thrown away), deletions (`announcement.delete`, `media.delete`, `media.expire`), live ticker edits, uploads (`media.upload`) and edit lock takeovers (`committee_page.take_over`). Recent activity shows only the sentences. The log is not the place for secrets or session data.
- **Draft edits are logged at most once per 15 minutes** per person and project ("Edited Nova", with the time of the first edit in that window), instead of one entry whose time moves forward: rows are never rewritten. What each publish changed is in its own entry. The previous release's in-place refresh now changes nothing and falls back to adding an entry, so a rollback still works.
- **Recent activity follows what each entry reveals** (AUTHZ-2). In committees the viewer only leads in, an announcement entry shows when the viewer wrote the announcement, or when it is open to everyone and the entry was not written while it waited (`announcement.submit`, `announcement.resubmit`, `announcement.send_back`): a version sent back and rewritten stays with its author and the source's managers after a later version goes live. Entries about a person show to leads only when they are about the viewer. `media.upload` and `media.expire` are left out of Recent activity for everyone (admins review uploads with `GET /api/media`).
- **People and search** (AUTHZ-5, DP-3). `GET /api/people/search` finds nobody for fewer than 2 characters (trimmed). Directors get `id`, `andrewId`, `name` and `initials` only: `hasSignedIn` is for admins and is optional in `PeopleSearchResponse`. The search still covers everyone the portal knows (spec PE-3). For directors, `GET /api/people` and the answers of `POST` and `PATCH` give `email` and `lastSeenAt` as null, list only roles in the committees they direct, and `hasSignedIn` says whether the person signed in since getting their role there. A director may add a lead only for a person with no role at all; anyone who is an admin or has a role elsewhere needs an admin (`403`, "<name> already has a role in the portal. Only an admin can give them another one.", or the existing second-committee message) (`permissions::can_appoint_person_with`).
- **What admins see to review roles** (GOV-3). Each role in `GET /api/people` also has `changedAt` and `changedBy` (from `role_assignments.updated_at` and `updated_by`, written by every People page change; null `changedBy` for the sync and sign-in) and `governanceListed`: whether governance listed the person in that role at the last successful sync (director against `governance_directors`, lead against `governance_heads`), null for admins, Tech leads and before the first sync (`PeopleRoleAssignment` in types.ts). For admins, `PeopleResponse.governance` (`GovernanceSyncStatus`) has `lastAttemptAt`, `lastSuccessAt`, `lastError` and the last successful run's `messages`. The People page shows a note under roles governance does not list, a warning when the last sync failed or is more than a day old, and the last run's messages. Roles are not expired automatically.
- **Governance sync** (GOV-2, AUTHZ-3, reviewer note on logs).
  - A governance director whom governance no longer lists is removed even when governance names no successor (no `director:`, or one that is not a valid Andrew ID). More than 2 committees losing their governance director that way in one run looks like a broken file: nobody is removed and the run says so. When a committee is left without a director, the run says so.
  - Opt-outs keep the role that was taken away (`governance_opt_outs.role`): removing or moving a director's role, or turning it into a lead, which only admins can do, keeps the sync from giving that person any role in the committee; removing or moving a lead's role, which directors can do, only keeps the sync from making them a lead again, so it never blocks governance from appointing them director. Opt-outs from before this change keep blocking both.
  - Directors governance does not list (roles the People page gave) are kept and named in `messages` on every run ("<name> is still director of <committee>, but governance lists <id>..."), with a `role.sync` entry when governance's director for that committee changes. A committee with roles from governance that is missing from the file keeps them and the run says so.
  - The run's messages, and why a run failed, are stored in `governance_sync_status` for the People page. The service log gets counts only, never names. `governance_directors (committee, user_id)` remembers the listed directors.
- **Edit lock takeover** (AUTHZ-8). `POST /api/committee-pages/:slug/lock/take-over` (admins only, `permissions::can_take_over_page`) takes the lock from whoever holds it and answers with the admin's `EditLock`; the holder's next heartbeat or save gets `409` naming the admin. Audited as `committee_page.take_over`. The page editor offers admins "Take over editing" with a confirmation.
- **Roles checked inside the write** (AUTHZ-8). `permissions::recheck(conn, viewer)` reads the viewer's roles and lead projects again inside the transaction and share-locks the role rows until it ends, so a request already running when an admin removes a role cannot finish with it, and the removal waits for it. People writes, project writes and approval hooks, committee page writes and media deletion use it.
- **One lock order for projects** (AUTHZ-6). Project writes from the editor (save, publish or send, archive, unarchive, discard, settings, editors) lock the project's waiting approval before the project, the order `routes/approvals.rs` uses, so approving while the lead autosaves no longer deadlocks.
- **Uploads** (AVAIL-7, UPLOAD-2). Each person may upload at most 100 images and 500 MB of stored variants in any 24 hours, counted under a per-person lock; over it, `POST /api/media` answers `429 rate_limited` ("You have uploaded 100 images in the last 24 hours, the most allowed. Try again tomorrow, or ask an admin if you need more.") with `Retry-After`. Every upload is audited. Admins have `GET /api/media?limit=&uploadedBy=` (`MediaListResponse`: newest first, at most 200, with `uploadedBy` and `inUse`) and `DELETE /api/media/:id` (204), which deletes the S3 objects or `media_blobs` rows, then the row, audited as `media.delete`. Pages that still show a deleted image lose it; drafts ask for another at their next save. `/media` keeps its immutable caching (ids are random), so browsers that already have a copy keep it. There is no portal screen for the list yet.
- **Retention** (DP-2, spec Q8), in `src/retention.rs`, hourly from five minutes after startup, under an advisory lock that another replica skips; each step deletes only what matches its rule then, so it is safe to repeat, and it logs counts only:
  - sessions past their idle or absolute limit, and sign-in attempts more than an hour old;
  - announcements that left the ticker more than 90 days ago (`coalesce(ended_at, expires_at)`), submissions never approved included, with their approvals (the expired tab already stopped at 90 days, spec TK-9);
  - uploads that no project or committee page draft or published version and no waiting or sent back submission has used for 7 days (`media.unreferenced_since`), files first, each under its row lock and checked again, audited as `media.expire` without an actor; committee page saves share-lock the images they use, so a page taking an image back cannot lose it;
  - audit entries more than 2 years old, through `expire_audit_log()`. Summaries name people, so a person's name stays in the log for up to two years after they leave; deleting a user keeps their entries with no actor.
- **Text rules** (TEXT-1). Image alt text in project and committee docs follows the one-line rules (`ImageRef::check` runs `check_line`). One-line fields also refuse the zero width space, word joiner, byte order mark and Mongolian vowel separator. Saving a project description or committee text block refuses the bidirectional embeddings, overrides and isolates (U+202A to U+202E, U+2066 to U+2069), also written as character references (`422` at `description` or `blocks.<i>.body`).
- **Rich text is cleaned on the way out** (WEB-XSS-1, server half). `GET /api/projects/:id` and every project write answer sanitize the draft's and published version's description, drop links the editor would refuse and images from outside the site, as approvals do; committee page editor answers and `GET /api/public/committees/:slug` sanitize text bodies. Both drop bidirectional controls.
- **Decisions kept**, recorded here because the audit asked:
  - Directors open every project, drafts included (`permissions::can_view_project`), as "GET /api/projects" above grants; edits stay limited to their committee's.
  - Any director may reorder the whole live ticker, as the matrix above grants ("Reorder live announcements"); it is one shared loop (spec TK-3) and every reorder is audited.
  - One director may fill all six ticker slots (CRIT-5): the slots are shared first come, first served (spec TK-7, F6), and admins end announcements when they must. No per-committee quota.

### 2026-11-25, security audit fixes: client-side security and privacy

Fixes for the audit findings on the web app and the page headers (DP-1, WEB-PRIV-1, PRIV-1, CSP-1, WEB-HARD-1, WEB-XSS-1, XSS-1, WEB-PRIV-2, the web part of DP-4, WEB-HARD-2, DEP-1).

- **Analytics (PostHog) count page views on public pages and nothing else.** Rules in `src/lib/site/analytics.ts`, applied by the root layout:
  - PostHog loads only when the page load starts on a public page, both `PUBLIC_POSTHOG_KEY` and `PUBLIC_POSTHOG_HOST` are set, and the browser sends neither Do Not Track nor Global Privacy Control. A page load that starts under `/portal` never loads it, even after the person moves on to public pages.
  - With PostHog in the page, every navigation into `/portal` (link, `goto`, form) is a full page load, so the portal never runs with it.
  - Options: page views on history changes and page leaves only. Off: autocapture (so no element text, attributes or form fields), rage clicks, dead clicks, heatmaps, exceptions, web vitals, session recording, surveys, product tours, conversations and web experiments. `advanced_disable_flags` stops the remote config and feature flag requests, so the analytics host cannot turn any of these on, and `disable_external_dependency_loading` stops every script from it (config.js, surveys.js, recorder.js, toolbar.js). `mask_all_text` and `mask_all_element_attributes` as a backstop, `respect_dnt`, `persistence: "localStorage"` (no analytics cookie on requests to the service) and `cross_subdomain_cookie: false`.
  - `before_send` drops any event whose `$current_url`, or the browser's current path, is under `/portal`, and cuts portal addresses in the other properties (`$referrer`, `$initial_referrer`, `$prev_pageview_pathname`, `$set_once` and so on) to `<origin>/portal/` or `/portal/`.
  - Ops, outside this repo: keep session replay off in the PostHog project, and add `/portal` to its replay URL blocklist in case it is ever turned on. A short privacy note naming PostHog on the public site is a product decision still open.
- **Content-Security-Policy.** `vite.config.ts` turns on SvelteKit's `csp` in `hash` mode with `script-src 'self' 'unsafe-hashes' 'sha256-7dQwUgLau1NFCCGjfn9FsYptB6ZtWxJin6VohGIu20I='`, `object-src 'none'` and `base-uri 'self'`, so every prerendered page and `200.html` carries a `<meta>` policy with the hash of its start-up script. The service sends the full policy as a header (`static_files::content_security_policy`); browsers enforce both:
  - `default-src 'self'; script-src 'self' 'unsafe-hashes' 'sha256-7dQw...' <page hashes>; style-src 'self'; style-src-attr 'unsafe-inline'; img-src 'self' data: blob: https://assets.scottylabs.org; connect-src 'self' [analytics host]; frame-src https://www.youtube-nocookie.com https://player.vimeo.com https://calendar.google.com; frame-ancestors 'none'; base-uri 'self'; form-action 'self' [Keycloak origin]; object-src 'none'`.
  - For an HTML file or the `200.html` shell, `<page hashes>` are computed from the inline scripts in that response (`inline_script_hashes`: no `src`, and no `type` other than JavaScript or `module`), so they always equal the ones SvelteKit wrote; a `304` carries the same header. Other responses get the policy without page hashes.
  - The one inline event handler allowed is Svelte's `this.__e=event`, which it writes on prerendered images with an `onload` or `onerror` handler so the event survives hydration; it can do nothing else.
  - `style-src-attr 'unsafe-inline'`: Svelte writes style attributes into prerendered pages and into the markup it creates in the browser (`style:` directives, `style="..."`), and removing them all is not practical. A style attribute cannot load anything the other directives refuse. Stylesheets and `<style>` elements must come from this origin.
  - The analytics host is in `connect-src` only for public pages (`Audience::Public`), and only when the service has both PostHog values (`Config::analytics_origin`, from `PUBLIC_POSTHOG_HOST`). Portal pages cannot connect to it, and no page can load a script from it.
  - The Google Calendar frame is the events calendar on the home page; the video hosts load only after a visitor presses Play.
- **The rich text editor writes only sanitized markup.** Every write into the editable element goes through `ShownRichText` (`src/lib/editor/shown.ts`), which runs `sanitize()` on the value from outside (load, kept work, undo) and on what it puts back when focus leaves. Before, leaving the field without typing put the API's raw value into the page. The server half, sanitizing rich text on read in the editor responses as `approvals.rs` does, belongs to the API owners.
- **Kept work is cleared** (`clearKeptChanges` in `src/lib/portal/kept-changes.ts`, called from `PortalSession`): all of it when the person signs out; everything but that person's when their session ends, and whenever `GET /api/me` answers, so a different person signing in on the tab never finds someone else's work there.
- **Signing out follows Keycloak's end-session address** when `POST /api/auth/logout` answers `200` with `{ "endSessionUrl": "<url>" }` (an absolute https address, or http on localhost); otherwise, including the current `204`, the person goes to `/portal/sign-in/?signedOut=1` as before. The sign-in track owns the server side.
- **New-tab links** all carry `rel="noopener noreferrer"`; `tests/links.test.ts` reads every component's markup and fails on a `target="_blank"` (or a passed-through target) without it. `Button` and `IconButton` add it themselves.
- **Dependencies:** `@sveltejs/kit` 3.1.1 and `@sveltejs/adapter-static` 4.0.1, the first releases without the published advisories (none of which applied here). Drop-in; `deno.lock` updated.

### 2026-11-25, follow-ups after the final review

What the final backend and web fixers left for each other, then the fixes for the end-to-end review of every flow (the first four items).

- **Edits typed after the session ends are kept** (spec S-5, CP-7, PR-7). A 401 still marks the session expired and sends the person to sign in and back (`?returnTo=&reason=session_ended`), but first `PortalSession.onEnd` callbacks run, while the page is still open (Svelte shows a closing page the session as it was before the change, so pages cannot do this as they close). The project editor, the committee page editor and the ticker composer keep what the service does not have in `sessionStorage` under `sl:kept-changes:<key>` (`project:<id>`, `committee-page:<slug>`, `announcement:<id>`, `announcement:new` or `announcement:new:<from>`) as `{ userId, base, value, keptAt }`, where `base` is what the work was written on: the project's `updatedAt`, the page's `draftVersion`, the announcement's `updatedAt`, or `""` for a new one. When the page opens again it puts the work back on its own if the same person is signed in and `base` still matches (the page editor records it as one undo step, then saves as usual; a toast says so), asks with "Put my changes back" or "Discard them" if the record changed since, and drops work that is someone else's, older than 24 hours or not shaped like the record. The committee page editor makes no save or lock release after its session ended; the lock lapses after five minutes. Helpers in `src/lib/portal/kept-changes.ts`; the notice is `KeptChangesNotice` in the UI kit.
- **Dragging a live announcement** (spec TK-3) follows the pointer on the window instead of the handle, whose pointer capture the browser drops when the keyed row moves in the DOM. Letting go anywhere ends the drag and sends one `PUT /api/announcements/order`; Escape, `pointercancel` or the window losing focus put the rows back without saving. Focus returns to the moved handle.
- **The composer's preview wraps a long message** (spec TK-6, F5 step 4) at its real size, as the approval dialog does, instead of clipping it, so the committee chip, the link arrow and both dog marks stay in view on desktop and phones. The home page ticker still keeps each announcement on one line and scrolls it.
- **Smaller fixes:** the Add person and Add a lead dialog drops the service's message about the last person once another is chosen; the project editor drops a failed action's banner at the next edit; "Show all projects" on the Projects page moves focus to the emptied search box instead of leaving it on the body.
- **`ProjectCard.videoUrl: string | null`** is required and filled from the published version, so the Projects page's featured card shows "Watch video" (to `/projects/<slug>/#watch`) and committee pages' project cards carry it too. `ProjectDetail` gets it from the card it flattens and no longer has its own field, so it is sent once. Draft and approval previews (`draftCard`, `projectCardFromDoc`) set it from the doc. Project pages go to the part their address points to once the project has loaded (the browser looked before it was on the page) and focus it, so `#watch` lands on the click-to-load video below the sticky header.
- **Search over whole descriptions** (spec P-5 and section 11, which said the API takes no `q`): `GET /api/public/projects` takes `q` (see Public). It mirrors `matchesSearch` in `src/lib/projects/filter.ts`: the same fields joined with spaces, the description as `richtext::plain_text`, the status label too, words split on what JavaScript's `\s` matches, and every word must appear. Folding matches `fold`, all in Rust: compatibility decomposition (NFKD, the `unicode-normalization` crate sqlx already builds), then characters with the Unicode `Diacritic` property dropped (a table listed from Deno's `/\p{Diacritic}/u`), then lowercase. It does not use Postgres's `normalize`, which fails unless the database is UTF-8 (the Nix test check's is not). `routes/projects/search.rs` holds it; descriptions are read and folded on the blocking pool. The answer keeps the public cache headers.
- **The Projects page** still filters the loaded cards at once and keeps `?q=` and the committee tab in the address as before. From two characters (trimmed, with a word left after folding) it also sends the search to the service 250 ms after typing stops, aborting the previous request; answers are kept per folded search (`searchKey`), and only for the current search, except that while a search only narrows the last answer (typing on, `narrows`), the projects the service alone found for the earlier words stay until it answers (`pendingSearch`), so they do not vanish at every key press. The ids it matched join the local matches among the loaded cards, in their order, and in the committee counts (`filterProjects` and `committeeCounts` take `alsoMatching`; `remoteQuery`, `remoteMatches` and `awaitingRemote` decide the rest). While no card matches and the answer is pending, the page says "Searching the descriptions..." instead of the empty state; a failed request falls back to the local result without a message. The screen reader count waits for the answer when one is pending.
- **Project video links** must play one video: `models::check_video_url` also requires `blocks::video_id`, so channel, playlist and profile links are refused with the committee editor's message, "Use a link to one YouTube or Vimeo video, not a channel or playlist." Links are still normalized (`url::Url`, https only). Committee video blocks use the same check, and approval previews drop a stored link it refuses. The seeded projects have no video links.
- **Covers at least 1,200 pixels wide on the web.** The project editor's "Before you publish" lists "Cover photo at least 1,200 pixels wide" (required, done when there is a cover and `coverWide`), and the photos section says so. The publish button is not driven by the checklist for any item; a narrow cover is refused by the service at `media.0`, shown under the first photo and in the action message. The committee publish dialog lists the service's message at `blocks.0.image` for an uploaded cover narrower than 1,200 pixels (`coverWidthProblem` and `MIN_COVER_WIDTH` in `src/lib/media/images.ts`; bundled `/seed/` photos and unknown sizes pass).
- **Paging the expired ticker tab** (see Ticker): 50 at a time with a `before` cursor (`<RFC 3339 time in microseconds>_<id>`, opaque to the web) and `nextBefore` in `AnnouncementListResponse`. The portal's expired tab shows "Show more" with "Showing n of total" under the list; it appends the next page without repeating rows, moves focus to the first added row (or its card on phones), and announces "Added 50 more announcements. Showing 100 of 120." A reload, an action or another tab starts again from the first page. Helpers in `src/lib/ticker/portal/paging.ts`.
- **Decision: the committee page editor's canvas stays at 100%** (design review #16, which suggested scaling the sheet to fit). At about 60% the block toolbar's buttons would fall under the 24 px target size (WCAG 2.5.8), text edited in place would be about 11 px, and the floating format bar and the drag and drop math, which work in page pixels, would need scaling too. A "Fit" toggle for looking over the whole page could be added later.
- **Decision: JPEG variants stay 4:4:4.** `image` 0.25's JPEG encoder has no chroma subsampling option, and about 13% smaller files are not worth a new crate.

### 2026-11-25, final backend review fixes

- **Rich text is size-capped before sanitizing.** `richtext::clean` refuses sources over 64,000 bytes or 4,000 tags (`blocks.<i>.body` or `description`: "... has too much formatting to save"), because html5ever takes time that grows with the square of nested-table markup. Committee and project drafts are cleaned on the blocking pool (`routes::common::blocking`), a project's before its row is locked, and html5ever logs only errors.
- **Uploads**: at most 40 megapixels, checked from the header before decoding (`413`, "That image has too many pixels..."). Large originals are first averaged down with the box filter, very tall ones skip the resampling filter, and each width is encoded as soon as it is made.
- **Activity scope.** `permissions::activity_scope` decides what `audit::recent` (activity and the overview) shows: admins everything; everyone their own actions; every entry for committees the viewer directs; for committees they only lead in, announcement entries only while the announcement is open to them (`permissions::can_view_announcement`: not waiting for approval, or theirs) and project entries only for projects they may open (`can_view_project`). Other entries (people, committee pages) are unchanged. The query reads index-backed branches instead of one filter over the log.
- **One-line fields** (`models::check_line`) also refuse control characters such as tab and NUL, U+2028, U+2029 and the bidirectional embeddings, overrides and isolates: "Remove the special characters from <label>." `crate::json::Json` refuses a NUL in any string with `validation_failed`, and `routes::common::query_param` drops control characters from filters and searches.
- **Configuration.** `APP_URL` is required. S3 credentials come from `S3_ACCESS_KEY`/`S3_SECRET_KEY` or kennel's `AWS_ACCESS_KEY_ID`/`AWS_SECRET_ACCESS_KEY`, so deployments store uploads in their Garage bucket instead of Postgres. Partly set S3 or Keycloak settings log which variables are missing; startup logs the sign-in mode and where uploads go. `Config::from_vars(lookup)` reads settings through a function, for tests.
- **Covers at least 1200 pixels wide** (spec section 12): publishing refuses an uploaded project cover (`media.0`) or committee cover (`blocks.0.image`) narrower than 1200 pixels: "Use a cover photo at least 1,200 pixels wide. This one is <n> pixels wide." Bundled `/seed/` images are exempt. `ProjectChecklist` gains `coverWide` (true without a cover). The web checklists show it since the follow-ups entry.
- **Approving too late** now says "This announcement leaves the ticker in less than 5 minutes, or already has. Send it back and ask the lead to pick a later time."
- **Indexes.** Migration `0601_lookup_indexes.sql` indexes `approvals` by subject and by submitter and `audit_log` by subject and by actor. The ticker lists read the latest approval of every row in one query (`approvals_core::latest_of_each`), without the snapshot. The expired tab is paged since the follow-ups entry.
- **Seed markers.** Migration `0602_seed_markers.sql` adds `seed_markers (name, created_at)`, marked `ticker` for databases that already have or had announcements; the ticker seeds only when it claims the marker.
- **Rollbacks.** `db::migrate` allows migrations it does not know, so the previous release starts against a newer schema. Every migration must therefore stay additive.
- **Errors.** Malformed announcement ids answer `404` in the error shape, like the other tracks (`routes::common::parse_id`). A wrong method on an API route answers `405 method_not_allowed` with `Allow`. A governance repository that is down or answers with something unusable gives `502 upstream_failed` instead of `400`. Both codes are in `ApiErrorBody` in types.ts.
- **Rate limits** take the visitor from `CF-Connecting-IP` behind a loopback peer, then the right-most `X-Forwarded-For`: Caddy in kennel replaces `X-Forwarded-For` with the tunnel's loopback address, so every visitor shared one bucket.
- **Logs** are plain text unless stdout is a terminal, every request has an info span with its method and path (never the query), internal errors log their causes on one line, and the trace layer's failure line moved to debug, so each 500 is logged once.
- **Secrets.** `PORTAL_DEV_LOGIN` (default `1`) and `PORTAL_DEV_LOGIN_KEY` are declared in the preview profile only, so keyed dev sign-in works on previews and other profiles never resolve the key. The dev profile no longer defaults `SEED_DEMO_CONTENT` to `1`: on kennel's long-lived dev deployment it gave real accounts with persona Andrew IDs the persona roles. Local development still gets it from the dev shell.
- **CI.** `services/site/src/routes/auth/test-signing-key.pem` is in `extraSources`, and `group_thousands` uses `is_multiple_of`, which stable clippy requires, so `site-clippy` and `site-test` build.
- **Shared route helpers** live in `routes::common`: `blocking`, `query_param`, `parse_id(raw, what)`, `no_store` and `quoted`.

### 2026-11-25, final web review fixes

Web only; no endpoint changed. `types.ts` gains one optional field.

- **`ProjectCard.videoUrl?: string | null`** (optional): when it is set, the featured card on the Projects page shows "Watch video", linking to `/projects/<slug>/#watch` (Figma 52:8705). The service did not send it yet; since the follow-ups entry it does, and the field is required.
- **One video reader and one link host helper.** `videoEmbed` lives in `src/lib/media/video.ts` (also exported from `#lib/media/index.ts` and `#lib/projects/index.ts`) and returns `{ provider: "youtube" | "vimeo", providerName, embedUrl }`. Committee and project pages play the same links, the ones `blocks::video_id` accepts (keep the two in step; this replaces `src/lib/committees/video.ts` in the 2026-11-24 committee pages review entry), and project pages keep the start time and the unlisted Vimeo hash. `models::check_video_url` accepted any YouTube or Vimeo link for projects; since the follow-ups entry it requires one `blocks::video_id` can play. `linkHost(url, { httpsOnly })` lives in `src/lib/site/links.ts` (hostname without `www.` or port, null for anything that is not an http or https link).
- **Seed photos in responsive sizes.** `src()` and `srcset()` in `src/lib/media/images.ts` treat `/seed/<path>.jpg` with a known width like uploads: `seedWidths` lists 480 and 960 where they are at most 80% of the width, then the width itself, and `seedVariantPath` names the copy (`/seed/photos/c10-480.jpg`). Run `deno run -A scripts/seed-sizes.ts` from `sites/web` after adding or replacing a seed photo.
- **Portal fields** are 48 px tall with 16 px text (`--p-field-height`, `--p-field-font`, `--p-field-line` in `tokens.css`). `.p-fields-compact` on a container gives 40 px and 14 px, for the committee page editor and list toolbars. Below 768 px every field uses 16 px text, so iOS does not zoom in on focus.
- **Phones, full screen.** Below 600 px the portal layout hides its top bar and tab bar on `/portal/ticker/<new or id>/` and `/portal/projects/<id>/` and sets `--p-tabbar-height` to 0, so bars that dock above the tab bar dock at the bottom edge. Those pages draw their own app bar.
- **Publishing a committee page returns to the list** at `/portal/committees/?highlight=<slug>`, which marks that row (spec F7 step 5). The publish dialog says so.
- **Rich text editing on the canvas.** `BlockRenderer` takes `textEditor` (the `RichTextEditor` component), which the editor canvas passes in; render blocks no longer import editor code. On the canvas the page's headings are one level lower (`headingTag` in `src/lib/committees/render/types.ts`).
- **Lighter public pages.** The prerendered Projects, Committees and committee pages preload their content JSON from the HTML; hero images load with `fetchpriority="high"`; project cards below the featured one are lazy; PostHog loads after the page in its own chunk; JetBrains Mono is not preloaded and Latin text uses a 44 KB WOFF subset (`JetBrainsMono-Regular-latin.woff`). The root error page loads the portal's error card only for portal addresses. The prerender crawler follows those preload links, so `vite.config.ts` has a `handleHttpError` again, narrowed to 404s under `/api/`; every other prerender error still fails the build.
- **Phone project cards** keep the 4:3 cover and the monogram of spec P-2 and P-3; Figma 44:8364 shows a square with the name.
- **Search over the whole description** (spec P-5): the Projects page filters its loaded cards, whose excerpt is the first 240 characters of the description, as you type, and searches of two or more characters also ask `GET /api/public/projects?q=`, which reads the whole description; projects it finds are added. See "2026-11-25, follow-ups after the final review".

### 2026-11-24, sign-in hardening and roles from governance

- **Callback checks**: the callback refuses an `iss` that is not exactly the Keycloak issuer (RFC 9207) and uses up the attempt; a callback without `iss` still works. Provider errors other than `access_denied` give `failed` and also use up the attempt. ID tokens must be RS256. Each unsuccessful sign-in logs why (see Keycloak sign-in). The contract used to say Ricochet forwards only `code` and `state`; it forwards every provider parameter.

- **Committee heads become leads**: the governance sync also adds a `lead` assignment (`can_post_ticker` true, no project editors, `appointed_by` null, source `governance`) for each head listed in `committees.yaml` who has no role in that committee, in the same transaction as the director sync. Existing director and lead rows are never changed. Leads with source `governance` whom governance no longer lists are removed with their project editors, and each removal is a sentence in `messages`. Tech is not part of the head sync. A committee missing from the file keeps its governance leads.

- **Editing a governance lead keeps it**: `PATCH /api/people/assignments/:id` on a lead sets its source to `portal`, so later syncs leave it alone. When governance stops listing a head whose lead role the portal keeps, the sync says so once in `messages` and writes a `role.sync` audit entry ("Kept <name> as a lead in <committee>; governance no longer lists them as a head").

- **Removals on the People page stick** (review fix): `DELETE /api/people/assignments/:id` on a director or lead, and a `PATCH` that moves a director or lead to another committee or role (except lead to director in the same committee), record an opt-out for the old committee. The sync skips opted-out people in that committee, for directors and heads alike, and says so in `messages` on every run while governance lists them. Opt-outs are forgotten once governance stops listing the person in that committee. Migration `0501_governance_sync.sql` adds `governance_opt_outs (user_id, committee, created_at)` and `governance_heads (committee, user_id)`, the heads listed at the last sync, which the "kept" message needs. The response shape is unchanged.

- **`GovernanceSyncResponse`** gains `headsAdded`, `headsRemoved` and `headsUnchanged`. `added`, `updated` and `unchanged` still count directors only. `messages` lists skipped entries first, then removals.

- **The sync runs on its own** at startup and every six hours (see "Where roles come from"), switched by `GOVERNANCE_SYNC`. Unset, it is on everywhere except local development with open dev sign-in, so builders working side by side do not fetch governance at every start. Set `GOVERNANCE_SYNC=1` to try it locally. `Config` gains `governance_sync` (false in `testing::config()`), and `routes::spawn_governance_sync(state)` starts the task.

- **Dev persona**: the Tech director persona is `apallati`, the Tech director's Andrew ID in governance, so a sync in development finds the persona instead of adding a second Tech director. Seeding renames an existing `anishp` user in place. The ticker seed's CMU Maps sample now names `apallati` too (a one-line change in `seed/ticker.rs`).

- **Local development**: the pitchfork `site` process now loads the dev profile's secrets and logs at `info,site=debug` (kennel's shell sets `RUST_LOG` for a crate named `scottylabs_org`, which hides this crate's logs; a `cargo run -p site` by hand needs `RUST_LOG=info,site=debug`). Open the portal at `http://localhost:5173`.

- **Tests**: the mock Keycloak in `routes/auth/tests.rs` signs ID tokens with a throwaway RS256 key checked in at `routes/auth/test-signing-key.pem` (the file says it is a test key) and serves a JWKS with an encryption key beside it, like the realm. No crate was added. `.gitleaks.toml` extends gitleaks' default rules and allows only that file, so the commit hook accepts it.

### 2026-11-24, integration

The merged tracks wired together. No shape in `types.ts` changed.

- **Committee pages list the Projects page's cards.** `GET /api/public/committees/:slug` and the editor's `projects` come from `routes::projects::public::published_cards(db, Some(committee))`, so a committee page shows exactly the cards `GET /api/public/projects?committee=` returns: the published version's committee, featured first, then by name. The committees module no longer has its own project query or `ProjectCard`.

- **Approvals run through the real hooks.** `routes/approvals/tests.rs` has no ignored tests; it also covers approving a project (published from the snapshot, one `project.approve` entry) and sending one back (the note in the lead's `mine=true` list, one `project.send_back` entry).

- **Head tags.** The project tags `static_files.rs` injects carry `data-sl-injected` and the root layout removes them as the app starts (see "Static serving"). The injected title is now `<Name> - ScottyLabs`, the same as the page sets. Committee pages and the Committees index return `{ ownDescription: true }`, so they have one description.

- **Web wiring.** `/portal/projects/?add=1` opens the Add project dialog for people who may add projects and then drops the flag with `goto(..., { replace: true, reset: false })`; for others it only drops the flag. The review dialog draws announcements with `#lib/ticker/TickerItem.svelte` (wrapping long messages so the approver reads all of them) instead of a copy. The home page's three project panels link to their project pages. The temporary prerender 404 handler in `vite.config.ts` is gone; the build prerenders `/projects/`, `/committees/` and the seven committee pages.

- **Rust clean-up.** `cargo clippy -p site --all-targets -- -D warnings` passes. Removed as dead: `AppError::forbidden`, `Problems::is_empty`, `permissions::leads`, and `ApprovalRow.submitted_by` and `created_at` (tests read `submitted_by` from the table). `AppError::AuthNotConfigured` stays, allowed, because `auth_not_configured` is one of the contract's error codes. `text_enum!` allows `ALL` once, since only some enums list their variants, so no module allows dead code around the enums it declares. The `[package.metadata.cargo-machete]` list is gone (`cargo machete` finds nothing unused), and `tower`, used only by tests, is a dev-dependency only. The "mixing target features will cause an ABI mismatch" warnings come from the dev toolchain (Cranelift for this crate, LLVM for `std` and `zune-jpeg`), not from the code.

- **Test databases.** `#[sqlx::test]` names a test's database after the test alone, and the local Postgres server is shared by every worktree, so the same test running in two processes at once (two worktrees, or a commit hook beside a manual run) drops and recreates the database under the other run whenever that run holds no connection for a second (sqlx closes idle test connections after one). The tests that encode or decode images for seconds failed this way: `routes::media::tests::{uploads_a_jpeg_and_serves_each_width, serves_images_stored_in_s3}` and `media::tests::{slow_downloads_are_not_cut_off, stalled_downloads_end_with_an_error}`. They now use `testing::IsolatedDb`, a migrated database named for the run and dropped when it goes out of scope; use it with `#[tokio::test]` for any test that spends seconds away from the database. The dev build's Cranelift backend does not unwind, so a failed test or a killed run skips that drop; each new `IsolatedDb` first drops the `_sl_test_*` databases more than an hour old.

- **Dialogs return focus to the page heading when their opener is gone.** `Modal` (so `Dialog` and `Sheet`) gives focus back to the element that had it when the dialog opened while that is still on the page. When it is not (a dialog opened on arrival, as for `?add=1`, or by a control that went away) and focus is lost, focus goes to the page heading through `focusPageHeading()` in `src/lib/portal/ui/focus.ts`, which the portal shell also uses after navigation. A page that moves focus itself on closing keeps that.

### 2026-11-24, projects and projects review fixes

- 2026-11-24, projects review fixes. Additive only.

  - Public project lists, `?committee=` and `published_cards(db, Some(committee))`, filter on the published version's committee (`published->>'committee'`), not the `committee` column, which follows the draft. A move saved in a draft shows on committee lists only once it is published (PR-7). Other modules that list a committee's published projects should do the same.
  - Discarding a draft that moved the project needs `can_publish_projects_for` the published committee, like any other move.
  - Migration `0201_one_featured_project.sql` adds a unique partial index, so at most one project is featured. Featuring takes a transaction advisory lock (`hashtext('projects.featured')`) first, so two admins featuring at once take turns instead of one failing. The seed features TartanHacks only when no project is featured yet.
  - The root layout's generic meta description is now a fallback: a page that writes its own returns `{ ownDescription: true }` from its `load` (the Projects page and project pages do), and the root layout leaves the default out. Other public pages with their own description can do the same.

- 2026-11-24, projects. Additive only.

  - `ProjectEditorResponse.project.approvers` lists the directors of the owning committee, so a lead's "Send for approval" can say who decides. `pendingApproval` is the waiting submission, or the sent back one (with its note) until the project is published again; approved and withdrawn ones are not shown.
  - `GET /api/public/projects` also takes `?committee=<slug>`, as the spec lists. Other route modules can build the same cards with `super::projects::public::published_cards(db, Some(committee))`, for example the committee page's projects block.
  - Rules the routes follow: a lead's edit to the draft (or discarding it) withdraws their waiting approval, since it no longer matches what was sent; a director's edit does not. Publishing directly and archiving withdraw a waiting approval. Approving publishes the snapshot and keeps later edits in the draft. Archived projects cannot be published or approved until they are unarchived. Featuring a project stops featuring the others, so one is featured at a time. A new project never takes an address that redirects to another project, and an admin cannot rename a project to one either. `builtWith` drops the owner and is kept in committee order. Uploaded photos take their width and height from the `media` table.
  - Saving drafts autosaves often, so a run of edits by one person within 15 minutes is one audit entry ("Edited Nova") whose time moves forward, instead of one entry per save.
  - The seed publishes the 15 projects of the approved Projects page. Terrier has no picture yet and shows its monogram, as in the design; republishing it needs a cover like any other project. The TartanHacks repository and the terrier.build site do not exist, so they are left out.

### 2026-11-24, committee pages review fixes

No shape in `types.ts` changed.

- **Video links play one video.** Saving answers 422 at `blocks.<index>.url` ("Use a link to one YouTube or Vimeo video, not a channel or playlist.") unless the link names one video the page can embed: YouTube watch, `youtu.be`, embed, shorts and live links, or a Vimeo link with a video number. `blocks::video_id` mirrors `videoEmbed` in `src/lib/committees/video.ts`; keep the two in step. Before, channel and playlist links saved and published, and the block then vanished from the public page. `models::check_video_url` is unchanged, so project video links keep their current rule.
- **The edit lock follows activity.** The editor asks to keep the lock once a minute only when the person clicked, typed or changed something since its last request, and saves refresh the lock as before. An editor left open in a background tab therefore frees the page five to six minutes after the last activity, as spec F7 describes. Back after a lapse, the editor asks again at once and loads any draft saved in the meantime. No endpoint changed.
- **Video posters are described.** The poster renders with its alt text, which publishing already required, so screen reader users hear it.
- **Path errors are sentences.** `check_button_url` refuses unsafe site paths with "Enter a path on this site that starts with a single /, such as /sponsors/."

### 2026-11-24, committee pages

Additive; no shape in `types.ts` changed.

- **Buttons may link to this site.** `models::check_button_url` also accepts paths on this site such as `/sponsors/`, so the Finance call to action can open the Sponsors page. It refuses `//host`, backslashes, spaces and control characters, which browsers read as links elsewhere.

- **Galleries fill up in drafts.** A draft gallery may hold 0 to 6 photos, so it can be filled one upload at a time; publishing still needs 3 to 6.

- **Publishing also needs** a cover photo, alt text on every image (cover, image blocks, gallery photos, video posters), a photo in every image block, a link in every video block, at least one team in a Teams block and a value for every Stats number. The editor checks the same rules in the browser (`src/lib/committees/editor/publish-checks.ts`) to list them before publishing.

- **Field paths.** Validation errors name fields `blocks.<index>.<prop>` (for example `blocks.3.items.1.value` or `blocks.0.image.alt`) and `settings.<field>`, by position in the document that was sent. `blocks` alone is about the page as a whole.

- **Text limits**, in characters after trimming: tagline 120; headings 80; captions 200; quotes, call to action text and the teams introduction 240; Stats value 12 and label 60; team name 40 and description 140; attribution 80; button label 40; video title 80; index description 300; teams line 120; meeting time 60; room 80; Slack channels 80. Text bodies hold 4000 visible characters. At most 20 teams. Plain text fields are one line. Block ids are 1 to 64 letters, digits, `-` and `_`. The web mirror is `src/lib/committees/limits.ts`.

- **Stored forms.** Rich text is sanitized and text that is only empty paragraphs becomes `""`. Quotes are stored without quotation marks; the page adds them. Team channels, like `settings.slackChannel`, are stored lower case without `#`. A button with neither label nor link becomes `null`. Uploaded images must exist in `media`, and their stored width and height replace what the browser sent.

- **The edit lock.** Migration `0301` adds `committee_pages.lock_since`, when the holder took the lock (`EditLock.since`). `POST /lock` answers with the `EditLock`. Saving, publishing and discarding also take a free or lapsed lock, so editing carries on after a laptop wakes up; all of them answer 409 with the holder's name while someone else holds a fresh lock. `DELETE /lock` answers 204 and only ever clears the caller's own lock.

- **Writes.** `PUT /draft` answers 409 when `baseVersion` is not the latest version. `POST /publish` answers 409 when `draftVersion` is not the latest and 422 with fields when the draft cannot go live; `POST /discard` answers 409 for a page never published. Both answer with the `CommitteePageEditorResponse`. Unknown slugs are 404 everywhere.

- **Changes** are sentences in page order: "Moved Stats above What we do", "Made What we do full width", "Added an image block", "Edited Teams", "Edited the cover", then "Removed Quote" and "Changed the page settings". A text block is named by its heading, any other block by its type. The list is empty when nothing changed; a page never published gets one sentence saying so.

- **Audit.** `committee_page.edit_draft` ("Edited the draft of the Tech page") is written at most once every 30 minutes per person and page, because the editor saves every couple of seconds; `committee_page.publish` and `committee_page.discard` are written every time.

- **Public endpoints** list only committees whose page is published; a page answers 404 until it is published. `projects` are read from `projects.published` (name, summary, the description for the excerpt, status, kind, siteUrl, repoUrl, and the first of `media` as the cover), featured first, then by name.

- **Seed.** The designs show two video blocks without real videos (the TartanHacks recap and a Foundry fireside chat); they are seeded as image blocks with the same photo and caption.

- **Portal routes.** `/portal/committees/<slug>/` (the activity link) redirects to `/portal/committees/<slug>/edit/`, which opens read only for people who cannot edit the page.

### 2026-11-24, overview, approvals and people

All additive. `types.ts` gains one field, `OverviewResponse.committees.lastEdited`.

- **`GET /api/approvals`**: `status` defaults to `waiting` (oldest first, at most 100); `decided` means approved or sent back (newest decision first, at most 50). Without `mine=true`, someone who approves for no committee gets `403`. With `mine=true`, only the latest submission of each subject is listed, and only while the subject still exists, the same rule Needs attention uses; so after a lead resends something, the old sent back one is gone from their list. Approvers' lists keep every decision. What reaches the browser is cleaned again: links that are no longer `https` (or YouTube and Vimeo for video) become `null`, the description is sanitized, and images from outside the site are dropped. A snapshot that does not read as its subject type comes back with `announcement` and `project` both `null` and is logged.

- **Deciding**: `POST /api/approvals/:id/approve` takes an optional body `{ "note" }` (up to 500 characters), kept on the approval and shown to the lead; a request without `Content-Type` has no note. Both decisions answer `200` with the updated `Approval`. `404` for an unknown id, `403` unless `permissions::can_approve_for` the committee, `409` when it is no longer waiting. The hook runs first, then the approval row is updated, all in one transaction. `routes/approvals.rs` writes no audit entry: the subject hooks (`announcement.approve`, `announcement.send_back`, `project.approve`, `project.send_back`) record the decision against the subject, so Recent activity shows each decision once.

- **`GET /api/overview`**: `ticker.mineLive` and `nextToExpire` count live announcements from sources the viewer manages or that they posted. `committees.published` counts all seven; `withUnpublishedChanges` lists pages the viewer edits whose draft differs from the published one (a page never published counts); `covers` has all seven in site order, from the published cover block. `lastEdited` (`{ slug, at, by }`, `by` a `PersonRef` or `null`) gives, for each page the viewer edits, the last save of its draft (`draft_updated_at` and `draft_updated_by`); a director's "Your committee page" card says "Last edited 3 weeks ago, by you." from it. `projects.total` counts every project that is not archived; `inScope`, `published` and `drafts` count those the viewer edits (all for admins, their committees' for directors, their own for leads); `covers` has up to six of those, published first.

- **Attention items**: severity `info` for approvals and unpublished pages, `warning` for sent back and expiring, `neutral` for projects and stale pages. Actions: Review `/portal/?review=<approvalId>`, Edit `/portal/ticker/<id>/` or `/portal/projects/<id>/`, Extend `/portal/ticker/?highlight=<id>`, Review or Open `/portal/committees/<slug>/`, Add image, Add summary or Add link `/portal/projects/<id>/`. A lead's sent back item shows only for the latest submission of a subject that still exists, with the note as `detail`. A project misses links when it has neither a site nor a repository link. A page with unpublished changes is not listed again as stale. Dates in titles and details are in Pittsburgh time.

- **Web**: `/portal/?review=<approvalId>` opens the review dialog on the overview (`reviewHref` in `#lib/portal/approvals/index.ts`), read only once decided. The overview's "Add project" goes to `/portal/projects/?add=1` (`NEW_PROJECT_HREF` in `#lib/portal/overview/copy.ts`); no route serves `/portal/projects/new/`. The projects list should open its Add project dialog when `add=1` is in the address and then drop the flag with `goto(..., { replace: true })`; until it does, the link still lands on the list with its Add project button. `src/lib/portal/approvals/AnnouncementPreview.svelte` mirrors the ticker item until `#lib/ticker` is merged, when it can use `TickerItem`. The People page reads `GET /api/projects` for project names and the lead checkboxes and still works when that fails. In SvelteKit 3, `goto` takes `{ replace: true, reset: false }` and links take `data-sveltekit-reset="false"` to keep focus and scroll; `keepFocus` and `noScroll` no longer exist.

- 2026-11-24, shared web components. Additive only. `Field` gives its label an id and `FieldContext` gains `labelId`, so controls that a `<label for>` cannot name (the contenteditable in `RichTextEditor`) use `aria-labelledby`. New shared pieces for the projects, committees and people tracks: `src/lib/editor/` (`RichTextEditor`, `RichText`, `sanitize`, `normalizeRichText`, `plainText`, `plainTextLength`; the client mirror of `richtext.rs`), `src/lib/media/` (`Img`, `ImageField`, `ImageListField`, `src`, `srcset`, `aspectRatio`, `uploadImage`), `PersonPicker` in the portal UI kit, `src/lib/projects/ProjectCard.svelte` (also exports `PROJECT_STATUS_LABELS`, `PROJECT_KIND_LABELS` and `projectPath`) and `src/lib/site/SelectionBox.svelte`. The projects track owns `src/lib/projects/` and may change the card; keep its props.

- 2026-11-24, platform web. `sites/web/vite.config.ts` proxies to `SITE_SERVICE_URL` when it is set (default `http://127.0.0.1:8080`), so builds can run side by side on other ports. Prerendering warns instead of failing when `/projects/` or `/committees/` answer 404, because the header links to them before those routes exist; remove that handler once both routes are in.

- 2026-11-24, platform web. How portal pages plug in: call the API with the helpers in `src/lib/api/client.ts`, whose paths are relative to `/api` (`get<MeResponse>("/me")`); read the signed-in person with `getPortalSession()` and show confirmations with `getToasts()` (both from `src/lib/portal/index.ts`); build pages from `src/lib/portal/ui/index.ts` and the `p-table` classes. Render the page's `<h1>` straight away (`PageHeader` does), since the shell moves focus to it after navigation. Unknown `/portal/` addresses get the portal error card from the root `+error.svelte`.

### 2026-11-24, ticker

All additive.

- **`GET /api/announcements/:id`** returns `AnnouncementResponse` (`{ announcement }`), for the edit page. Waiting announcements are visible only to their author and to those who manage the source (admins, its director); anyone else gets 404.

- **Write responses**: `POST /api/announcements` and `POST /:id/repost` answer `201` with `AnnouncementResponse`; `PATCH /:id`, `POST /:id/extend` and `POST /:id/end` answer `200` with it; `PUT /api/announcements/order` answers `200` with the live tab's `AnnouncementListResponse`; `DELETE` answers `204`.

- **`AnnouncementListResponse`** gains `counts` (`{ live, waiting, expired }` as this person sees them, for the tab badges) and `approvers` (for leads, the directors of each committee they post for with approval, so the composer can name who approves). `waiting` shows a person their own submissions plus those from sources they manage; admins see all.

- **Rules settled while building**: a new or approved announcement joins the end of the loop (`position` after every live one). `expiresAt` must be in the future and at most 30 days from now when posting, editing or reposting; `extend` adds days but caps at 30 days from now and refuses (422) when that gains nothing. Only live, unexpired announcements can be extended or ended (409 otherwise); expired and ended ones cannot be edited (409, post again instead). Editing a pending or sent back announcement resubmits it: the row returns to `pending_approval` and a new approval with the new snapshot replaces the waiting one, still credited to the original author. When an approver edits someone else's waiting announcement it stays waiting the same way (logged as "Edited <message> while it waits for approval"); they approve it from the overview as usual. Waiting announcements cannot move to the `club` source, since approvals belong to a committee. Managers may delete any announcement; authors may delete their own while it waits.

- **Approval hooks**: `announcements::apply_approval` checks `permissions::can_approve_for`, requires `pending_approval`, refuses with `validation_failed` (field `expiresAt`) when the snapshot's expiry is less than 5 minutes away, takes the six-slot lock and refuses with `conflict` when the ticker is full, then publishes the snapshot (`starts_at` now, `approved_by`, `position` at the end). `apply_send_back` sets `sent_back`. Both write their own audit entry about the announcement ("Approved <message> for the ticker", "Sent <message> back for changes", with the message in curly quotes); `routes/approvals.rs` only needs to update the approval row (and may log the decision with `subject::APPROVAL`). They lock the announcement after the approval row, the same order the ticker's own writes use, so they cannot deadlock. Both refuse with `conflict` an approval that is no longer the announcement's open submission: one whose status is neither `waiting` nor the decision being made (so `routes/approvals.rs` may update the approval row before or after calling the hook), one stored as `withdrawn`, or one replaced by a newer waiting approval after the lead edited the announcement. An approval about another subject type is an internal error. Pass the row as `approvals_core::lock_for_decision` returned it.

- **Six slots under concurrency**: every write that can add to or reorder the live set takes `pg_advisory_xact_lock(724610)` before counting.

- **Web**: the ticker is `#lib/ticker/Ticker.svelte`. Without props it fetches `/api/public/ticker` in the browser (and again every 5 minutes); pass `announcements` and `variant="card"` to show a given list inside a card, as the portal's live preview does. Portal links other tracks can use: `/portal/ticker/` (`?tab=waiting|expired`, `?highlight=<id>` marks a row), `/portal/ticker/new/` (`?from=<id>` prefills from an announcement that left the ticker) and `/portal/ticker/<id>/` to edit. In `Hero.svelte` the strip keeps its place between the two hairlines on wide screens and moves under the hero text on screens up to 1024 px, as in the mobile design; the hero clips with `overflow: clip` so focusing a moving announcement cannot scroll it sideways.

- **Ticker modes after review**: one announcement stands still only while it fits between the label and the controls; when it does not, it scrolls like a longer list, so it can be read in full (T-9 reads "shown static" for an announcement that fits). With reduced motion nothing scrolls: still announcements are cut short with an ellipsis, on phones they take two lines without the committee chip, and the list of every announcement opens from the position button ("2 of 6") or, for a single cut-short one, from "All 1". With no announcements, `Hero.svelte` keeps one hairline where the strip was. The browser remembers the last feed size in `localStorage` (`sl-ticker-count`) and skips the loading placeholder when it was empty, so an empty feed does not make the page jump on later visits.

### 2026-11-24, platform backend review fixes

- **Activity links depend on the viewer**: `audit::recent` leaves `href` null when the viewer cannot open the page, so a lead no longer gets a link to People (403 for leads) or to a project they do not edit. `audit::href` itself is unchanged and viewer independent. The project rule is the new helper `permissions::can_view_project(viewer, project_id)`, which mirrors `GET /api/projects`: admins and directors see every project, leads the ones they edit. The projects track can use it for `GET /api/projects/:id`.
- **`auth::Assignment` has no `id` field any more**; nothing read it. Build test viewers with `permissions::tests::viewer`.
- **`MediaStore::new` returns `anyhow::Result`** (one `?` in `state.rs`). It builds its own client for streaming images from S3, with a connect timeout and a 15 second stall timeout but no deadline for the whole download, so slow visitors no longer get cut-off images. Uploads to S3 keep the shared client and its 15 second total timeout.
- **`PROJECT_GROUP`** is still required for Keycloak sign-in and is logged at startup with the admin group. It grants nothing.

### 2026-11-24, platform backend

All additive; no shape in `types.ts` changed.

- **`crate::json::Json`** (`src/json.rs`): use it instead of `axum::Json` to read request bodies. Its rejections use the API error shape (`bad_request`, `unsupported_media_type`, `payload_too_large`); axum's own answer in plain text. Responses can keep using `axum::Json`.
- **Test helpers** in `src/testing.rs` (`cfg(test)`): `testing::app(db)`, `testing::user(&db, andrew_id, roles)`, `UserSpec` for leads with projects, `testing::project` and `testing::published_project`, and `Req` to send requests with the session cookie and CSRF token. Usage is at the top of the file.
- **Role assignments for the browser** live in `crate::auth::roles` (`RoleAssignmentDto`, `assignments_for`), shared by `/api/me` and the People endpoints.
- **Activity for the overview**: `audit::recent(db, viewer, limit)` returns the viewer's `ActivityEntry` list, scoped as `GET /api/activity` is. Links come from `audit::href`, which knows the subject types in `audit::subject` (`announcement`, `committee_page`, `project`, `person`, `approval`); use those names in `audit::Entry.subject_type` so entries link to the right portal page.
- **Sign-in is bound to the browser that started it**: `/api/auth/login` also sets a ten-minute cookie (`__Host-sl_signin` on https, `sl_signin` on http) holding the attempt's `csrf`. The callback requires it: missing gives `error=expired`, a different value gives `error=failed`. Without it, someone could finish their own sign-in in another person's browser.
- **Admins from the Keycloak group**: when a token's `groups` claim no longer contains `PROJECT_ADMIN_GROUP`, an admin assignment with source `keycloak_group` is removed, unless it is the last admin. Tokens without a `groups` claim change nothing. Assignments added in the portal are never touched.
- **Accounts**: a Keycloak account whose Andrew ID already belongs to a user linked to a different `sub` is refused (`error=failed`) rather than taking that user over.
- **Dev login** also needs `SEED_DEMO_CONTENT=1`, since it signs in only as seeded personas; `GET /api/auth/config` reports `devLogin: false` otherwise.
- **People**: a person has at most one role per committee (adding a second one in the same committee is `409`; change the existing one). `POST /api/people` answers `201` with the person's `PersonRow`; `PATCH /api/people/assignments/:id` answers `200` with it. Removing or demoting a lead removes their `project_editors` rows in that committee.
- **Governance sync** also removes director assignments with source `governance` for a committee whose listed director changed, reports each in `messages`, and counts it under `updated`.
- **Media**: the stored widths are 480, 960 and 1920 below the original, plus the original capped at 1920 (a 1200 px image gives 480, 960 and 1200). `media.width` and `height` are the widest stored variant. Files over 5 MB are `413`; the request body limit stays 6 MB.
- **Static serving**: `/projects/<slug>` without the slash redirects (308) to `/projects/<slug>/`, and a renamed slug in `project_slug_redirects` redirects (308) to the current one. HTML and other non-immutable files carry a content `ETag` and no `Last-Modified`, because the Nix store dates every file 1970. API responses that set no `Cache-Control` get `no-store`. Every response also carries `Content-Security-Policy: frame-ancestors 'none'; base-uri 'self'; object-src 'none'; form-action 'self'`, plus the Keycloak origin in `form-action` when sign-in is configured. `script-src` is not restricted yet.
- **`Cargo.toml`**: `zune-jpeg` (the JPEG decoder) is built with LLVM in dev builds. Cranelift cannot compile its NEON code yet, and decoding a JPEG aborted the process.

### 2026-11-24, scaffold

- 2026-11-24: `ProjectKind` gains `program` and `fundraiser`, which the approved designs use for the Speaker series and Giving Day.
