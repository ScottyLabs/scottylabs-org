// One image upload with its progress, for ImageField and ImageListField.

import { isAbortError, isApiError } from "../api/client.ts";
import type { ImageRef } from "../api/types.ts";
import { uploadImage } from "./upload.ts";

export type UploadStatus = "uploading" | "processing" | "done" | "failed" | "cancelled";

let nextKey = 0;

export class UploadTask {
  readonly key = `upload-${(nextKey += 1).toString()}`;
  readonly file: File;
  /** A local preview while the upload runs. Released by dispose(). */
  readonly preview: string;
  status = $state<UploadStatus>("uploading");
  /** Share of bytes sent, from 0 to 1. */
  progress = $state(0);
  error = $state<string | null>(null);
  /** The HTTP status of a failure, so callers can react to a 401. */
  failedStatus = $state<number | null>(null);
  readonly #controller = new AbortController();

  constructor(file: File) {
    this.file = file;
    this.preview = URL.createObjectURL(file);
  }

  get percent(): number {
    return Math.round(this.progress * 100);
  }

  /** Uploads the file. Resolves with the image, or null when it failed or was cancelled. */
  async run(csrfToken: string | null, alt = ""): Promise<ImageRef | null> {
    try {
      const response = await uploadImage(this.file, {
        csrfToken,
        alt,
        signal: this.#controller.signal,
        onprogress: (fraction) => {
          this.progress = fraction;
          // The server still has to check and resize the image once every byte is sent.
          if (fraction >= 1) this.status = "processing";
        },
      });
      this.progress = 1;
      this.status = "done";
      return response.image;
    } catch (error) {
      if (isAbortError(error)) {
        this.status = "cancelled";
        return null;
      }
      this.status = "failed";
      this.failedStatus = isApiError(error) ? error.status : null;
      this.error = isApiError(error)
        ? error.message
        : `${this.file.name} did not upload. Try again.`;
      return null;
    }
  }

  cancel(): void {
    this.#controller.abort();
  }

  dispose(): void {
    this.cancel();
    URL.revokeObjectURL(this.preview);
  }
}
