import { searchForWorkspaceRoot, defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import topLevelAwait from "vite-plugin-top-level-await";

// https://vitejs.dev/config/
export default defineConfig({
    server: {
        fs: { allow: [searchForWorkspaceRoot(process.cwd()), "../pkg"] },
    },
    plugins: [topLevelAwait(), react()],
});
