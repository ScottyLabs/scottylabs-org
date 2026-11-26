// Typed calls to the site service's JSON API on this origin. Paths are relative to /api, so `get<MeResponse>("/me")` fetches /api/me. Writes carry the session's CSRF token, and every failure, including a dropped connection, becomes an ApiError with a message people can read.
// The JSON shapes are in ./types.ts.

import type { ApiErrorBody, UploadResponse } from "./types.ts";

export type ApiErrorCode = ApiErrorBody["error"]["code"] | "network" | "unexpected_response";

/** A failed API call. `message` is a plain sentence that can be shown as is. */
export class ApiError extends Error {
  override readonly name = "ApiError";
  /** HTTP status, or 0 when the request never reached the server. */
  readonly status: number;
  readonly code: ApiErrorCode;
  /** Per-field messages for `validation_failed`, keyed by camelCase field path. */
  readonly fields: Readonly<Record<string, string>>;

  constructor(
    status: number,
    code: ApiErrorCode,
    message: string,
    fields: Record<string, string> = {},
  ) {
    super(message);
    this.status = status;
    this.code = code;
    this.fields = fields;
  }

  /** The message for one field, if the server sent one. */
  field(path: string): string | undefined {
    return this.fields[path];
  }
}

export function isApiError(error: unknown): error is ApiError {
  return error instanceof ApiError;
}

/** True when a call was cancelled through its AbortSignal. Callers usually ignore these. */
export function isAbortError(error: unknown): boolean {
  return error instanceof DOMException && error.name === "AbortError";
}

type QueryValue = string | number | boolean | null | undefined;

export interface RequestOptions {
  /** Added to the URL. `null` and `undefined` values are left out. */
  query?: Record<string, QueryValue | readonly QueryValue[]>;
  signal?: AbortSignal;
  /** SvelteKit's `fetch` inside load functions; the global one otherwise. */
  fetch?: typeof fetch;
  /**
   * Whether a 401 runs the handler set with `onUnauthenticated` (the portal sends people to sign
   * in). Turn it off for calls that expect to be signed out, such as the first GET /api/me.
   */
  redirectOnUnauthenticated?: boolean;
}

type Method = "GET" | "POST" | "PUT" | "PATCH" | "DELETE";

const WRITE_METHODS: ReadonlySet<Method> = new Set(["POST", "PUT", "PATCH", "DELETE"]);

let csrfToken: string | null = null;
let unauthenticatedHandler: ((error: ApiError) => void) | null = null;

/** Set from GET /api/me. Sent as X-CSRF-Token on every write. Pass `null` after signing out. */
export function setCsrfToken(token: string | null): void {
  csrfToken = token;
}

/**
 * Called once for each 401 response, unless the request opted out. Returns a function that
 * removes the handler.
 */
export function onUnauthenticated(handler: (error: ApiError) => void): () => void {
  unauthenticatedHandler = handler;
  return () => {
    if (unauthenticatedHandler === handler) unauthenticatedHandler = null;
  };
}

export function get<T>(path: string, options?: RequestOptions): Promise<T> {
  return request<T>("GET", path, undefined, options);
}

export function post<T = void>(path: string, body?: unknown, options?: RequestOptions): Promise<T> {
  return request<T>("POST", path, body, options);
}

export function put<T = void>(path: string, body?: unknown, options?: RequestOptions): Promise<T> {
  return request<T>("PUT", path, body, options);
}

export function patch<T = void>(
  path: string,
  body?: unknown,
  options?: RequestOptions,
): Promise<T> {
  return request<T>("PATCH", path, body, options);
}

export function del<T = void>(path: string, options?: RequestOptions): Promise<T> {
  return request<T>("DELETE", path, undefined, options);
}

/** POST /api/media with the file and its alt text. */
export function upload(file: File, alt = "", options?: RequestOptions): Promise<UploadResponse> {
  const form = new FormData();
  form.append("file", file);
  if (alt.trim() !== "") form.append("alt", alt.trim());
  return request<UploadResponse>("POST", "/media", form, options);
}

/** All helpers in one object, for `import { api } from "#lib/api/client.ts"`. */
export const api = { get, post, put, patch, del, upload };

/** The URL for an API path, with query parameters. Exported for links such as sign-in. */
export function apiUrl(path: string, query?: RequestOptions["query"]): string {
  if (!path.startsWith("/") || path.startsWith("//")) {
    throw new Error(`API paths start with a single "/": ${path}`);
  }
  const url = `/api${path}`;
  const search = encodeQuery(query);
  return search === "" ? url : `${url}${url.includes("?") ? "&" : "?"}${search}`;
}

function isQueryList(value: QueryValue | readonly QueryValue[]): value is readonly QueryValue[] {
  return Array.isArray(value);
}

function encodeQuery(query: RequestOptions["query"]): string {
  if (query === undefined) return "";
  const params = new URLSearchParams();
  for (const [key, value] of Object.entries(query)) {
    const values = isQueryList(value) ? value : [value];
    for (const item of values) {
      if (item !== null && item !== undefined) params.append(key, String(item));
    }
  }
  return params.toString();
}

/**
 * Responses follow the shapes in ./types.ts, which the service and this app share, so a parsed
 * body is trusted as the caller's type instead of being checked field by field.
 */
// oxlint-disable-next-line typescript/no-unnecessary-type-parameters
function fromContract<T>(value?: unknown): T {
  // oxlint-disable-next-line typescript/no-unsafe-type-assertion
  return value as T;
}

/**
 * The session's CSRF token as GET /api/me gives it now, or null when it cannot say. A tab loaded
 * before a deploy (or a rollback) that changed how the token is made still holds the old one.
 */
async function freshCsrfToken(doFetch: typeof fetch): Promise<string | null> {
  try {
    const response = await doFetch(apiUrl("/me"), {
      method: "GET",
      headers: { Accept: "application/json" },
      credentials: "same-origin",
    });
    if (!response.ok) return null;
    const me: unknown = await response.json();
    if (typeof me !== "object" || me === null || !("csrfToken" in me)) return null;
    return typeof me.csrfToken === "string" ? me.csrfToken : null;
  } catch {
    return null;
  }
}

async function request<T>(
  method: Method,
  path: string,
  body: unknown,
  options: RequestOptions = {},
  retryWithFreshToken = true,
): Promise<T> {
  const headers = new Headers({ Accept: "application/json" });
  let payload: BodyInit | undefined;
  if (body instanceof FormData) {
    payload = body;
  } else if (body !== undefined) {
    headers.set("Content-Type", "application/json");
    payload = JSON.stringify(body);
  }
  const sentToken = WRITE_METHODS.has(method) ? csrfToken : null;
  if (sentToken !== null) headers.set("X-CSRF-Token", sentToken);

  const doFetch = options.fetch ?? fetch;
  let response: Response;
  try {
    response = await doFetch(apiUrl(path, options.query), {
      method,
      headers,
      body: payload,
      credentials: "same-origin",
      signal: options.signal,
    });
  } catch (error) {
    if (isAbortError(error)) throw error;
    throw new ApiError(
      0,
      "network",
      "We could not reach scottylabs.org. Check your connection and try again.",
    );
  }

  if (!response.ok) {
    const error = await errorFromResponse(response);
    if (error.status === 401 && options.redirectOnUnauthenticated !== false) {
      unauthenticatedHandler?.(error);
    }
    // A write the server refused before doing anything, because the token is not the one the session has now: ask for the current token once, and send the write again with it.
    if (error.status === 403 && sentToken !== null && retryWithFreshToken) {
      const fresh = await freshCsrfToken(doFetch);
      if (fresh !== null && fresh !== sentToken) {
        csrfToken = fresh;
        return request<T>(method, path, body, options, false);
      }
    }
    throw error;
  }
  return readBody<T>(response);
}

async function readBody<T>(response: Response): Promise<T> {
  if (response.status === 204 || response.status === 205) return fromContract<T>();
  const text = await response.text();
  if (text === "") return fromContract<T>();
  try {
    return fromContract<T>(JSON.parse(text));
  } catch {
    throw new ApiError(
      response.status,
      "unexpected_response",
      "The server sent a response we could not read. Try again in a moment.",
    );
  }
}

/** Builds an ApiError from a failed response, using the server's message when it sent one. */
export async function errorFromResponse(response: Response): Promise<ApiError> {
  let body: unknown = null;
  try {
    body = await response.json();
  } catch {
    // Not JSON, for example a proxy error page. Fall back to a message for the status.
  }
  if (isErrorBody(body)) {
    const { code, message, fields } = body.error;
    return new ApiError(response.status, code, message, stringRecord(fields));
  }
  return new ApiError(
    response.status,
    codeForStatus(response.status),
    messageForStatus(response.status),
  );
}

function isErrorBody(value: unknown): value is ApiErrorBody {
  if (typeof value !== "object" || value === null || !("error" in value)) return false;
  const { error } = value;
  if (typeof error !== "object" || error === null) return false;
  return (
    "code" in error &&
    typeof error.code === "string" &&
    "message" in error &&
    typeof error.message === "string"
  );
}

function stringRecord(value: unknown): Record<string, string> {
  if (typeof value !== "object" || value === null) return {};
  const entries = Object.entries(value).filter(
    (entry): entry is [string, string] => typeof entry[1] === "string",
  );
  return Object.fromEntries(entries);
}

function codeForStatus(status: number): ApiErrorCode {
  switch (status) {
    case 400:
      return "bad_request";
    case 401:
      return "unauthenticated";
    case 403:
      return "forbidden";
    case 404:
      return "not_found";
    case 409:
      return "conflict";
    case 413:
      return "payload_too_large";
    case 415:
      return "unsupported_media_type";
    case 422:
      return "validation_failed";
    case 429:
      return "rate_limited";
    // A 503 without the service's JSON body came from a proxy or a starting service, so it says nothing about sign-in. Only the service's own body may carry auth_not_configured.
    default:
      return status >= 500 ? "internal" : "unexpected_response";
  }
}

function messageForStatus(status: number): string {
  switch (status) {
    case 401:
      return "Your session has ended. Sign in again to continue.";
    case 403:
      return "You do not have permission to do that. Ask your director if you think you should.";
    case 404:
      return "We could not find that. It may have been moved or removed.";
    case 413:
      return "That is too large to send. Choose a smaller file and try again.";
    case 429:
      return "That was a lot of requests in a short time. Wait a minute and try again.";
    default:
      return status >= 500
        ? "Something went wrong on our side. Try again in a moment."
        : "That request did not work. Check what you entered and try again.";
  }
}
