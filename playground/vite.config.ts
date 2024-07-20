import { searchForWorkspaceRoot, defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import topLevelAwait from "vite-plugin-top-level-await";

const proxy = process.env.VITE_PROXY == "1";

// https://vitejs.dev/config/

export default defineConfig({
    server: {
        fs: {
            allow: [
                searchForWorkspaceRoot(process.cwd()),
                "../bindings/wasm/pkg",
            ],
        },
    },
    base: proxy ? "/play" : "/sqlsonnet",
    plugins: [topLevelAwait(), react()],
    build: {
        outDir: proxy ? "dist-proxy" : "../docs",
        emptyOutDir: true,
    },
});
