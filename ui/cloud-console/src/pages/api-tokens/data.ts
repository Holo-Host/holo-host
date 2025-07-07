import { request } from "@/api";
import type { ApiKey } from "./types";

export async function getApiKeys() {
  const req = await request("/protected/v1/apikeys?page=1&limit=100", {
    method: "get",
  });
  if (!req.ok) {
    console.error("failed to get api keys");
    return [];
  }
  const rawData = await req.json();
  const data = rawData["items"];
  return data.map((item) => ({
    id: item.id,
    name: item.description,
    expiresAt: new Date(item.expire_at),
    permissions: item.permissions.map(
      (perm) => perm.resource + "." + perm.action + "." + perm.owner
    ),
  }));
}

export async function deleteApiKey(key: ApiKey) {
  await request(`/protected/v1/apikey/${key.id}`, {
    method: "delete",
  });
}
