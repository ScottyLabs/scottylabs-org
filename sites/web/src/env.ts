import { defineEnvVars } from "@sveltejs/kit/env";

const optional = (value: string | undefined) => value;

export const variables = defineEnvVars({
  POSTHOG_KEY: { public: true, static: true, schema: optional },
  POSTHOG_HOST: { public: true, static: true, schema: optional },
});
