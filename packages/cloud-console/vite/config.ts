import type { InlineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import deno from "@deno/vite-plugin";

export const config: InlineConfig = {
  plugins: [deno(), svelte()],
  ssr: {
    noExternal: true,
  },
  build: {
    ssr: false,
    outDir: "build",
  },
};
