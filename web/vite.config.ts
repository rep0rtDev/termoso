import { readFileSync, readdirSync, statSync } from "node:fs";
import { join, relative } from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig, type Plugin } from "vite";
import react from "@vitejs/plugin-react";
import { LANDING_SEO, SPONSOR_SEO } from "./src/seo";

/**
 * Emits `dist/seo.json`: the `<title>` / description of every public page
 * (landing + docs), keyed by route. The server injects them into `index.html`
 * so crawlers and link previews see real metadata without running the SPA.
 */
function seoManifest(): Plugin {
  const root = fileURLToPath(new URL("./src/docs/content", import.meta.url));
  const walk = (dir: string): string[] =>
    readdirSync(dir).flatMap((name) => {
      const p = join(dir, name);
      return statSync(p).isDirectory() ? walk(p) : p.endsWith(".md") ? [p] : [];
    });
  return {
    name: "termoso-seo-manifest",
    apply: "build",
    generateBundle() {
      const pages: Record<string, { title: string; description: string }> = {
        "/": LANDING_SEO,
        "/sponsor": SPONSOR_SEO,
      };
      for (const file of walk(root).sort()) {
        const src = readFileSync(file, "utf8");
        const m = /^---\n([\s\S]*?)\n---\n?/.exec(src);
        const meta: Partial<Record<string, string>> = {};
        for (const line of (m?.[1] ?? "").split("\n")) {
          const i = line.indexOf(":");
          if (i > 0) meta[line.slice(0, i).trim()] = line.slice(i + 1).trim();
        }
        if (!meta.title) throw new Error(`docs: ${file} has no title`);
        const slug = relative(root, file).replace(/\.md$/, "").split("\\").join("/");
        const route = slug === "index" ? "/docs" : `/docs/${slug}`;
        pages[route] = {
          title: slug === "index" ? "Termoso docs" : `${meta.title} · Termoso docs`,
          description: meta.description ?? "",
        };
      }
      this.emitFile({ type: "asset", fileName: "seo.json", source: JSON.stringify(pages) });
    },
  };
}

// The API is same-origin in production (axum serves this bundle). In dev the
// server runs on :8080 and Vite proxies /api and /healthz to it.
const PKG_VERSION = (
  JSON.parse(readFileSync(new URL("./package.json", import.meta.url), "utf8")) as {
    version: string;
  }
).version;

export default defineConfig({
  // The cabinet reports this as its app_version; it must track the server release.
  define: { __APP_VERSION__: JSON.stringify(PKG_VERSION) },
  plugins: [react(), seoManifest()],
  resolve: { alias: { "@": fileURLToPath(new URL("./src", import.meta.url)) } },
  build: {
    target: "es2022",
    sourcemap: false,
    outDir: "dist",
    emptyOutDir: true,
    // The server's CSP is `font-src 'self'`; inlined `data:` fonts would be blocked.
    assetsInlineLimit: 0,
  },
  server: {
    port: 5173,
    proxy: {
      "/api": { target: "http://127.0.0.1:8080", ws: true },
      "/healthz": "http://127.0.0.1:8080",
    },
  },
});
