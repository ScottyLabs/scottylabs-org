// JSON shapes shared by the site service (services/site) and this app.
// docs/implementation/contract.md describes every endpoint that returns them.
// Keys are camelCase on the wire. Timestamps are RFC 3339 strings in UTC.

export const COMMITTEE_SLUGS = [
  "events",
  "outreach",
  "tech",
  "labrador",
  "design",
  "finance",
  "foundry",
] as const;
export type CommitteeSlug = (typeof COMMITTEE_SLUGS)[number];

/** Who an announcement is from: a committee, or ScottyLabs as a whole. */
export type TickerSource = CommitteeSlug | "club";

/**
 * An image. `src` is either `/media/<uuid>` (uploaded; request a width with
 * `?w=480`, `?w=960` or `?w=1920`) or `/seed/<path>` (bundled with the site).
 */
export interface ImageRef {
  src: string;
  alt: string;
  width: number | null;
  height: number | null;
}

export interface PersonRef {
  id: string;
  andrewId: string;
  name: string;
  initials: string;
}

export interface ApiErrorBody {
  error: {
    code:
      | "bad_request"
      | "validation_failed"
      | "unauthenticated"
      | "forbidden"
      | "not_found"
      | "conflict"
      | "payload_too_large"
      | "unsupported_media_type"
      | "rate_limited"
      | "auth_not_configured"
      | "internal";
    message: string;
    /** Per-field messages for validation_failed, keyed by camelCase field path. */
    fields?: Record<string, string>;
  };
}

// ---------------------------------------------------------------------------
// Ticker

export type AnnouncementKind = "event" | "project_update" | "milestone" | "club_news";

export interface PublicAnnouncement {
  id: string;
  kind: AnnouncementKind;
  committee: TickerSource;
  message: string;
  linkUrl: string | null;
}

/** GET /api/public/ticker */
export interface PublicTickerResponse {
  announcements: PublicAnnouncement[];
}

export type AnnouncementState = "pending_approval" | "live" | "expired" | "ended" | "sent_back";

export interface Announcement {
  id: string;
  kind: AnnouncementKind;
  committee: TickerSource;
  message: string;
  linkUrl: string | null;
  state: AnnouncementState;
  startsAt: string | null;
  expiresAt: string;
  endedAt: string | null;
  position: number;
  createdBy: PersonRef | null;
  approvedBy: PersonRef | null;
  createdAt: string;
  updatedAt: string;
  /** The latest approval for this announcement, if a lead submitted it. */
  approval: ApprovalRef | null;
  canEdit: boolean;
  /** Extend, end and reorder. */
  canManage: boolean;
}

export type AnnouncementTab = "live" | "waiting" | "expired";

/** GET /api/announcements?tab=live|waiting|expired, and PUT /api/announcements/order (the live tab). */
export interface AnnouncementListResponse {
  announcements: Announcement[];
  slots: { used: number; total: number };
  /** How many announcements each tab holds for this person. */
  counts: Record<AnnouncementTab, number>;
  /** For leads: the directors who approve their announcements, per committee they post for. */
  approvers: { committee: CommitteeSlug; people: PersonRef[] }[];
}

/**
 * GET /api/announcements/:id; POST /api/announcements and /:id/repost (201); PATCH /:id,
 * POST /:id/extend and /:id/end (200).
 */
export interface AnnouncementResponse {
  announcement: Announcement;
}

/** POST /api/announcements, and PATCH with every field optional. */
export interface AnnouncementInput {
  kind: AnnouncementKind;
  committee: TickerSource;
  message: string;
  linkUrl: string | null;
  expiresAt: string;
}

// ---------------------------------------------------------------------------
// Committees

export interface CommitteeSettings {
  /** One or two sentences for the Committees index. */
  indexDescription: string;
  /** For example "3 teams: Logistics, TartanHacks and NOVA". */
  teamsLine: string;
  /** For example "Saturdays, 4-6 PM". */
  meetingTime: string;
  /** For example "Tepper 2700, 2701, 2613". */
  meetingRoom: string;
  mapUrl: string | null;
  /** Slack channel name without the leading #. */
  slackChannel: string | null;
}

export type BlockWidth = 4 | 6 | 8 | 12;

export interface LinkButton {
  label: string;
  url: string;
}

export type BlockByType = {
  cover: { tagline: string; image: ImageRef | null };
  /** `body` is sanitized HTML limited to p, strong, em, a, ul, ol, li and br. */
  text: { heading: string; body: string };
  image: { image: ImageRef | null; caption: string };
  gallery: { images: ImageRef[]; layout: "mosaic" | "thirds"; caption: string };
  /** `url` is a YouTube or Vimeo watch URL. */
  video: { url: string; title: string; poster: ImageRef | null };
  stats: { items: { value: string; label: string }[] };
  teams: {
    heading: string;
    intro: string;
    teams: { name: string; about: string; channel: string }[];
  };
  /** Filled with the committee's published projects when rendered. */
  projects: { heading: string };
  quote: { text: string; attribution: string };
  /** Reads meeting time and room from the committee settings. */
  work_sessions: Record<string, never>;
  cta: { heading: string; text: string; primary: LinkButton | null; secondary: LinkButton | null };
};

export type BlockType = keyof BlockByType;

export type Block = {
  [T in BlockType]: { id: string; type: T; width: BlockWidth; props: BlockByType[T] };
}[BlockType];

/** Widths each block type may use. The cover is always first and full width. */
export const BLOCK_WIDTHS: Record<BlockType, BlockWidth[]> = {
  cover: [12],
  text: [4, 6, 8, 12],
  image: [4, 6, 8, 12],
  gallery: [8, 12],
  video: [6, 8, 12],
  stats: [6, 8, 12],
  teams: [8, 12],
  projects: [12],
  quote: [4, 6, 8, 12],
  work_sessions: [4, 6],
  cta: [12],
};

export const MAX_BLOCKS = 12;

export interface CommitteePageDoc {
  blocks: Block[];
  settings: CommitteeSettings;
}

/** A committee as visitors see it, built from its published page. */
export interface CommitteeSummary extends CommitteeSettings {
  slug: CommitteeSlug;
  name: string;
  color: string;
  tint: string;
  tagline: string;
  cover: ImageRef | null;
}

/** GET /api/public/committees */
export interface PublicCommitteesResponse {
  committees: CommitteeSummary[];
}

/** GET /api/public/committees/:slug */
export interface PublicCommitteePageResponse {
  committee: CommitteeSummary;
  blocks: Block[];
  /** Published projects owned by this committee, for the projects block. */
  projects: ProjectCard[];
  publishedAt: string | null;
}

export type CommitteePageStatus = "published" | "unpublished_changes" | "never_published";

export interface CommitteePageSummary {
  slug: CommitteeSlug;
  name: string;
  color: string;
  tint: string;
  status: CommitteePageStatus;
  cover: ImageRef | null;
  publishedAt: string | null;
  draftUpdatedAt: string;
  draftUpdatedBy: PersonRef | null;
  directors: PersonRef[];
  canEdit: boolean;
}

/** GET /api/committee-pages */
export interface CommitteePagesResponse {
  pages: CommitteePageSummary[];
}

export interface EditLock {
  holder: PersonRef;
  since: string;
  /** True when the current user holds it. */
  mine: boolean;
}

/** GET /api/committee-pages/:slug */
export interface CommitteePageEditorResponse {
  committee: { slug: CommitteeSlug; name: string; color: string; tint: string };
  draft: CommitteePageDoc;
  published: CommitteePageDoc | null;
  draftVersion: number;
  status: CommitteePageStatus;
  lock: EditLock | null;
  canEdit: boolean;
  /** Published projects owned by this committee, for the projects block. */
  projects: ProjectCard[];
}

/** PUT /api/committee-pages/:slug/draft */
export interface SaveDraftInput {
  doc: CommitteePageDoc;
  baseVersion: number;
}

export interface SaveDraftResponse {
  draftVersion: number;
  savedAt: string;
  status: CommitteePageStatus;
}

/** GET /api/committee-pages/:slug/changes */
export interface PageChangesResponse {
  changes: string[];
}

// ---------------------------------------------------------------------------
// Projects

export type ProjectStatus = "live" | "in_development" | "annual" | "ongoing";
export type ProjectKind = "app" | "event" | "tool" | "brand" | "program" | "fundraiser" | "other";

export interface ProjectCard {
  id: string;
  slug: string;
  name: string;
  /** At most 48 characters. */
  summary: string;
  /** Plain text from the description, at most about 240 characters. */
  excerpt: string;
  committee: CommitteeSlug;
  status: ProjectStatus;
  kind: ProjectKind;
  cover: ImageRef | null;
  siteUrl: string | null;
  repoUrl: string | null;
  featured: boolean;
}

export interface ProjectDetail extends ProjectCard {
  /** Sanitized HTML, same rules as the text block. */
  description: string;
  builtWith: CommitteeSlug[];
  media: ImageRef[];
  videoUrl: string | null;
  publishedAt: string;
}

/** GET /api/public/projects?committee= */
export interface PublicProjectsResponse {
  projects: ProjectCard[];
}

/** GET /api/public/projects/:slug. A renamed slug answers with `redirectTo`. */
export interface PublicProjectResponse {
  project: ProjectDetail | null;
  redirectTo: string | null;
}

/** What an editor changes. Published as a whole. */
export interface ProjectDoc {
  name: string;
  summary: string;
  description: string;
  committee: CommitteeSlug;
  builtWith: CommitteeSlug[];
  status: ProjectStatus;
  kind: ProjectKind;
  siteUrl: string | null;
  repoUrl: string | null;
  videoUrl: string | null;
  /** The first image is the cover. At most 8. */
  media: ImageRef[];
}

export type ProjectState = "draft" | "published" | "archived";

export interface ProjectAdminSummary {
  id: string;
  slug: string;
  name: string;
  committee: CommitteeSlug;
  state: ProjectState;
  hasUnpublishedChanges: boolean;
  pendingApproval: boolean;
  cover: ImageRef | null;
  hasRepo: boolean;
  hasSite: boolean;
  featured: boolean;
  updatedAt: string;
  updatedBy: PersonRef | null;
  canEdit: boolean;
  canPublish: boolean;
}

/** GET /api/projects?committee=&state=&q= */
export interface ProjectListResponse {
  projects: ProjectAdminSummary[];
}

export interface ProjectChecklist {
  cover: boolean;
  summary: boolean;
  description: boolean;
  links: boolean;
  altText: boolean;
}

/** GET /api/projects/:id, also returned by every project write. */
export interface ProjectEditorResponse {
  project: {
    id: string;
    slug: string;
    state: ProjectState;
    featured: boolean;
    draft: ProjectDoc;
    published: ProjectDoc | null;
    hasUnpublishedChanges: boolean;
    /** A lead's submission while it waits, or the note it came back with since the last publish. */
    pendingApproval: ApprovalRef | null;
    editors: PersonRef[];
    /** Directors of the owning committee, who approve what leads send. */
    approvers: PersonRef[];
    checklist: ProjectChecklist;
    updatedAt: string;
    canEdit: boolean;
    canPublish: boolean;
    canArchive: boolean;
    canManageEditors: boolean;
  };
}

// ---------------------------------------------------------------------------
// Media

/** POST /api/media (multipart: file, alt) */
export interface UploadResponse {
  id: string;
  image: ImageRef;
}

// ---------------------------------------------------------------------------
// Approvals

export type ApprovalStatus = "waiting" | "approved" | "sent_back" | "withdrawn";

export interface ApprovalRef {
  id: string;
  status: ApprovalStatus;
  note: string | null;
  decidedBy: PersonRef | null;
  decidedAt: string | null;
}

export interface Approval extends ApprovalRef {
  subjectType: "announcement" | "project";
  subjectId: string;
  committee: CommitteeSlug;
  submittedBy: PersonRef | null;
  submittedAt: string;
  /** Exactly what was submitted. */
  announcement: AnnouncementInput | null;
  project: ProjectDoc | null;
  /** Portal path that shows the subject. */
  href: string;
  canDecide: boolean;
}

/** GET /api/approvals?status=waiting|decided&mine=true */
export interface ApprovalListResponse {
  approvals: Approval[];
}

// ---------------------------------------------------------------------------
// People and roles

export type Role = "admin" | "director" | "lead";

export interface RoleAssignment {
  id: string;
  role: Role;
  committee: CommitteeSlug | null;
  canPostTicker: boolean;
  /** Projects a lead may edit. Empty for other roles. */
  projectIds: string[];
  appointedBy: PersonRef | null;
  appointedAt: string;
  source: "keycloak_group" | "governance" | "portal";
}

export interface PersonRow {
  person: PersonRef & { email: string | null; hasSignedIn: boolean; lastSeenAt: string | null };
  roles: RoleAssignment[];
}

/** GET /api/people?committee= */
export interface PeopleResponse {
  people: PersonRow[];
}

/** POST /api/people */
export interface AddPersonInput {
  andrewId: string;
  name: string | null;
  role: Role;
  committee: CommitteeSlug | null;
  canPostTicker: boolean;
  projectIds: string[];
}

/** PATCH /api/people/assignments/:id */
export interface UpdateAssignmentInput {
  role?: Role;
  committee?: CommitteeSlug | null;
  canPostTicker?: boolean;
  projectIds?: string[];
}

/** GET /api/people/search?q= */
export interface PeopleSearchResponse {
  people: (PersonRef & { hasSignedIn: boolean })[];
}

/** POST /api/people/sync-governance */
export interface GovernanceSyncResponse {
  added: number;
  updated: number;
  unchanged: number;
  messages: string[];
}

// ---------------------------------------------------------------------------
// Session

export interface Permissions {
  canUsePortal: boolean;
  isAdmin: boolean;
  directorOf: CommitteeSlug[];
  leadOf: CommitteeSlug[];
  /** Sources this person may post ticker announcements from. */
  tickerSources: TickerSource[];
  /** True for leads, whose announcements wait for a director. */
  tickerNeedsApproval: boolean;
  /** Committees whose ticker announcements this person may extend, end and reorder. */
  tickerManages: TickerSource[];
  editablePages: CommitteeSlug[];
  /** Committees this person may create and publish projects for. */
  projectCommittees: CommitteeSlug[];
  /** Projects a lead was given. Directors and admins reach theirs through projectCommittees. */
  editableProjectIds: string[];
  approvesFor: CommitteeSlug[];
  /** "all" for admins, the director's committees for directors, empty otherwise. */
  peopleScope: "all" | CommitteeSlug[];
}

/** GET /api/me */
export interface MeResponse {
  user: PersonRef & { email: string | null };
  roles: RoleAssignment[];
  permissions: Permissions;
  csrfToken: string;
  authMode: "oidc" | "dev";
}

/** GET /api/auth/config */
export interface AuthConfigResponse {
  oidc: boolean;
  devLogin: boolean;
  /** Previews may require a shared key for dev sign-in. */
  devLoginNeedsKey: boolean;
  devPersonas: { andrewId: string; name: string; label: string }[];
}

/** POST /api/auth/dev-login */
export interface DevLoginInput {
  andrewId: string;
  key: string | null;
}

// ---------------------------------------------------------------------------
// Overview and activity

export interface AttentionItem {
  kind:
    | "approval"
    | "expiring"
    | "unpublished_page"
    | "project_incomplete"
    | "stale_page"
    | "sent_back";
  severity: "info" | "warning" | "neutral";
  title: string;
  detail: string;
  action: { label: string; href: string };
  at: string;
}

export interface ActivityEntry {
  id: string;
  actor: PersonRef | null;
  summary: string;
  at: string;
  href: string | null;
}

/** GET /api/overview */
export interface OverviewResponse {
  ticker: {
    live: number;
    slots: number;
    mineLive: number;
    nextToExpire: { id: string; message: string; expiresAt: string } | null;
  };
  committees: {
    total: number;
    published: number;
    withUnpublishedChanges: CommitteeSlug[];
    editable: CommitteeSlug[];
    covers: { slug: CommitteeSlug; cover: ImageRef | null }[];
  };
  projects: {
    total: number;
    published: number;
    drafts: number;
    inScope: number;
    covers: ImageRef[];
  };
  attention: AttentionItem[];
  activity: ActivityEntry[];
}

/** GET /api/activity?limit= */
export interface ActivityResponse {
  entries: ActivityEntry[];
}
