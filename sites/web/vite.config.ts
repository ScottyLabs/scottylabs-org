import { defineConfig } from "vite";
import { ViteImageOptimizer } from "vite-plugin-image-optimizer";
import { reactRouter } from "@react-router/dev/vite";

// https://vite.dev/config/
export default defineConfig({
  plugins: [
    reactRouter(),
    ViteImageOptimizer({
      jpg: {
        quality: 90,
      },
    }),
  ],
  envPrefix: ["VITE_", "POSTHOG_"],
  ssr: {
    noExternal: ["posthog-js"],
  },
});
