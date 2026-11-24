// Uploading images to POST /api/media with progress. fetch() cannot report upload progress, so this uses XMLHttpRequest and turns failures into the same ApiError the API client throws.

import { ApiError, errorFromResponse } from "../api/client.ts";
import type { UploadResponse } from "../api/types.ts";

/** The server refuses anything larger. */
export const MAX_UPLOAD_BYTES = 5 * 1024 * 1024;

export const ACCEPTED_TYPES = ["image/jpeg", "image/png", "image/webp"] as const;

/** For the file input's `accept`. */
export const ACCEPT = ACCEPTED_TYPES.join(",");

/** Shown under drop zones. */
export const UPLOAD_HINT = "JPG, PNG or WebP, up to 5 MB";

const EXTENSIONS = /\.(jpe?g|png|webp)$/iu;

/** "820 KB", "4.2 MB". */
export function formatBytes(bytes: number): string {
  if (bytes < 1024 * 1024) return `${Math.max(1, Math.round(bytes / 1024)).toString()} KB`;
  const megabytes = bytes / (1024 * 1024);
  return `${megabytes.toFixed(megabytes < 10 ? 1 : 0)} MB`;
}

/** Why a file cannot be uploaded, or null when it can. The server checks the bytes again. */
export function checkImageFile(file: Pick<File, "name" | "size" | "type">): string | null {
  const typeOk =
    (ACCEPTED_TYPES as readonly string[]).includes(file.type) ||
    (file.type === "" && EXTENSIONS.test(file.name));
  if (!typeOk)
    return `${file.name} is not a JPG, PNG or WebP image. Choose a photo in one of those formats.`;
  if (file.size > MAX_UPLOAD_BYTES) {
    return `${file.name} is ${formatBytes(file.size)}. Choose a photo of 5 MB or less.`;
  }
  if (file.size === 0) return `${file.name} is empty. Choose another photo.`;
  return null;
}

export interface UploadOptions {
  /** The session's CSRF token, from getPortalSession().csrfToken. */
  csrfToken: string | null;
  alt?: string;
  /** Called with the share of bytes sent, from 0 to 1. */
  onprogress?: (fraction: number) => void;
  signal?: AbortSignal;
}

function abortError(): DOMException {
  return new DOMException("The upload was cancelled.", "AbortError");
}

/** Sends one image to POST /api/media. Rejects with ApiError, or an AbortError when cancelled. */
export function uploadImage(file: File, options: UploadOptions): Promise<UploadResponse> {
  const { csrfToken, alt = "", onprogress, signal } = options;
  return new Promise((resolve, reject) => {
    if (signal?.aborted === true) {
      reject(abortError());
      return;
    }
    const xhr = new XMLHttpRequest();
    const form = new FormData();
    form.append("file", file);
    if (alt.trim() !== "") form.append("alt", alt.trim());

    // Settle straight away: an abort before the request is under way fires no events.
    const onAbort = () => {
      xhr.abort();
      reject(abortError());
    };
    signal?.addEventListener("abort", onAbort, { once: true });
    const done = () => {
      signal?.removeEventListener("abort", onAbort);
    };

    xhr.upload.addEventListener("progress", (event) => {
      if (event.lengthComputable && event.total > 0) onprogress?.(event.loaded / event.total);
    });
    xhr.addEventListener("load", () => {
      done();
      const response = new Response(xhr.responseText === "" ? null : xhr.responseText, {
        status: xhr.status,
        headers: { "Content-Type": xhr.getResponseHeader("Content-Type") ?? "application/json" },
      });
      if (response.ok) {
        response
          .json()
          .then((body: UploadResponse) => {
            resolve(body);
          })
          .catch(() => {
            reject(
              new ApiError(
                xhr.status,
                "unexpected_response",
                "The server sent a response we could not read. Try again in a moment.",
              ),
            );
          });
      } else {
        void errorFromResponse(response).then(reject);
      }
    });
    xhr.addEventListener("error", () => {
      done();
      reject(
        new ApiError(
          0,
          "network",
          "We could not reach scottylabs.org. Check your connection and try again.",
        ),
      );
    });
    xhr.addEventListener("abort", () => {
      done();
      reject(abortError());
    });

    xhr.open("POST", "/api/media");
    xhr.setRequestHeader("Accept", "application/json");
    if (csrfToken !== null) xhr.setRequestHeader("X-CSRF-Token", csrfToken);
    xhr.withCredentials = true;
    xhr.send(form);
  });
}
