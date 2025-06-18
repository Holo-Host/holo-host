import { writable } from "svelte/store";

export type AuthStore = {
  accessToken: string;
  refreshToken: string;
};

const savedAuth = localStorage.getItem("auth");
export const authStore = writable<AuthStore | null>(
  savedAuth ? JSON.parse(savedAuth) : null
);

export function storeLoginCredentials(auth: AuthStore) {
  localStorage.setItem("auth", JSON.stringify(auth));
  authStore.set(auth);
}
export function clearLoginCredentials() {
  localStorage.removeItem("auth");
  authStore.set(null);
}
