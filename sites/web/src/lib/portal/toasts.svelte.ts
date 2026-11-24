// Short confirmations at the bottom of the screen ("TartanHacks is published. View page").
// The portal layout renders them with <ToastRegion>; pages add them through getToasts().

import { createContext } from "svelte";

export type ToastTone = "success" | "error" | "info";

export interface ToastAction {
  label: string;
  href: string;
  /** Opens in a new tab, for links to the public site. */
  external?: boolean;
}

export interface ToastInput {
  message: string;
  tone?: ToastTone;
  action?: ToastAction;
  /** Milliseconds before it leaves. Defaults to 5 seconds, 8 with an action or for errors. */
  duration?: number;
}

export interface Toast {
  id: number;
  message: string;
  tone: ToastTone;
  action: ToastAction | null;
}

interface Timer {
  handle: ReturnType<typeof setTimeout> | null;
  remaining: number;
  startedAt: number;
}

const MAX_VISIBLE = 3;

export class ToastStore {
  toasts = $state<Toast[]>([]);

  #nextId = 1;
  #timers = new Map<number, Timer>();

  show(input: ToastInput): number {
    const tone = input.tone ?? "success";
    const toast: Toast = {
      id: this.#nextId++,
      message: input.message,
      tone,
      action: input.action ?? null,
    };
    const duration = input.duration ?? (toast.action !== null || tone === "error" ? 8000 : 5000);
    this.toasts = [...this.toasts, toast];
    while (this.toasts.length > MAX_VISIBLE) {
      const oldest = this.toasts[0];
      if (oldest === undefined) break;
      this.dismiss(oldest.id);
    }
    this.#start(toast.id, duration);
    return toast.id;
  }

  success(message: string, action?: ToastAction): number {
    return this.show({ message, tone: "success", action });
  }

  error(message: string, action?: ToastAction): number {
    return this.show({ message, tone: "error", action });
  }

  info(message: string, action?: ToastAction): number {
    return this.show({ message, tone: "info", action });
  }

  dismiss(id: number): void {
    const timer = this.#timers.get(id);
    if (timer !== undefined && timer.handle !== null) clearTimeout(timer.handle);
    this.#timers.delete(id);
    this.toasts = this.toasts.filter((toast) => toast.id !== id);
  }

  /** Holds a toast while the pointer or focus is on it. */
  pause(id: number): void {
    const timer = this.#timers.get(id);
    if (timer === undefined || timer.handle === null) return;
    clearTimeout(timer.handle);
    timer.handle = null;
    timer.remaining = Math.max(1500, timer.remaining - (Date.now() - timer.startedAt));
  }

  resume(id: number): void {
    const timer = this.#timers.get(id);
    if (timer === undefined || timer.handle !== null) return;
    this.#start(id, timer.remaining);
  }

  clear(): void {
    for (const id of this.#timers.keys()) this.dismiss(id);
  }

  #start(id: number, duration: number): void {
    const handle = setTimeout(() => {
      this.dismiss(id);
    }, duration);
    this.#timers.set(id, { handle, remaining: duration, startedAt: Date.now() });
  }
}

const [getToastStore, setToastStore] = createContext<ToastStore>();

/** The toast store. Available in every component inside the portal layout. */
export function getToasts(): ToastStore {
  return getToastStore();
}

export function setToasts(store: ToastStore): ToastStore {
  return setToastStore(store);
}
