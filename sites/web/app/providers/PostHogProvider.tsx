import { useEffect } from "react";
import posthog from "posthog-js";

const POSTHOG_KEY = import.meta.env.POSTHOG_KEY;
const POSTHOG_HOST = import.meta.env.POSTHOG_HOST;

export function PostHogProvider({ children }: { children: React.ReactNode }) {
  useEffect(() => {
    if (POSTHOG_KEY === undefined) return;
    posthog.init(POSTHOG_KEY, {
      api_host: POSTHOG_HOST,
      ui_host: "https://us.posthog.com",
      defaults: "2025-05-24",
    });
  }, []);

  return children;
}
