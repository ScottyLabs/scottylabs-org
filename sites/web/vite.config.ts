import adapter from "@sveltejs/adapter-static";
import { sveltekit } from "@sveltejs/kit/vite";
import { defineConfig } from "vite";
import { ViteImageOptimizer } from "vite-plugin-image-optimizer";

// In development the site service runs on 127.0.0.1:8080. Proxying /api and /media keeps one origin in the browser, as in production, where the service serves this build.
const service = "http://127.0.0.1:8080";

export default defineConfig({
  plugins: [
    // Pages that are not prerendered (project pages and the portal) load from 200.html, which the site service returns for those paths.
    sveltekit({ adapter: adapter({ fallback: "200.html" }) }),
    ViteImageOptimizer({
      jpg: {
        quality: 90,
      },
    }),
  ],
  server: {
    proxy: {
      "/api": { target: service, changeOrigin: false },
      "/media": { target: service, changeOrigin: false },
    },
  },
});
