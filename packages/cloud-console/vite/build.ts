import { build } from "vite";
import { config } from "./config.ts";

build({
  ...config,
  mode: "production",
});
