import type { InlineConfig } from "vite";
import { sveltekit } from "@sveltejs/kit/vite";
import deno from "@deno/vite-plugin";

export const config: InlineConfig = {
  plugins: [deno(), sveltekit()],
};
