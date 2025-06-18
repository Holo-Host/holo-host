import { writable } from "svelte/store";

export type AuthStore = {
  isAuthenticated?: boolean;
};

export const authStore = writable<AuthStore>({});
