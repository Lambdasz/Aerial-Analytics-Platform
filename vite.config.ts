import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import process from "node:process";
const host = process.env.TAURI_DEV_HOST;

import fs from "node:fs";
import path from "node:path";

function serveMapStorage() {
  return {
    name: "serve-map-storage",
    configureServer(server: {
      middlewares: {
        use: (
          prefix: string,
          handler: (
            req: { url?: string },
            res: { setHeader: (k: string, v: string) => void },
            next: () => void,
          ) => void,
        ) => void;
      };
    }) {
      server.middlewares.use("/map_storage", (req, res, next) => {
        try {
          if (req.url && (req.url.includes("?import") || req.url.includes("&import"))) {
            return next();
          }
          const rawUrl = decodeURIComponent(req.url || "").split("?")[0];
          const safePrefix = path.resolve(process.cwd(), "map_storage");
          const filePath = path.resolve(safePrefix, rawUrl.replace(/^\//, ""));

          if (!filePath.startsWith(safePrefix + path.sep) && filePath !== safePrefix) {
            (res as unknown as { statusCode: number; end: (msg: string) => void }).statusCode = 403;
            (res as unknown as { end: (msg: string) => void }).end(
              "Forbidden: Path traversal not allowed",
            );
            return;
          }

          if (fs.existsSync(filePath) && fs.statSync(filePath).isFile()) {
            const ext = path.extname(filePath).toLowerCase();
            const mimeTypes: Record<string, string> = {
              ".jpg": "image/jpeg",
              ".jpeg": "image/jpeg",
              ".png": "image/png",
              ".webp": "image/webp",
            };
            if (mimeTypes[ext]) {
              res.setHeader("Content-Type", mimeTypes[ext]);
              fs.createReadStream(filePath).pipe(res as unknown as NodeJS.WritableStream);
              return;
            }
          }
        } catch (e) {
          console.error("Error serving map_storage:", e);
        }
        next();
      });
    },
  };
}

// https://vite.dev/config/
export default defineConfig(() => ({
  plugins: [react(), serveMapStorage()],

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. tell Vite to ignore watching `src-tauri`, `map_storage`, and `storage`
      ignored: ["**/src-tauri/**", "**/map_storage/**", "**/storage/**"],
    },
  },
  optimizeDeps: {
    entries: ["index.html"],
  },
}));
