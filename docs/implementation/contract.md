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
| `DATABASE_URL`, `PGUSER` | kennel, or the dev shell | yes |
| `APP_URL` | kennel; the dev shell sets `http://localhost:5173` | yes. The service refuses to start without it |
| `PORT` | kennel. Binds `0.0.0.0:$PORT` | in deploys |
| `SITE_BIND_ADDR` | dev shell, `127.0.0.1:8080` | local |
| `STATIC_DIR` | flake `env`. Unset locally, where Vite serves pages | in deploys |
| `S3_ENDPOINT`, `S3_BUCKET`, and the key as `AWS_ACCESS_KEY_ID` and `AWS_SECRET_ACCESS_KEY` | kennel `garage` provisioner in deploys; the dev shell names the key `S3_ACCESS_KEY` and `S3_SECRET_KEY`, which win when both are set. `S3_REGION` defaults to `garage` | optional. Without them media is stored in Postgres; a partial set logs which are missing, and startup logs where uploads go |
| `OIDC_CLIENT_ID`, `OIDC_CLIENT_SECRET`, `KEYCLOAK_URL`, `KEYCLOAK_REALM`, `OAUTH_RELAY_URL`, `PROJECT_GROUP`, `PROJECT_ADMIN_GROUP` | governance `oidc_client` | optional. Sign-in through Keycloak is on only when all seven are set |
| `PORTAL_DEV_LOGIN` | dev shell sets `1`; the preview profile defaults it to `1` | optional |
| `GOVERNANCE_SYNC` | `1` or `0`; declared in `secretspec.toml` so a deployment can turn it off | optional. Unset, the governance sync runs everywhere except local development with open dev sign-in |
| `PORTAL_DEV_LOGIN_KEY` | OpenBao, preview profile only | optional |
| `SEED_DEMO_CONTENT` | dev shell and preview profile | optional |
| `RUST_LOG` | anywhere | optional |
| `PUBLIC_POSTHOG_KEY`, `PUBLIC_POSTHOG_HOST` | governance `posthog` feature, prod profile; kennel serves them to the browser in `/_app/env.js` and passes them to the service | optional. Page views on public pages are sent only when both are set (see Changes, "security audit fixes: client-side security and privacy"); the service then lets public pages connect to the host |

Dev login rules, enforced in the service:

1. Off unless `PORTAL_DEV_LOGIN=1`.
1. Always off when the `APP_URL` host is `scottylabs.org` or `www.scottylabs.org`.
1. When `APP_URL` is not `localhost` or `127.0.0.1`, `PORTAL_DEV_LOGIN_KEY` must be set and the request must send the same key. Compare in constant time.
1. Only the seeded personas below can be used.

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
- Handlers return `Result<T, AppError>` (`src/error.rs`). Errors serialize as `{ "error": { "code", "message", "fields"? } }` with the matching status: `bad_request` 400, `validation_failed` 422, `unauthenticated` 401, `forbidden` 403, `not_found` 404, `conflict` 409, `method_not_allowed` 405, `payload_too_large` 413, `unsupported_media_type` 415, `rate_limited` 429, `upstream_failed` 502, `auth_not_configured` 503, `internal` 500. Internal errors log the cause and never return it.
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
  - `redirect_uri` is `OAUTH_RELAY_URL`. Scopes: exactly `openid email profile`. Do not add `microprofile-jwt`: the realm offers it, and its mapper writes realm roles into a claim also named `groups`, replacing the group paths below.
  - Redirect (302) to Keycloak's authorization endpoint from discovery at `<KEYCLOAK_URL>/realms/<KEYCLOAK_REALM>`.
- `GET /api/auth/callback?code&state` (or `?error=...`)
  - If `iss` is present it must equal `<KEYCLOAK_URL>/realms/<KEYCLOAK_REALM>` exactly (RFC 9207), on errors too; otherwise `failed`. A callback without `iss` is accepted.
  - On `error`, use up the attempt and go to `cancelled` for `access_denied`, `failed` for anything else.
  - Decode `state`, load and delete the attempt by `csrf`. Reject attempts older than 10 minutes.
  - Exchange the code with the verifier and `redirect_uri = OAUTH_RELAY_URL`. Keycloak's `invalid_grant` gives `expired`; other token errors give `failed`. Verify the ID token (signature, issuer, audience, expiry, nonce). Only RS256 is accepted, the algorithm of the realm's signing key, so a token signed with the client secret (HS256, which discovery also lists) is refused. Keys are picked by `kid`; the realm's encryption key (`use: enc`) is skipped.
  - Read `sub`, `preferred_username` (the Andrew ID), `name`, `email` and the `groups` claim.
  - Upsert the user: match `keycloak_sub`, else an existing row with that `andrew_id` (people can be added before they first sign in), else insert.
  - If `groups` contains `PROJECT_ADMIN_GROUP`, ensure an `admin` assignment with source `keycloak_group`.
  - Create a session and redirect to the attempt's `returnTo`. On any failure redirect to `/portal/sign-in/?error=<code>` with codes `cancelled`, `expired`, `failed`.
  - Every unsuccessful callback logs why, never with a code, token or secret: a provider `error` (warn, with `error` and `error_description`), a refused code exchange (warn, with Keycloak's `error` and `error_description`), a missing sign-in cookie or an unknown, used or stale attempt (info), and every other failure (warn). Text taken from the URL is cut to 200 characters.

### Ricochet relay

- Production, staging and previews share `https://oauth.scottylabs.org/oauth2/callback`. Its allowlist accepts `https://scottylabs.org` and every `*.scottylabs.net` deployment host, so `APP_URL` works in prod and on previews. It refuses `www.scottylabs.org` and plain http.
- Development: kennel runs Ricochet as the pitchfork process `ricochet` on `127.0.0.1:8090` with `RICOCHET_DEV=1`, which accepts any http loopback `return_to`. The `scottylabs-org-dev` client allows only `http://localhost:8090/oauth2/callback`.
- Open the portal at `http://localhost:5173` in development, not `http://127.0.0.1:5173`. Sign-in returns to `APP_URL` (`http://localhost:5173`), and the sign-in cookie set on another host is not sent there, so the callback ends with `error=expired`.
- The pitchfork `site` process loads the dev profile's secrets with `secretspec export` before `cargo run`, as the dev shell does, because kennel starts processes without them. Without an OpenBao token it starts anyway, with dev sign-in only. Log in once with `nix run git+https://git.cmu.dev/ScottyLabs/kennel#login`.

### Where roles come from

Authorization reads only `role_assignments`. Three things write to it:

- **Admins from `PROJECT_ADMIN_GROUP`** (`/projects/scottylabs-org/admins`) in the token's `groups` claim, at every sign-in, with source `keycloak_group`. Governance fills that group with the UIA team leads, plus the Tech director because governance adds the Tech director to every team's leads. A `groups` list without the group, including an empty list, removes an admin role that came from the group (never the last admin). A token without the claim changes nothing. `PROJECT_GROUP` grants nothing.
- **Directors and heads from governance's `data/committees.yaml`**, with source `governance`: each committee's `director` becomes its director, and the members of `committees.<c>.roles.*` and `committees.<c>.teams.*.roles.*` become leads of that committee (can post to the ticker, which still needs approval; no projects). Tech is left out of the head sync, since its leads live in governance's team files. The organization director is never made an admin.
  - A head who already has a role in the committee is left alone. A governance lead whom governance stops listing is removed with their projects. Editing a lead on the People page makes it `portal`, so later syncs keep it; when governance stops listing such a lead, the sync says so once in `messages` and in the activity log ("Kept ... as a lead in ...").
  - The People page has the last word. Removing a director or lead role there, moving it to another committee, or changing a director to a lead writes a row to `governance_opt_outs` in the same transaction, and the sync then gives that person no role in that committee (director or lead) while governance still lists them there; each run says so in `messages`. The row is deleted once governance lists the person in that committee neither as director nor as a head. To give the role back, add it on the People page.
- **Everything else in the portal**: admins, directors and leads added on the People page, with source `portal`.

Committee roles are not in the Keycloak token: governance creates Keycloak groups only for tech teams and projects. The portal does not read Grouper, which is generated from the same `committees.yaml` and syncs only to Google Groups.

The governance sync runs from the admin Sync button, at startup (in the background, after seeding, with a one minute limit) and every six hours while the service runs. A failed run logs a warning and leaves roles as they were. `GOVERNANCE_SYNC` turns it on or off; see Environment. Runs by the service itself are audited with no actor.

### Sessions

- Cookie `__Host-sl_session` when `APP_URL` is https, otherwise `sl_session`. `HttpOnly`, `Path=/`, `SameSite=Lax`, `Secure` on https. Value is 32 random bytes, base64url. The database stores only its SHA-256.
- Expires after 12 hours without a request, and 7 days after creation at most. Refresh `last_seen_at` at most once a minute.
- `POST /api/auth/logout` deletes the session and clears the cookie (204). Signing out also clears work kept in the tab, and when the answer is `200` with `{ "endSessionUrl": "https://..." }` the browser goes there to end the Keycloak session too (see Changes, "security audit fixes: client-side security and privacy").
- When a portal call gets a 401, the web sends the person to sign in and back to the page, and editors keep work the service does not have yet in the tab until then (see Changes, "2026-11-25, follow-ups after the final review").

### CSRF and origin checks

Every `POST`, `PUT`, `PATCH` and `DELETE` under `/api/` must:

1. carry `X-CSRF-Token` equal to the session's `csrf_token` (from `GET /api/me`), except `/api/auth/dev-login`, which has no session yet; and
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

### Public (no auth, `Cache-Control: public, max-age=60, stale-while-revalidate=600`)

| Method and path | Response |
| -- | -- |
| GET `/api/health` | `200 ok` (plain text). Checks the database |
| GET `/api/public/ticker` | `PublicTickerResponse`: live, unexpired, ordered by `position` then `starts_at` |
| GET `/api/public/projects?committee=&q=` | `PublicProjectsResponse`: published, featured first, then by name. `committee` keeps one committee's projects. `q` keeps those where every word appears, ignoring case and accents, in the name, summary, whole description, committee name, kind or status (the words on the card); it is trimmed, cut to 100 characters, and control characters are dropped. Blank means no search |
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
| POST `/api/auth/logout` | 204 |
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
- Decode with limits: at most 12000 pixels on each side, 40 megapixels in all (checked from the header before decoding), and 512 MB of allocation.
- Re-encode, which strips metadata: JPEG at quality 82 for opaque images, PNG when the image has transparency. Make widths 480, 960 and 1920, never wider than the original.
- Store under `media/<uuid>/<width>` in S3 when configured, otherwise in `media_blobs`.
- Respond `201 UploadResponse`.

`GET /media/:id?w=`: the smallest stored variant at least `w` wide, or the largest. `Cache-Control: public, max-age=31536000, immutable`, `X-Content-Type-Options: nosniff`.

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
- Per IP: sign-in and dev login 20 a minute; writes 120 a minute; uploads 30 a minute. Behind kennel's proxy (a loopback peer) the client is `CF-Connecting-IP`, then the right-most `X-Forwarded-For`; headers from other peers are ignored.

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
| Sign-in and governance | `auth/`, `routes/auth.rs`, `routes/auth/`, `routes/people/governance*`, `seed/people.rs`, `migrations/05*`, `.gitleaks.toml` |

## Changes

Record changes to this contract here, newest first, with the reason.

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
