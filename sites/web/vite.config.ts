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
        // Each page starts with an inline script. SvelteKit hashes it into a <meta> policy that allows only scripts from this origin and that one. The site service sends the full policy as a header with the same hashes (services/site/src/static_files.rs); the directives that depend on the deployment, such as the analytics host, live there.
        // The one inline event handler allowed is the one Svelte writes on prerendered images with an onload or onerror handler, `this.__e=event`, which keeps the event for after hydration.
        csp: {
          mode: "hash",
          directives: {
            "script-src": [
              "self",
              "unsafe-hashes",
              "sha256-7dQwUgLau1NFCCGjfn9FsYptB6ZtWxJin6VohGIu20I=",
            ],
            "object-src": ["none"],
            "base-uri": ["self"],
          },
        },
        prerender: {
          // Prerendered pages preload their content from the site service (/api/...), which the build does not run, so the crawler finds nothing there. Anything else still fails.
          handleHttpError: ({ path, status, message }) => {
            if (status === 404 && path.startsWith("/api/")) return;
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
