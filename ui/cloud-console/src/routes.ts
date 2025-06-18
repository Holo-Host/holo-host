import type { Component } from "svelte";
import Dashboard from "./pages/dashboard/page.svelte";
import ApiTokens from "./pages/api-tokens/page.svelte";
import Constellations from "./pages/constellations/page.svelte";
import Billing from "./pages/billing/page.svelte";
import Settings from "./pages/settings/page.svelte";
import GenerateToken from "./pages/generate-token/page.svelte";
import Login from "./pages/login/page.svelte";
import Register from "./pages/register/page.svelte";
import ForgotPassword from "./pages/forgot-password/page.svelte";

export type Route = {
  path: string;
  component: Component;
  // show drawer (left bar) in the page
  // default: true
  drawer?: boolean;
  // show header (top bar) in the page
  // default: true
  header?: boolean;
  // if set to true then login is required
  // default: true
  isAuthenticated?: boolean;
};

export const NotFoundComponent: Component | null = null;
export const routes: Route[] = [
  {
    path: "/",
    component: Dashboard,
  },
  {
    path: "/login",
    component: Login,
    isAuthenticated: false,
    drawer: false,
    header: false,
  },
  {
    path: "/register",
    component: Register,
    isAuthenticated: false,
    drawer: false,
    header: false,
  },
  {
    path: "/forgot-password",
    component: ForgotPassword,
    isAuthenticated: false,
    drawer: false,
    header: false,
  },
  {
    path: "/api-tokens",
    component: ApiTokens,
  },
  {
    path: "/constellations",
    component: Constellations,
  },
  {
    path: "/billing",
    component: Billing,
  },
  {
    path: "/settings",
    component: Settings,
  },
  {
    path: "/generate-token",
    component: GenerateToken,
  },
];
