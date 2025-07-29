import { jwtDecode } from "jwt-decode";
import { writable } from "svelte/store";

export type AuthStore = {
  accessToken: string;
  refreshToken: string;
  claims: Record<string, string>;
};

const savedAuth = localStorage.getItem("auth");
export const authStore = writable<AuthStore | null>(
  savedAuth
    ? {
        ...JSON.parse(savedAuth),
        claims: jwtDecode(JSON.parse(savedAuth).accessToken),
      }
    : null
);
export function storeLoginCredentials(auth: Omit<AuthStore, "claims">) {
  localStorage.setItem("auth", JSON.stringify(auth));
  authStore.set({
    ...auth,
    claims: jwtDecode(auth.accessToken),
  });
}
export function clearLoginCredentials() {
  localStorage.removeItem("auth");
  authStore.set(null);
}
