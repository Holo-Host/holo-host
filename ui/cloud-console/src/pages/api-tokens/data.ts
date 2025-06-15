import type { ApiKey } from "./types";

const data: ApiKey[] = [
  {
    id: "",
    name: "My ApiKey",
    permissions: ["all.all.self"],
    expiresAt: new Date(Date.now() + 1000000000),
  },
  {
    id: "",
    name: "Workload Apikey",
    permissions: ["workload.read.self", "workload.create.self"],
    expiresAt: new Date(),
  },
];

export function getApiKeys() {
  return data;
}

export function deleteApiKey(key: ApiKey) {
  console.log("deleted api key");
}
