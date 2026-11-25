# ScottyLabs site content platform

Requirements, user flows and implementation plan for the Projects page, the Committees pages, the home page ticker, and the portal that manages all three.

|  |  |
| -- | -- |
| Status | Draft for review |
| Date | 2026-11-24 |
| Owner | Thomas Kanz (Tech UIA) |
| Repo | `ScottyLabs/scottylabs-org` on git.cmu.dev |
| Design source | Figma file "Projects and Committees Pages" (key `8EbIg2lNqqNl9nhAxk8k3Q`) |

Design pages in the Figma file, each wired as a clickable prototype:

| Area | Figma page | Start frame |
| -- | -- | -- |
| Projects | Option C - cards + detail pages | [C v2 index](https://www.figma.com/design/8EbIg2lNqqNl9nhAxk8k3Q/Projects-Page?node-id=50-8319) |
| Committees | Committees - index, pages + editor | [Committees index](https://www.figma.com/design/8EbIg2lNqqNl9nhAxk8k3Q/Projects-Page?node-id=75-4160) |
| Ticker | Home ticker - banner + editor | [Home with ticker](https://www.figma.com/design/8EbIg2lNqqNl9nhAxk8k3Q/Projects-Page?node-id=93-6279) |
| Portal | Portal - sign in + manage | [Portal overview](https://www.figma.com/design/8EbIg2lNqqNl9nhAxk8k3Q/Projects-Page?node-id=106-4378) |

Each Figma page has a "Read me" frame with the rules for that area. Where this document and Figma disagree, this document wins and the Figma should be corrected.

## Contents

1. Summary
1. Goals and non-goals
1. Decisions
1. People and roles
1. Sign-in with OIDC: what exists and how to turn it on
1. Architecture
1. Data model
1. Requirements: public site
1. Requirements: portal
1. User flows
1. API
1. Limits and validation rules
1. Quality requirements
1. Implementation plan
1. Test plan
1. Open questions

## 1. Summary

scottylabs.org is a static SvelteKit site today. Content lives in the repository, so every change needs a pull request. This project adds four things:

1. **Projects page.** A searchable showcase of work from all seven committees, with a page per project (pictures, description, repository and site links, video).
1. **Committees pages.** An index of the seven committees and one page per committee, built from a fixed set of blocks that the committee's Director arranges.
1. **Home page ticker.** A slow scrolling strip of announcements under the home hero, with the ScottyLabs dog mark between items. Each announcement expires on its own.
1. **Portal.** One place, behind ScottyLabs single sign-on, where admins, Directors and leads manage the three areas above.

## 2. Goals and non-goals

### Goals

- Committee members can publish and update site content without a pull request.
- Every public page matches the existing site style. Editors choose content and arrangement, never fonts, colors or spacing.
- Access follows the organization: exec administers, Directors own their committee, leads own what they are given.
- Everything works on a phone, including posting an announcement and editing a project.
- Sign-in uses the Keycloak instance and tooling ScottyLabs already runs. No new accounts or passwords.

### Non-goals

- A general page builder. Only committee pages are block-built, and only from the listed blocks.
- Editing the home hero, Team page or Sponsors page from the portal.
- Comments, likes or any visitor accounts on the public site.
- Rich text beyond bold, italic, links and lists.
- Scheduling a future start time for announcements (the designs only cover expiry). Listed under open questions.

## 3. Decisions

### Confirmed by Thomas (2026-11-24)

| Topic | Decision |
| -- | -- |
| Portal navigation | Top bar with Overview, Ticker, Committees, Projects, People |
| Roles | Admin, Director, Lead, Member |
| Ticker access | Leads may write ticker announcements |
| Publishing | Admins and Directors publish directly. Anything a Lead publishes waits for a Director's approval |
| Appointing leads | Directors appoint leads for their own committee. Admins appoint anyone, and only admins appoint Directors |
| Mobile | Sign-in and all composers work on phones |

### Also confirmed (2026-11-24)

These started as assumptions in the first draft. Thomas confirmed all six.

| # | Decision | Alternative that was not chosen |
| -- | -- | -- |
| A1 | The Projects page follows prototype C v2 (card grid plus a detail page per project) | Prototype A (dense tiles) or B (one long page, no detail pages, no portal) |
| A2 | Committees pages live on the main site at `/committees` and `/committees/<slug>` with a nav item | A standalone page linked from the main site, as discussed at the August exec meeting |
| A3 | One committee owns each project; other committees can be credited under "Built with" | A project can belong to several committees equally |
| A4 | Ticker limits: 6 live at once, 90 characters, expiry required, at most 30 days out | Different numbers |
| A5 | Committee page limit: 12 blocks | Different number |
| A6 | Type on the built site is Satoshi, as today. Figma uses Inter only because Satoshi is not installed there | Switch the site to Inter |

## 4. People and roles

### Roles

| Role | Who | Can do |
| -- | -- | -- |
| Admin | Exec board | Everything in every section, plus People and roles |
| Director | The Director of a committee | Edit and publish their committee page. Create, edit and publish their committee's projects. Post to the ticker. Appoint leads for their committee. Approve or send back what their leads submit |
| Lead | Appointed by a Director or admin | Edit the projects they are added to. Write ticker announcements if allowed. Everything they publish needs approval |
| Member | Anyone with a ScottyLabs account who has no role | Sign in and see the "nothing to edit yet" screen |

### Permission matrix

| Action | Admin | Director | Lead | Member |
| -- | -- | -- | -- | -- |
| Sign in | yes | yes | yes | yes |
| View portal overview | yes | own scope | own scope | no |
| Ticker: write | yes | yes | if permitted, needs approval | no |
| Ticker: publish, extend, end | yes | own committee's | no | no |
| Committee page: edit and publish | any | own | no | no |
| Project: create | any committee | own committee | no | no |
| Project: edit | any | own committee's | assigned projects, needs approval to publish | no |
| Approve or send back a submission | any | own committee's leads | no | no |
| Appoint or remove a lead | any committee | own committee | no | no |
| Appoint or remove a Director or admin | yes | no | no | no |

### Where roles come from

Built (2026-11-24). Roles are stored in the portal's database (`role_assignments`, section 7), keyed by the user's Keycloak subject and Andrew ID, and that table is the only thing permissions read. Three sources write to it:

| Source | Gives | When |
| -- | -- | -- |
| `PROJECT_ADMIN_GROUP` (`/projects/scottylabs-org/admins`) in the token's `groups` claim | Admin | At every sign-in. Governance fills the group with the UIA team leads and, because governance adds the Tech director to every team's leads, the Tech director. A token whose `groups` no longer has the group (an empty list counts) takes that admin role away again, unless it is the last admin. A token without the claim changes nothing |
| Governance `data/committees.yaml` | Director for each committee's `director`; Lead for each head (`roles.*.members` and `teams.*.roles.*.members`) | At startup, every six hours, and when an admin presses Sync on People. A head who already has a role in the committee is left alone. A lead added by an earlier sync goes away when governance stops listing them, unless someone edited it on People, which makes it the portal's; the sync then says once that governance no longer lists them. A role removed or changed on People is not given back (see below) |
| The People page | Any role an admin or Director may give | When they change it |

Notes:

- Heads become leads who can write ticker announcements (which still need their Director's approval) and edit no projects until a Director adds them.
- People has the last word. When a Director or admin removes someone's director or lead role there, moves it to another committee, or changes a director to a lead, the sync gives that person no role in that committee again for as long as governance keeps listing them there, and each sync says so. Once governance stops listing them there, a later listing counts again. To give the role back sooner, add it on People.
- Tech is left out of the head sync: `committees.yaml` lists no Tech heads, because Tech's leads are the team leads in governance's team files. Tech leads are added on People.
- The organization director in `committees.yaml` is never made an admin. Other exec members who should be admins are added on People.
- Committee roles are not in the Keycloak token. Governance creates Keycloak groups only for tech teams and projects, so the token says who someone is and whether they lead UIA (or direct Tech), not who directs or heads a committee.
- The portal does not read Grouper. Grouper's committee groups are generated from the same `committees.yaml` and sync only to Google Groups.
- Later, if exec wants committee roles visible to other ScottyLabs tools, governance can create `/committees/<slug>/director` and `/committees/<slug>/heads` groups in Keycloak, and the portal can read them from the token the way it reads the admin group. That is a governance change and is not required for launch.

## 5. Sign-in with OIDC: what exists and how to turn it on

This section records what was verified in the kennel, governance and Ricochet sources and against the live Keycloak on 2026-11-24, and what was built.

### What is already there

- **Identity provider:** Keycloak at `idp.scottylabs.org`, realm `scottylabs`.

- **The switch is in governance, not in this repository.** Each repository lists its features in its team file. `scottylabs-org` is registered in `data/teams/uia.yaml` under the "ScottyLabs Website" project with these features before this project:

  ```yaml
  - name: scottylabs-org
    features:
      kennel: {}
      posthog: {}
      cdn: {}
  ```

- **The feature is `oidc_client`, now enabled (step 1).** Governance:

  - creates three Keycloak clients: `scottylabs-org` for prod, `scottylabs-org-staging` shared by staging and previews, and `scottylabs-org-dev` for local development;
  - writes these secrets to OpenBao for each profile: `OIDC_CLIENT_ID`, `OIDC_CLIENT_SECRET`, `KEYCLOAK_URL`, `KEYCLOAK_REALM`, `OAUTH_RELAY_URL`, `PROJECT_GROUP`, `PROJECT_ADMIN_GROUP`;
  - adds the user's group memberships to the token as a `groups` claim in full-path form, for example `/projects/scottylabs-org`. The claim is in the ID token, the access token and userinfo, and lists every realm group the user is in directly.

- **Tokens.** Keycloak signs ID tokens with RS256 and names the key with `kid`; its key set also holds an RSA encryption key, which verifiers skip. The ID token's audience is the client id alone. Keycloak adds `iss` to every redirect (RFC 9207). The realm also offers a `microprofile-jwt` scope whose mapper writes realm roles into a claim also named `groups`, so the portal asks only for `openid email profile`.

- **Redirects go through a relay called Ricochet.** Prod, staging and previews share `https://oauth.scottylabs.org/oauth2/callback`. Local development uses `http://localhost:8090/oauth2/callback`, which kennel runs for you when the flake option below is on.

- **PKCE is mandatory.** Every client requires `code_challenge` with method `S256`. Libraries such as `openid-client` and `axum-oidc` do this already.

- **`cdn` is already enabled** for this repository. It provides a public-read Garage (S3-compatible) bucket and writes `CDN_S3_ENDPOINT`, `CDN_S3_BUCKET`, `CDN_ACCESS_KEY_ID`, `CDN_SECRET_ACCESS_KEY` and `CDN_PUBLIC_URL` on the prod profile. That is where uploaded pictures go.

### Step 1: enable the feature in governance

Done. [Governance pull request #611](https://git.cmu.dev/ScottyLabs/governance/pulls/611) is merged and applied: the three clients exist in Keycloak and the secrets are in OpenBao. It changed `data/teams/uia.yaml` to:

```yaml
- name: scottylabs-org
  features:
    kennel: {}
    posthog: {}
    cdn: {}
    oidc_client: {}
```

Options on `oidc_client` (from the governance schema):

| Option | Effect | Recommendation |
| -- | -- | -- |
| `admin: true` | Also creates a service-account client that can view and manage users across the realm, written as `KEYCLOAK_ADMIN_CLIENT_ID` and `KEYCLOAK_ADMIN_CLIENT_SECRET` | Not at launch. No other project uses it, and it grants more than the portal needs. Add it later only if looking people up in Keycloak proves necessary |
| `groups: [...]` | Creates empty groups under `/projects/scottylabs-org/` for app-specific roles | Not needed if roles live in the portal database |
| `public: true` | Public clients with no secret, for SPAs and native apps | Do not use. The portal has a server and can keep a secret |
| `redirect_uris`, `audiences` | Overrides for native clients and extra token audiences | Not needed |

The team file is owned by the UIA leads through CODEOWNERS, so the pull request can be reviewed within the team. Once it merges and governance applies it, the secrets appear in OpenBao for every profile.

### Step 2: declare the secrets in this repository

Done. `secretspec.toml` declares the seven variables in `[profiles.default]` with `required = false`, so a developer without an OpenBao login still gets a working shell with dev sign-in. Sign-in through Keycloak turns on only when all seven are present.

Do not add `APP_URL`. Kennel sets it for deployed services and the flake option sets it locally.

### Step 3: turn on the local relay and a database in `flake.nix`

Done. The settings that matter for sign-in:

```nix
scottylabs = {
  project.name = "scottylabs-org";
  ricochet = {
    enable = true;
    appUrl = "http://localhost:5173";
  };
  postgres.enable = true;
  # deno settings unchanged
};
```

`scottylabs.ricochet.enable` only affects local development: it runs the relay as the pitchfork process `ricochet` on `127.0.0.1:8090` and exports `APP_URL`. Governance already seeds the dev profile's `OAUTH_RELAY_URL` to match.

Kennel starts pitchfork processes without the dev shell's secrets, so the `site` process loads the dev profile's secrets itself with `secretspec export`, the way the shell does, before `cargo run -p site`. Without an OpenBao token it starts anyway, with dev sign-in only. Log in once per machine with `nix run git+https://git.cmu.dev/ScottyLabs/kennel#login`.

Open the portal at `http://localhost:5173`, not `http://127.0.0.1:5173`. Sign-in comes back to `APP_URL`, and the sign-in cookie from another host name would not be sent, so the sign-in would end as expired.

### Step 4: add a backend service

Done. An authorization-code flow with a client secret needs a server, so the site is now the kennel service `site`. See section 6.

### The Ricochet contract

Verified in Ricochet's source (the version kennel pins) and against `oauth.scottylabs.org`:

- Ricochet reads only `state`. It must be base64url without padding of a JSON object with `return_to`, the absolute URL to send the browser to; other fields, such as the portal's `csrf`, are ignored. Anything else gets a 400 and no redirect.
- It forwards the browser (302) to `return_to` with every provider parameter unchanged and in order: `code`, `state`, `iss`, `session_state`, `error`, `error_description` and any others. It does not touch PKCE, cookies or the code. Only `GET` (query response mode) is handled.
- `return_to` must be https on an allowed host. The production relay allows `scottylabs.org` and every `*.scottylabs.net` deployment host (previews and branches); it refuses `www.scottylabs.org`. The development relay runs with `RICOCHET_DEV=1`, which also allows http to a loopback host.
- The portal's callback is `<APP_URL>/api/auth/callback`.

### Sign-in sequence

1. The user opens the portal and chooses "Continue with ScottyLabs".
1. The service creates a PKCE (S256) verifier, a nonce and a random `csrf`, and stores them with the page to return to in the `auth_attempts` table for ten minutes. It sets a ten-minute `HttpOnly`, `SameSite=Lax` cookie holding the `csrf` (`__Host-sl_signin` on https, `sl_signin` on http), which ties the sign-in to this browser. It redirects to Keycloak's authorization endpoint, found through `${KEYCLOAK_URL}/realms/${KEYCLOAK_REALM}/.well-known/openid-configuration`, with `redirect_uri = OAUTH_RELAY_URL`, scopes `openid email profile`, and `state` as described above.
1. Keycloak authenticates the user, through the CMU identity provider. People already signed in see the account chooser only.
1. Keycloak redirects to the relay, which forwards the browser back to the service at `APP_URL` with Keycloak's parameters.
1. The service checks `iss` when present, requires the sign-in cookie to match the `state`, uses up the stored attempt, exchanges the code for tokens, validates the ID token (RS256 signature, issuer, audience, expiry, nonce), and reads `sub`, name, email, preferred username and `groups`.
1. The service upserts the user, applies the admin group (section 4), creates a session and sets an `HttpOnly`, `SameSite=Lax` session cookie, `Secure` on https.
1. Users with no role land on the no-access screen. Everyone else lands on the page they asked for, or Overview. A failure lands on the sign-in page with `cancelled`, `expired` or `failed`, and the service logs why.

### Still open

- Which profiles the `cdn` secrets are written to. The docs say prod only, so previews and local development use the local `garage` resource, or Postgres when no bucket is configured.
- Whether sign-out should also end the Keycloak session (RP-initiated logout) or only the portal session. The portal signs out of its own session only.
- That `preferred_username` is the bare Andrew ID can only be confirmed by a real sign-in. If it is not, the callback ends with `failed` and the log says the name is not an Andrew ID.

## 6. Architecture

### Today

- `sites/web`: SvelteKit with Deno, built by `helpers.buildDenoTask` and served by kennel as a static site on `scottylabs.org`.
- No backend, no database. PostHog for analytics.
- The home page announcement is a hard-coded array in `sites/web/src/lib/sections/home/Hero.svelte`.

### Target

Changed during implementation (2026-11-24): one service serves everything on one origin, instead of a static site plus a separate portal service. A static host cannot answer `/projects/<slug>/` for projects created after the build, and a separate API origin would need CORS, a cross-site session cookie and per-preview URL wiring. With one origin each preview is a single working URL.

```
Browser
  |
  +-- scottylabs.org                  kennel service "site" (Rust)
        |-- /                         public pages: the SvelteKit build, prerendered
        |-- /projects/<slug>/         SvelteKit fallback page, with title and meta tags injected
        |-- /portal/...               the portal (same SvelteKit app, client-rendered)
        |-- /api/...                  JSON API, authenticated except /api/public/...
        |-- /media/<id>               uploaded images
        |-- PostgreSQL                content, roles, approvals, audit log
        +-- Garage bucket             uploaded images (kennel garage resource)
```

Components:

| Component | Kennel target | Notes |
| -- | -- | -- |
| Site service | `services.site`, `customDomain = "scottylabs.org"` | Rust (axum). Serves the SvelteKit build from `env.STATIC_DIR`, the API and media. Exposes `GET /api/health`, binds to `PORT`, keeps no state on disk |
| Web app | built with `helpers.buildDenoTask` from `sites/web` | One SvelteKit app: public pages in a `(site)` route group, the portal under `/portal/` |
| Database | `scottylabs.postgres.enable = true` | Kennel provides `DATABASE_URL` locally and per deployment |
| Object storage | `scottylabs.garage.enable = true` | Per-deployment bucket; without one, images are stored in Postgres |

Rust is used because kennel builds Rust services directly and other ScottyLabs services that sign in through Keycloak (for example Tartan.vote) use it, so the relay and token handling follow a working example.

The `implementation contract` (`docs/implementation/contract.md`) pins the API shapes, schema and rules that the code follows.

### How public pages get content

- Public pages load published content from `/api/public/*` (same origin) in the browser. Responses carry `Cache-Control: public, max-age=60, stale-while-revalidate=600`, so a publish shows up within about a minute.
- The ticker endpoint returns only announcements that are live at request time, so expiry needs no rebuild and no cron job.
- Pages render a skeleton first and must not shift layout when content arrives.
- If the API is unreachable, the ticker strip hides itself, and Projects and Committees pages show a short "could not load" message with a retry.

A later improvement is to prerender Projects and Committees pages at build time and trigger a rebuild on publish. It is not needed for launch.

## 7. Data model

All tables have `id` (UUIDv7), `created_at` and `updated_at`. `pg_uuidv7` is available by default in kennel databases.

### users

| Field | Type | Notes |
| -- | -- | -- |
| keycloak_sub | text, unique | Subject from the ID token |
| andrew_id | text, unique | Preferred username |
| name, email | text | From the token, refreshed at each sign-in |
| last_seen_at | timestamptz |  |

### role_assignments

| Field | Type | Notes |
| -- | -- | -- |
| user_id | fk users |  |
| role | enum `admin`, `director`, `lead` | Members have no row |
| committee | enum of the seven committees, nullable | Required for director and lead |
| can_post_ticker | bool | Leads only. Directors and admins always can |
| appointed_by | fk users, nullable | Null when seeded from governance or the admin group |
| source | enum `keycloak_group`, `governance`, `portal` |  |

A lead's editable projects are rows in `project_editors`.

### committees

Seven fixed rows: `events`, `outreach`, `tech`, `labrador`, `design`, `finance`, `foundry`. Fields: `slug`, `name`, `color`, `tint`, `index_description`, `index_cover_media_id`, `meeting_time`, `meeting_room`, `map_url`, `sort_order`.

### committee_pages

| Field | Type | Notes |
| -- | -- | -- |
| committee | enum, unique | One page per committee |
| draft | jsonb | Ordered list of blocks being edited |
| published | jsonb, nullable | Last published list of blocks |
| published_at, published_by |  |  |
| draft_updated_at, draft_updated_by |  | Drives "Unpublished changes" |

A block is `{ id, type, width, props }`. `width` is 4, 6, 8 or 12. `type` and `props`:

| Type | Props | Allowed widths |
| -- | -- | -- |
| cover | tagline, media_id | 12, always first, cannot be removed |
| text | heading, body (restricted rich text) | 4, 6, 8, 12 |
| image | media_id, caption, alt (required) | 4, 6, 8, 12 |
| gallery | 3 to 6 media_ids, layout `mosaic` or `thirds`, caption | 8, 12 |
| video | url (YouTube or Vimeo), title, poster media_id | 6, 8, 12 |
| stats | 2 to 4 items of value and label | 6, 8, 12 |
| teams | heading, intro, list of name, blurb, channel | 8, 12 |
| projects | heading | 12. Filled automatically with the committee's published projects |
| quote | text, attribution | 4, 6, 8, 12 |
| work_sessions | none | 4, 6. Reads time and room from `committees` |
| cta | heading, text, primary label and url, secondary label and url | 12 |

### projects

| Field | Type | Notes |
| -- | -- | -- |
| slug | text, unique | Used in `/projects/<slug>` |
| name | text |  |
| summary | text, max 48 | Shown on cards |
| description | text | Restricted rich text |
| committee | enum | Owner |
| built_with | enum array | Other committees credited |
| status | enum `live`, `in_development`, `annual`, `ongoing`, `archived` | Shown on the card |
| kind | enum `app`, `event`, `tool`, `brand`, `other` | Shown on the card |
| site_url, repo_url, video_url | text, nullable |  |
| state | enum `draft`, `pending_approval`, `published` |  |
| draft | jsonb, nullable | Unpublished edits to an already published project |
| published_at, published_by |  |  |

Related tables: `project_media` (project, media, position, with the first as cover) and `project_editors` (project, user) for leads.

### announcements

| Field | Type | Notes |
| -- | -- | -- |
| kind | enum `event`, `project_update`, `milestone`, `club_news` | Chooses the icon |
| committee | enum or `club` | Shown as the chip |
| message | text, max 90 |  |
| link_url | text, nullable |  |
| starts_at | timestamptz | Set when it goes live |
| expires_at | timestamptz | Required, at most 30 days after `starts_at` |
| ended_at | timestamptz, nullable | Set by "End now" |
| state | enum `pending_approval`, `live`, `sent_back`, `expired`, `ended` | `expired` is derived from `expires_at` |
| position | int | Order in the loop |
| created_by, approved_by | fk users |  |

### approvals

| Field | Type | Notes |
| -- | -- | -- |
| subject_type | enum `announcement`, `project` |  |
| subject_id | uuid |  |
| submitted_by | fk users |  |
| committee | enum | Routes it to the right Director |
| status | enum `waiting`, `approved`, `sent_back` |  |
| decided_by, decided_at, note |  | Note is shown to the submitter when sent back |

### media and audit_log

- `media`: `key`, `url`, `width`, `height`, `bytes`, `content_type`, `alt`, `uploaded_by`.
- `audit_log`: `actor_id`, `action`, `subject_type`, `subject_id`, `summary`, `at`. Feeds "Recent activity" and answers who changed what.

## 8. Requirements: public site

Requirement IDs are stable so issues and pull requests can cite them.

### 8.1 Home page ticker

| ID | Requirement |
| -- | -- |
| T-1 | The ticker replaces the announcement button under the home hero and sits between the same two hairlines |
| T-2 | Live announcements scroll right to left at 40 px per second and loop without a visible jump |
| T-3 | The dog mark, in the brand gradient (`#40D2FC`, `#1F4CE0`, `#FB1A32`), appears between every two announcements |
| T-4 | Each announcement shows a type icon, the message, a committee chip, and an arrow when it has a link. The whole item is the link and opens in a new tab |
| T-5 | Hovering or keyboard-focusing an item pauses the ticker and shows the selection box with the link's host name |
| T-6 | A pause button stops and restarts the motion. The state is remembered for the visit |
| T-7 | An "All N" button opens a list of every live announcement. The list is keyboard reachable and closes on Escape |
| T-8 | With `prefers-reduced-motion`, the ticker does not scroll. It shows one announcement at a time with previous and next buttons and a position such as "2 of 5" |
| T-9 | With one announcement, it is shown static. With none, the strip is not rendered and the page keeps the single hairline |
| T-10 | Screen readers get the announcements as a plain list. The moving track is hidden from assistive technology so it is not read twice |
| T-11 | An announcement stops appearing at its expiry time without a deploy |
| T-12 | On phones the strip keeps the pause button and drops the left label |

### 8.2 Projects index (`/projects`)

| ID | Requirement |
| -- | -- |
| P-1 | Shows published projects as cards in two columns on desktop and one on phones, with one featured project at the top |
| P-2 | A card shows a 4:3 cover, name, description up to four lines, committee chip, status and kind, and icon links to the repository and site when present |
| P-3 | Projects without a cover show the first letter of the name on the committee tint |
| P-4 | Committee tabs filter the list and show counts. "All" is the default. The selected tab is reflected in the URL |
| P-5 | Search matches name, summary, description, committee and kind as you type, and shows a clear empty state with a way to reset |
| P-6 | Hovering a card shows the selection box and the project path. The whole card opens the project page |
| P-7 | Default order is alphabetical by name |

### 8.3 Project page (`/projects/<slug>`)

| ID | Requirement |
| -- | -- |
| P-8 | Shows a large picture with a thumbnail rail, the description beside it, and repository, site and video links below |
| P-9 | Choosing a thumbnail swaps the large picture. Videos play in place from YouTube or Vimeo |
| P-10 | Shows the owning committee and any "Built with" committees, each linking to its committee page |
| P-11 | The "Projects" breadcrumb returns to the index with the previous filter and search intact |
| P-12 | Unpublished and archived projects return 404 to visitors |

### 8.4 Committees index (`/committees`)

| ID | Requirement |
| -- | -- |
| C-1 | Lists the seven committees in the site's order, each with a large picture, description, a sentence naming its teams, and where and when it meets |
| C-2 | Picture and text alternate sides on desktop and stack on phones |
| C-3 | Jump chips at the top scroll to a committee |
| C-4 | Hovering a row shows the selection box and path. The row and its button open the committee page |
| C-5 | A closing section lists Saturday work session rooms with links to CMU Maps |

### 8.5 Committee page (`/committees/<slug>`)

| ID | Requirement |
| -- | -- |
| C-6 | Renders the published blocks in order on a 12-column grid with 24 px gutters. Rows fill left to right and wrap when a block does not fit |
| C-7 | The cover shows the committee name, tagline, Director, join and Slack actions, and the cover picture |
| C-8 | The Projects block lists the committee's published projects using the same card as the Projects page |
| C-9 | The Teams block shows sub-teams with a channel to reach them. It lists teams, not individual roles |
| C-10 | On phones every block is full width, in the same order |
| C-11 | Visitors never see draft content |

## 9. Requirements: portal

### 9.1 Sign-in and session

| ID | Requirement |
| -- | -- |
| S-1 | The portal is reachable only after sign-in through ScottyLabs single sign-on (section 5) |
| S-2 | The sign-in page explains what the portal manages and who can edit |
| S-3 | After the provider returns, a short "checking what you can edit" state resolves the role and moves on by itself |
| S-4 | A signed-in user with no role sees the no-access screen with how to get access and a sign-out button |
| S-5 | Sessions last 12 hours of inactivity and at most 7 days. Expired sessions return to sign-in and then back to the page the user was on |
| S-6 | The account menu shows name, role and sign-out |
| S-7 | Role changes apply at the user's next request, not only at next sign-in |

### 9.2 Shell and Overview

| ID | Requirement |
| -- | -- |
| O-1 | One top bar on every screen: Overview, Ticker, Committees, Projects, People, "View site" and the account menu. On phones the sections move to a bottom tab bar |
| O-2 | Sections the user cannot use are hidden, and items they can see but not edit show a lock and the reason |
| O-3 | Overview shows what is live in each section, scoped to the user's role |
| O-4 | "Needs attention" lists, in this order: approvals waiting for the user, announcements expiring within 3 days, unpublished committee page changes, projects missing a cover or links, and pages not updated in 90 days. Each item has one action |
| O-5 | "Recent activity" shows the last 10 audit entries in the user's scope |

### 9.3 Ticker management

| ID | Requirement |
| -- | -- |
| TK-1 | The list shows a live preview of the ticker, the slots in use, and tabs for Live, Scheduled and Expired |
| TK-2 | Each row shows type, message, link host, committee, time left with a bar, who posted it, and a menu with Edit, Extend by 1 week and End now |
| TK-3 | Rows can be dragged to change the order in the loop |
| TK-4 | The composer has type, committee, message with a live character count, optional link, and expiry |
| TK-5 | Expiry offers Tomorrow, In 3 days, In 1 week, In 2 weeks, and a date and time picker. A sentence states exactly when the announcement will leave |
| TK-6 | The composer shows the announcement at real size between two dog marks |
| TK-7 | When all slots are in use, "New announcement" is disabled with the reason |
| TK-8 | For a Lead, the primary button reads "Send for approval" and names the approver |
| TK-9 | Expired announcements are kept for 90 days and can be posted again, which opens the composer prefilled |
| TK-10 | Editing a live announcement by an admin or Director takes effect immediately |

### 9.4 Committee pages

| ID | Requirement |
| -- | -- |
| CP-1 | The list shows the seven pages with status (Published or Unpublished changes), last edit and Director. Users see "Edit page" only where they may edit |
| CP-2 | The editor opens full screen with the block library and page outline on the left, the live page in the middle and settings on the right |
| CP-3 | Blocks can be added from the library, selected, dragged to a new position, moved up and down, duplicated and removed. The cover cannot be moved or removed |
| CP-4 | Width is chosen from the widths allowed for that block type (section 7). The grid shows free columns as a drop target |
| CP-5 | Text is edited in place. Formatting is limited to bold, italic, links and lists |
| CP-6 | Theme settings (accent color, type, spacing) are shown as locked |
| CP-7 | The draft saves automatically within 2 seconds of a change and shows "Saved" |
| CP-8 | Preview shows the draft as visitors would see it, on desktop and phone widths |
| CP-9 | Publish shows a summary of changes since the last published version and then makes the draft live |
| CP-10 | On phones the editor is a list of blocks that can be reordered and opened to edit. Widths are not editable on phones and the screen says so |
| CP-11 | Images require alt text before the page can be published |
| CP-12 | Undo and redo cover at least the last 20 changes in the session |

### 9.5 Projects

| ID | Requirement |
| -- | -- |
| PR-1 | The list shows cover, name, path, committee, status, which links exist, and last update, with search, committee filters and a Published or Drafts filter |
| PR-2 | The editor has name, committee, one-line summary (48 characters), description, photos and video, live site, repository, and "Built with" |
| PR-3 | Photos can be uploaded, reordered and removed. The first is the cover. Up to 8 photos |
| PR-4 | A live preview shows the card and the page |
| PR-5 | A checklist shows what is missing before publishing: cover, summary length, working links |
| PR-6 | Admins and Directors publish directly. A Lead's publish creates an approval |
| PR-7 | Editing a published project edits a draft. Visitors keep seeing the published version until the draft is published |
| PR-8 | The slug is generated from the name at creation and can be changed only by an admin, with a redirect from the old path |
| PR-9 | Archiving removes a project from the public site without deleting it |

### 9.6 People, leads and approvals

| ID | Requirement |
| -- | -- |
| PE-1 | Admins see everyone with a role, with committee, role and what they can edit, and can add a person, change a role or remove it |
| PE-2 | Directors see their own committee's people and can add or remove leads |
| PE-3 | "Add a lead" takes a person, the projects they can edit, and whether they can write ticker announcements. The person search covers everyone who has signed in to the portal; anyone else is added by typing their Andrew ID. For Directors the role is fixed to Lead in their committee |
| PE-4 | A person who has never signed in can still be added by Andrew ID. The role applies when they first sign in |
| PE-5 | The page explains each role in plain words |
| PE-6 | A user cannot remove their own admin role, and the last admin cannot be removed |
| AP-1 | A Lead's submission creates an approval for the Directors of that committee and for admins |
| AP-2 | The approver sees the item exactly as it will appear, its details, and may add a note |
| AP-3 | "Approve and publish" makes it live at once. Announcements keep the expiry time the Lead chose; if it has already passed, approval is blocked and the Lead is asked to pick a new time |
| AP-4 | "Send back" returns it to the Lead with the note. The Lead can edit and resubmit |
| AP-5 | The Lead sees the status of their submissions: waiting, approved, or sent back with the note |

## 10. User flows

Each flow lists the happy path, then the exceptions the build must handle.

### F1. Visitor reads the ticker

1. Opens scottylabs.org. The ticker scrolls under the hero.
1. Hovers an announcement. The ticker pauses and shows where the link goes.
1. Clicks. The link opens in a new tab.

Exceptions: reduced motion shows the stepper instead. No announcements means no strip. API unreachable means no strip.

### F2. Visitor finds a project

1. Opens Projects. Sees the featured project and the grid.
1. Clicks a committee tab or types in search.
1. Opens a project, looks through the pictures, follows the repository or site link.
1. Uses the breadcrumb to return to the same filtered list.

Exceptions: search with no matches shows the empty state with a reset.

### F3. Visitor explores a committee

1. Opens Committees and reads the seven rows.
1. Opens a committee, reads its page, and follows "Join" or the work session map link.

### F4. Sign in

1. Opens scottylabs.org/portal and chooses "Continue with ScottyLabs".
1. Keycloak shows the account chooser or asks for credentials.
1. The portal shows "Checking what you can edit" and then Overview.

Exceptions: no role leads to the no-access screen. A cancelled or failed sign-in returns to the sign-in page with a plain error. An expired session returns to sign-in and then to the original page.

### F5. Director posts an announcement

1. From Overview or Ticker, chooses "New announcement".
1. Picks the type and committee, writes the message, adds a link.
1. Picks an expiry preset, or opens the picker and sets a date and time.
1. Checks the preview and chooses "Publish announcement".
1. Lands on the list with the new row highlighted and a confirmation. The home page shows it within a minute.

Exceptions: message over 90 characters, expiry in the past or beyond 30 days, an invalid link, and no free slot each block publishing with a message beside the field.

### F6. Lead posts, Director approves

1. The Lead completes the composer and chooses "Send for approval". The item shows as waiting.
1. The Director sees it under "Needs attention" and chooses "Review".
1. The Director reads the preview and chooses "Approve and publish", or writes a note and chooses "Send back".
1. The Lead sees the result. If sent back, they edit and resubmit.

Exceptions: if the slots filled up while waiting, approval is blocked with the reason and the Director can end another announcement first.

### F7. Director edits their committee page

1. Committees, then "Edit page" on their committee.
1. Selects a block and changes its content or width, drags it to a new place, or adds a block from the library.
1. Uses Preview on desktop and phone widths.
1. Chooses Publish, reads the summary of changes, and confirms.
1. Returns to the list with the page marked Published.

Exceptions: an image without alt text blocks publishing. Two people editing the same page: the second sees who is editing and opens read-only until the first leaves or 5 minutes pass without a change.

### F8. Add or edit a project

1. Projects, then "Add project" or "Edit" on a row.
1. Fills in the fields, uploads photos and orders them.
1. Checks the preview and the checklist.
1. Publishes (admin or Director) or sends for approval (Lead).

Exceptions: a duplicate slug is resolved by adding a number. An upload that is too large or the wrong type is rejected with the limit stated.

### F9. Director appoints a lead

1. People, then "Add a lead".
1. Searches for the person, ticks the projects they can edit, and chooses whether they can write ticker announcements.
1. Confirms. The person appears in the list and gets access at their next request.

Exceptions: the person already has a role in another committee: shown with a message, and only an admin can assign a second committee.

### F10. Admin manages roles

1. People and roles, then "Add person" or the role menu on a row.
1. Chooses Lead, Director or Admin, and the committee where needed.
1. Confirms. The change is written to the audit log.

### F11. An announcement expires

1. At `expires_at` the public API stops returning it. No one has to act.
1. Three days earlier it appears under "Needs attention" for its poster and Director with "Extend".
1. Afterwards it is listed under Expired for 90 days with "Post again".

### F12. Sign out

1. Account menu, then "Sign out".
1. The session ends and the sign-in page is shown.

## 11. API

All paths are under `https://scottylabs.org`. Authenticated routes use the session cookie and require a CSRF token on writes. Errors return JSON with `code` and `message`.

This section summarizes the routes. `docs/implementation/contract.md` is the authority for paths, bodies, status codes and error codes; where the two differ, the contract wins.

### Public, cacheable

| Method and path | Returns |
| -- | -- |
| GET `/api/health` | 200 when ready. Required by kennel |
| GET `/api/public/ticker` | Live announcements in order |
| GET `/api/public/projects` | Published projects, with `committee` and `q` filters. The Projects page filters its loaded list in the browser as you type and asks the API with `q` for matches deeper in the descriptions; its `?q=` stays in the page address |
| GET `/api/public/projects/:slug` | One published project |
| GET `/api/public/committees` | The seven committees with index content |
| GET `/api/public/committees/:slug` | Published blocks for one committee page |

### Session

| Method and path | Purpose |
| -- | -- |
| GET `/api/auth/config` | Which sign-in methods are on |
| GET `/api/auth/login?returnTo=` | Starts the OIDC flow |
| GET `/api/auth/callback` | Completes it and sets the session |
| POST `/api/auth/dev-login` | Signs in as a seeded persona where dev sign-in is allowed |
| POST `/api/auth/logout` | Ends the session |
| GET `/api/me` | Current user, role, scope and permissions |

### Portal

| Method and path | Who |
| -- | -- |
| GET `/api/overview` | Any role |
| GET, POST `/api/announcements`; GET, PATCH, DELETE `/api/announcements/:id` | Admin, Director, permitted Lead |
| POST `/api/announcements/:id/extend`, `/end`, `/repost`; PUT `/api/announcements/order` | Admin, Director |
| GET `/api/committee-pages`; GET `/api/committee-pages/:slug` | Any role, read only unless they may edit |
| POST, DELETE `/api/committee-pages/:slug/lock`; PUT `/api/committee-pages/:slug/draft`; GET `/api/committee-pages/:slug/changes` | Admin, that committee's Director |
| POST `/api/committee-pages/:slug/publish`, `/discard` | Admin, that committee's Director |
| GET, POST `/api/projects`; GET `/api/projects/:id` | By scope |
| PUT `/api/projects/:id/draft` | Editors of the project |
| POST `/api/projects/:id/publish`, `/archive`, `/unarchive`, `/discard` | Admin, Director; a Lead's publish creates an approval |
| PATCH `/api/projects/:id/settings`; PUT `/api/projects/:id/editors` | Admin (settings); Admin and the owning Director (editors) |
| POST `/api/media` (multipart) | Any role |
| GET `/api/approvals`; POST `/api/approvals/:id/approve`, `/send-back` | Admin, Director; Leads list their own with `mine=true` |
| GET, POST `/api/people`; GET `/api/people/search?q=` | Admin; Director for own leads. Search reads the portal's own `users` table |
| PATCH, DELETE `/api/people/assignments/:id` | Admin; Director for own leads |
| POST `/api/people/sync-governance` | Admin |
| GET `/api/activity` | By scope |

Authorization is enforced in the service on every route. The interface hiding a button is never the only check.

## 12. Limits and validation rules

| Item | Rule |
| -- | -- |
| Announcement message | 1 to 90 characters, one line, plain text |
| Announcement link | Optional, `https` only |
| Announcement expiry | Required, in the future, at most 30 days after it goes live |
| Live announcements | At most 6 |
| Expired announcements kept | 90 days |
| Committee page blocks | At most 12 including the cover |
| Block widths | 4, 6, 8 or 12 columns, restricted per block type |
| Stats | 2 to 4 items |
| Gallery | 3 to 6 photos |
| Project summary | At most 48 characters |
| Project photos | At most 8 |
| Uploads | JPG, PNG or WebP, at most 5 MB, at least 1200 px wide for covers. The service strips metadata and produces 480, 960 and 1920 px versions |
| Alt text | Required for every image on committee pages and for project covers |
| Rich text | Bold, italic, links, bulleted and numbered lists. Everything else is removed on save |
| Video | YouTube or Vimeo URLs only |

## 13. Quality requirements

### Accessibility

- WCAG 2.2 AA. Text contrast at least 4.5 to 1, including text on committee tints.
- Everything is usable by keyboard with visible focus. Drag and drop always has a keyboard or button alternative (move up, move down, drag the order list).
- Moving content can be paused (T-6) and respects reduced motion (T-8).
- Dialogs trap focus, close on Escape and return focus to the control that opened them.
- Form errors are next to the field, say what is wrong and how to fix it, and are announced to screen readers.

### Performance

- Public pages: Largest Contentful Paint under 2.5 seconds on a mid-range phone on 4G. The ticker adds no layout shift.
- Images are served in responsive sizes with `width` and `height` set and lazy loading below the fold.
- The ticker animates with CSS transforms only.
- Portal pages respond to an action within 200 ms or show progress.

### Security

- OIDC authorization-code flow with PKCE `S256`, state and nonce. Tokens stay on the server.
- Session cookies are `HttpOnly`, `Secure`, `SameSite=Lax`. Writes require a CSRF token.
- Role and scope are checked on the server for every request.
- Rich text is sanitized on save and on render. Links are limited to `https` and `mailto`.
- Uploads are checked by content, not file extension, re-encoded, and stored under generated keys.
- Rate limits on sign-in, uploads and writes.
- No secrets in the repository. All come from OpenBao through `secretspec.toml`.
- Public endpoints expose no personal data beyond names already on the Team page.

### Reliability and operations

- The service keeps no state on disk. Kennel's filesystem is read-only apart from `/tmp`.
- Database migrations run at startup and are backward compatible with the previous release.
- Every publish, approval, role change and deletion is written to the audit log.
- Errors are reported with the `sentry` governance feature (to be enabled). Product analytics continue through PostHog.
- The database is backed up on kennel's schedule. Published content can be re-published from the last draft if a publish is reverted.

### Content and voice

- Interface text is sentence case, plain, and names things the way members do.
- A button says what it does, and the confirmation uses the same word: "Publish" then "Published".
- Empty states say what to do next.

## 14. Implementation plan

Each phase ends with something usable. Estimates assume two people part-time and are for ordering, not commitment.

### Phase 0. Foundations (about 1 week)

- Governance: merge pull request #611 (`oidc_client`). Add `sentry` in a second pull request.
- `flake.nix`: enable `ricochet` and `postgres`, add `services.portal`, add `sites/portal` to `deno.dirs`.
- `secretspec.toml`: declare the OIDC and CDN secrets.
- Service skeleton with `/api/health`, configuration loading, migrations and CI checks.
- `flake.nix`: replace the static `sites.web` target with `services.site`, which serves the build.
- Portal app skeleton sharing tokens and components with `sites/web`.

Done when: a preview deployment passes the health check and serves an empty portal page.

### Phase 1. Sign-in and roles (about 1 week)

- OIDC login, callback, session, logout.
- `users` and `role_assignments`, admin bootstrap from `PROJECT_ADMIN_GROUP`, Director import from governance.
- Portal shell: top bar, account menu, no-access screen, role-aware navigation.
- People and roles for admins.

Done when: a UIA lead signs in locally and on a preview and lands on an empty Overview as Admin; a member with no role sees the no-access screen.

### Phase 2. Ticker (about 1.5 weeks)

- `announcements`, portal list and composer with expiry, extend, end, reorder.
- Public endpoint and the ticker component on the home page, including pause, list, reduced motion and phone layout.
- Remove the hard-coded array in `Hero.svelte`.

Done when: T-1 to T-12 and TK-1 to TK-10 pass, and an announcement disappears at its expiry without a deploy.

### Phase 3. Projects (about 2 weeks)

- `projects`, media upload to the Garage bucket, portal list and editor.
- Public index, search, filters and project page.
- Import the current projects from the repository data.

Done when: P-1 to P-12 and PR-1 to PR-9 pass, and every existing project is visible with its pictures.

### Phase 4. Committee pages (about 2.5 weeks)

- `committees`, `committee_pages`, the block renderer shared by the public page and the editor canvas.
- Editor: library, outline, selection, drag, widths, in-place text, preview, publish.
- Public index and committee pages.
- Seed each committee's page with a starting layout for Directors to edit.

Done when: C-1 to C-11 and CP-1 to CP-12 pass, and all seven Directors have published once.

### Phase 5. Leads and approvals (about 1 week)

- Director-scoped People, "Add a lead", project editors.
- Approvals for announcements and projects, with notes.
- Overview "Needs attention" and "Recent activity".

Done when: PE-1 to PE-6 and AP-1 to AP-5 pass.

### Phase 6. Hardening and launch (about 1 week)

- Accessibility audit, performance pass, security review.
- Documentation for Directors (one page) and a short walkthrough at exec.
- Add the Projects and Committees nav items and announce.

### Suggested order of release

Ticker first: it is small, visible, and proves sign-in and publishing end to end. Projects second, Committees third.

## 15. Test plan

| Layer | What |
| -- | -- |
| Unit | Validation rules in section 12, permission checks for every role and action in the matrix, expiry logic, block width rules, rich text sanitizing |
| API | Every route with each role, including forbidden cases. Approval state transitions |
| Component | Ticker (loop, pause, reduced motion, list), project card, block renderer at every width, composer forms |
| End to end | Flows F1 to F12 against a preview deployment with the dev Keycloak client |
| Accessibility | Automated checks in CI plus a manual keyboard and screen reader pass on the ticker, the composers and the page editor |
| Visual | Screenshots of public pages at 390, 768 and 1440 px compared with the Figma frames |

## 16. Open questions

| # | Question | Needed by |
| -- | -- | -- |
| Q2 | Rust or Python for the service | Phase 0 |
| Q3 | Should committee and Director groups be emitted to Keycloak by governance, so other tools can use them too | Before Phase 5, not blocking |
| Q4 | Should announcements support a future start time ("Scheduled" tab) | Phase 2 |
| Q5 | Sign out of the portal only, or of Keycloak as well | Phase 1 |
| Q6 | Who approves when a committee has no Director for a while: any admin | Phase 5 |
| Q7 | Serving scottylabs.org from the site service instead of a static deployment: confirm before merging to main | Before launch |
| Q8 | Retention for the audit log and for archived projects | Phase 6 |
| Q9 | Should leads be able to edit their committee page as well, or does that stay with Directors | Phase 5 |

## References

- Kennel docs: [deploying](https://docs.kennel.scottylabs.org/guides/deploying/), [secrets](https://docs.kennel.scottylabs.org/guides/secrets/), [local development and the OAuth relay](https://docs.kennel.scottylabs.org/guides/local-development/)
- Governance docs: [enabling features, including OIDC](https://docs.governance.scottylabs.org/features/)
- Governance data: `data/teams/uia.yaml` (features for this repository) and `data/committees.yaml` (Directors and heads)
- This repository: `flake.nix`, `secretspec.toml`, `sites/web/src/lib/sections/home/Hero.svelte`, `sites/web/src/lib/data/team.ts`
