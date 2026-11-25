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
| `APP_URL` | kennel; the dev shell sets `http://localhost:5173` | yes |
| `PORT` | kennel. Binds `0.0.0.0:$PORT` | in deploys |
| `SITE_BIND_ADDR` | dev shell, `127.0.0.1:8080` | local |
| `STATIC_DIR` | flake `env`. Unset locally, where Vite serves pages | in deploys |
| `S3_ENDPOINT`, `S3_REGION`, `S3_ACCESS_KEY`, `S3_SECRET_KEY`, `S3_BUCKET` | kennel `garage` | optional. Without them media is stored in Postgres |
| `OIDC_CLIENT_ID`, `OIDC_CLIENT_SECRET`, `KEYCLOAK_URL`, `KEYCLOAK_REALM`, `OAUTH_RELAY_URL`, `PROJECT_GROUP`, `PROJECT_ADMIN_GROUP` | governance `oidc_client` | optional. Sign-in through Keycloak is on only when all seven are set |
| `PORTAL_DEV_LOGIN` | dev shell sets `1` | optional |
| `PORTAL_DEV_LOGIN_KEY` | OpenBao, preview profile only | optional |
| `SEED_DEMO_CONTENT` | dev shell and preview profile | optional |
| `RUST_LOG` | anywhere | optional |

Dev login rules, enforced in the service:

1. Off unless `PORTAL_DEV_LOGIN=1`.
1. Always off when the `APP_URL` host is `scottylabs.org` or `www.scottylabs.org`.
1. When `APP_URL` is not `localhost` or `127.0.0.1`, `PORTAL_DEV_LOGIN_KEY` must be set and the request must send the same key. Compare in constant time.
1. Only the seeded personas below can be used.

## Seed data

Seeding runs at startup inside one transaction and only inserts what is missing, so it is safe on every restart and never overwrites edits.

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
| `anishp` | Anish Pallati | director, tech |
| `spchoksh` | Sara Chokshi | lead, events, can post to the ticker, edits Nova |
| `newmember` | New Member | no role |

Bundled seed images live in `sites/web/static/seed/` and are referenced as `/seed/...`.

## Conventions

### Rust

- Edition 2024, nightly toolchain from kennel. Format with `cargo fmt`. `cargo clippy` clean.
- `sqlx` with runtime-checked queries (`query`, `query_as`, `query_scalar`). Do not use the `query!` macros, which need a database at compile time. `sqlx::migrate!` is fine.
- IDs are `Uuid::now_v7()`.
- Handlers return `Result<T, AppError>` (`src/error.rs`). Errors serialize as `{ "error": { "code", "message", "fields"? } }` with the matching status: `bad_request` 400, `validation_failed` 422, `unauthenticated` 401, `forbidden` 403, `not_found` 404, `conflict` 409, `payload_too_large` 413, `unsupported_media_type` 415, `rate_limited` 429, `auth_not_configured` 503, `internal` 500. Internal errors log the cause and never return it.
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
  - `state` is base64url (no padding) of `{"return_to": "<APP_URL>/api/auth/callback", "csrf": "<csrf>"}`. Ricochet reads `return_to` and forwards the browser there with `code` and `state` unchanged.
  - `redirect_uri` is `OAUTH_RELAY_URL`. Scopes: `openid email profile`.
  - Redirect (302) to Keycloak's authorization endpoint from discovery at `<KEYCLOAK_URL>/realms/<KEYCLOAK_REALM>`.
- `GET /api/auth/callback?code&state` (or `?error=...`)
  - Decode `state`, load and delete the attempt by `csrf`. Reject attempts older than 10 minutes.
  - Exchange the code with the verifier and `redirect_uri = OAUTH_RELAY_URL`. Verify the ID token (signature, issuer, audience, expiry, nonce).
  - Read `sub`, `preferred_username` (the Andrew ID), `name`, `email` and the `groups` claim.
  - Upsert the user: match `keycloak_sub`, else an existing row with that `andrew_id` (people can be added before they first sign in), else insert.
  - If `groups` contains `PROJECT_ADMIN_GROUP`, ensure an `admin` assignment with source `keycloak_group`.
  - Create a session and redirect to the attempt's `returnTo`. On any failure redirect to `/portal/sign-in/?error=<code>` with codes `cancelled`, `expired`, `failed`.

### Sessions

- Cookie `__Host-sl_session` when `APP_URL` is https, otherwise `sl_session`. `HttpOnly`, `Path=/`, `SameSite=Lax`, `Secure` on https. Value is 32 random bytes, base64url. The database stores only its SHA-256.
- Expires after 12 hours without a request, and 7 days after creation at most. Refresh `last_seen_at` at most once a minute.
- `POST /api/auth/logout` deletes the session and clears the cookie (204).

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
| Sync directors from governance | yes | no | no |

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
| GET `/api/public/projects` | `PublicProjectsResponse`: published, featured first, then by name |
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
| GET `/api/announcements?tab=live\|waiting\|expired` | portal | `waiting` holds pending and sent back. Leads see their own plus live ones. `expired` keeps 90 days |
| POST `/api/announcements` | may post | Live at once for admins and directors. For leads, creates a `pending_approval` announcement and an approval |
| PATCH `/api/announcements/:id` | may edit | Partial `AnnouncementInput`. A lead editing a sent back one resubmits it |
| POST `/api/announcements/:id/extend` | may manage | Body `{ "days": 7 }`. Never beyond 30 days from now |
| POST `/api/announcements/:id/end` | may manage | Sets `state = ended`, `ended_at = now` |
| POST `/api/announcements/:id/repost` | may post from its source | Body `{ "expiresAt" }`. New announcement copied from an expired or ended one |
| PUT `/api/announcements/order` | reorder | Body `{ "ids": [...] }`, the live ones in their new order |
| DELETE `/api/announcements/:id` | creator of a pending or sent back one, or manager | 204 |

Rules: message 1 to 90 characters on one line, trimmed. `linkUrl` optional, `https` only, at most 2048 characters. `expiresAt` in the future and at most 30 days after it goes live. At most 6 live at once; posting or approving a seventh fails with `conflict` and says to end one first. Approving keeps the lead's chosen `expiresAt`; if it has passed, approval fails with `validation_failed` and asks the lead to pick a new time.

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

Draft validation (saving): block types and widths per `BLOCK_WIDTHS`, the cover first and only once, at most 12 blocks, unique block ids, stats 2 to 4 items, gallery 3 to 6 images, text fields within limits, URLs `https` (or `mailto:` for buttons), images only `/media/<uuid>` or `/seed/...`, video URLs that play one YouTube or Vimeo video, rich text sanitized with `ammonia`. Publishing additionally requires alt text on every image.

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

Rules: name 1 to 60 characters; summary 1 to 48; description up to 4000 characters of text after sanitizing; links `https`; `repoUrl` any https host; `videoUrl` YouTube or Vimeo; at most 8 media; `builtWith` excludes the owner. Publishing needs a cover with alt text and a summary.

### Media

`POST /api/media`, multipart with `file` and optional `alt`, any portal user.

- At most 5 MB. JPEG, PNG or WebP, detected from the bytes, not the name or header.
- Decode with limits (at most 12000 by 12000 pixels and 512 MB of allocation).
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
| POST `/api/people/sync-governance` | admins. Reads `data/committees.yaml` from the governance repository and makes each listed director a director of their committee |

### Overview and activity

| Method and path | Who |
| -- | -- |
| GET `/api/overview` | portal, scoped to the viewer |
| GET `/api/activity?limit=` | portal, scoped: admins see all; others see entries for their committees and their own actions |

Attention items, in this order: approvals waiting for the viewer; the viewer's own sent back items; announcements the viewer can manage that expire within 3 days; committee pages they can edit with unpublished changes; projects they can edit missing a cover, summary or links; pages they can edit not published for 90 days.

## Static serving

For a request outside `/api` and `/media`, with `STATIC_DIR` set:

1. Reject paths with `..` or NUL. Decode percent-encoding once.
1. Serve the file if it exists. A directory with `index.html` and no trailing slash redirects (308) to the slash form.
1. Otherwise serve `200.html`:
   - `/projects/<slug>/`: inject the project's name, summary and cover as `<title>`, the meta description and Open Graph tags. Status 404 when no published project has that slug.
   - `/portal/` and below: status 200, `Cache-Control: no-store`.
   - Anything else: status 404 (SvelteKit renders its error page).

Headers on every response: `X-Content-Type-Options: nosniff`, `Referrer-Policy: strict-origin-when-cross-origin`, `X-Frame-Options: DENY`, `Permissions-Policy: camera=(), microphone=(), geolocation=()`, and HSTS when `APP_URL` is https. `/_app/immutable/*`: `public, max-age=31536000, immutable`. HTML: `no-cache`. Other static files: `public, max-age=3600`.

## Limits on requests

- JSON bodies at most 256 KB, uploads 6 MB.
- Per IP: sign-in and dev login 20 a minute; writes 120 a minute; uploads 30 a minute.

## Ownership of files during the build

Each builder owns the files of their track. Edit shared files only where this table allows.

| Track | Owns |
| -- | -- |
| Scaffold | `flake.nix`, `Cargo.toml`, `secretspec.toml`, `services/site/src/{main,state,error,db,config}.rs`, `services/site/src/routes/mod.rs`, migration `0001`, `sites/web/src/lib/api/types.ts`, this file |
| Platform | `auth/`, `permissions.rs`, `audit.rs`, `ratelimit.rs`, `static_files.rs`, `media/`, `routes/{auth,me,people,media,activity}.rs`, `seed/{mod,people}.rs`; web: `src/lib/api/client.ts`, `src/lib/portal/`, `src/routes/portal/{+layout*,+page*,sign-in,no-access}`, the `(site)` group move, header nav |
| Ticker | `routes/announcements.rs`, `seed/ticker.rs`, `migrations/01*`; web: `src/lib/ticker/`, `src/routes/portal/ticker/`, the home hero strip |
| Projects | `routes/projects.rs`, `seed/projects.rs`, `migrations/02*`; web: `src/lib/projects/`, `src/routes/(site)/projects/`, `src/routes/portal/projects/` |
| Committees | `routes/committee_pages.rs`, `blocks.rs`, `richtext.rs`, `seed/committees.rs`, `migrations/03*`; web: `src/lib/committees/`, `src/routes/(site)/committees/`, `src/routes/portal/committees/` |
| Overview | `routes/{overview,approvals}.rs`, `migrations/04*`; web: `src/routes/portal/overview` pieces, `src/routes/portal/people/`, `src/lib/portal/approvals/` |

## Changes

Record changes to this contract here, newest first, with the reason.

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

- 2026-11-24, shared web components. Additive only. `Field` gives its label an id and `FieldContext` gains `labelId`, so controls that a `<label for>` cannot name (the contenteditable in `RichTextEditor`) use `aria-labelledby`. New shared pieces for the projects, committees and people tracks: `src/lib/editor/` (`RichTextEditor`, `RichText`, `sanitize`, `normalizeRichText`, `plainText`, `plainTextLength`; the client mirror of `richtext.rs`), `src/lib/media/` (`Img`, `ImageField`, `ImageListField`, `src`, `srcset`, `aspectRatio`, `uploadImage`), `PersonPicker` in the portal UI kit, `src/lib/projects/ProjectCard.svelte` (also exports `PROJECT_STATUS_LABELS`, `PROJECT_KIND_LABELS` and `projectPath`) and `src/lib/site/SelectionBox.svelte`. The projects track owns `src/lib/projects/` and may change the card; keep its props.

- 2026-11-24, platform web. `sites/web/vite.config.ts` proxies to `SITE_SERVICE_URL` when it is set (default `http://127.0.0.1:8080`), so builds can run side by side on other ports. Prerendering warns instead of failing when `/projects/` or `/committees/` answer 404, because the header links to them before those routes exist; remove that handler once both routes are in.

- 2026-11-24, platform web. How portal pages plug in: call the API with the helpers in `src/lib/api/client.ts`, whose paths are relative to `/api` (`get<MeResponse>("/me")`); read the signed-in person with `getPortalSession()` and show confirmations with `getToasts()` (both from `src/lib/portal/index.ts`); build pages from `src/lib/portal/ui/index.ts` and the `p-table` classes. Render the page's `<h1>` straight away (`PageHeader` does), since the shell moves focus to it after navigation. Unknown `/portal/` addresses get the portal error card from the root `+error.svelte`.

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
