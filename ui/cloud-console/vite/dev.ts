import { createServer } from "vite";
import { config } from "./config.ts";

const server = await createServer({
  ...config,
  server: {
    fs: {
      allow: [".", "../.."],
    },
  },
});
await server.listen();
server.printUrls();