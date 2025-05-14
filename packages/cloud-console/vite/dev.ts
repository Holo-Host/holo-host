import { createServer } from "vite";
import { config } from "./config.ts";

const server = await createServer(config);
await server.listen();
server.printUrls();
