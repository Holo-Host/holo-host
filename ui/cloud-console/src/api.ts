import { get } from "svelte/store";
import {
  AuthStore,
  authStore,
  clearLoginCredentials,
  storeLoginCredentials,
} from "./auth";
import { decode, JwtPayload } from "jsonwebtoken";

export const host = import.meta.env.VITE_API_HOST ?? "http://localhost:3000";

export async function request(url: string, options: RequestInit) {
  let credentials = get(authStore);

  const claims = decode(credentials.accessToken) as JwtPayload;
  if (claims.exp - 30 < Date.now() / 1000) {
    credentials = await refreshAccessToken(credentials);
  }

  return fetch(`${host}${url}`, {
    ...options,
    headers: {
      ...options.headers,
      accept: "*/*",
      "Content-Type": "application/json",
      Authorization: `Bearer ${credentials.accessToken}`,
    },
  });
}

export async function login(email: string, password: string) {
  const req = await fetch(`${host}/public/v1/auth/login-with-password`, {
    method: "post",
    headers: {
      accept: "*/*",
      "content-type": "application/json",
    },
    body: JSON.stringify({
      email,
      password,
    }),
  });
  if (!req.ok) {
    console.error("failed to login");
    return;
  }
  const res: { access_token: string; refresh_token: string } = await req.json();

  storeLoginCredentials({
    refreshToken: res.refresh_token,
    accessToken: res.access_token,
  });
  location.href = "/";
}

export async function logout() {
  clearLoginCredentials();
  location.href = "/login";
}

async function refreshAccessToken(auth: AuthStore) {
  const { accessToken, refreshToken } = auth;
  const req = await fetch(`${host}/public/v1/auth/refresh`, {
    method: "post",
    headers: {
      accept: "*/*",
      "content-type": "application/json",
    },
    body: JSON.stringify({
      access_token: accessToken,
      refresh_token: refreshToken,
    }),
  });
  if (!req.ok) {
    console.error("failed to refresh token");
    return;
  }
  const res: { access_token: string; refresh_token: string } = await req.json();
  const credentials: AuthStore = {
    refreshToken: res.refresh_token,
    accessToken: res.access_token,
  };

  storeLoginCredentials(credentials);
  return credentials;
}
