import { defineEnvVars } from "@sveltejs/kit/env";

const optional = (value: string | undefined) => value;

export const variables = defineEnvVars({
  PUBLIC_POSTHOG_KEY: { public: true, schema: optional },
  PUBLIC_POSTHOG_HOST: { public: true, schema: optional },
});
