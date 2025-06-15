import type { Component } from "svelte";
import Dashboard from "./pages/dashboard/page.svelte";
import ApiTokens from "./pages/api-tokens/page.svelte";
import Constellations from "./pages/constellations/page.svelte";
import Billing from "./pages/billing/page.svelte";
import Settings from "./pages/settings/page.svelte";

export type Route = {
  path: string;
  component: Component;
  // show drawer (left bar) in the page
  // default: true
  drawer?: boolean;
  // show header (top bar) in the page
  // default: true
  header?: boolean;
};

export const NotFoundComponent: Component | null = null;
export const routes: Route[] = [
  {
    path: "/",
    component: Dashboard,
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
];
