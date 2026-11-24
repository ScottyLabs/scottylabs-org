import adapter from "@sveltejs/adapter-static";
import { sveltekit } from "@sveltejs/kit/vite";
import { defineConfig, loadEnv } from "vite";
import { ViteImageOptimizer } from "vite-plugin-image-optimizer";

export default defineConfig(({ mode }) => {
  // In development the site service runs on 127.0.0.1:8080, or wherever SITE_SERVICE_URL points. Proxying /api and /media keeps one origin in the browser, as in production, where the service serves this build.
  const service =
    loadEnv(mode, false, "SITE_SERVICE_URL").SITE_SERVICE_URL ?? "http://127.0.0.1:8080";

  return {
    plugins: [
      // Pages that are not prerendered (project pages and the portal) load from 200.html, which the site service returns for those paths.
      sveltekit({
        adapter: adapter({ fallback: "200.html" }),
        prerender: {
          // The header links to /projects/ and /committees/, whose routes are built separately. Until a route exists, its link warns instead of failing the build; every other broken link still fails it.
          handleHttpError: ({ path, status, message }) => {
            const pendingSection = path === "/projects/" || path === "/committees/";
            if (status === 404 && pendingSection) {
              console.warn(`Prerender: ${path} has no page yet.`);
              return;
            }
            throw new Error(message);
          },
        },
      }),
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
  };
});
