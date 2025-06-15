import type { Component } from "svelte";
import { derived, readable } from "svelte/store";
import { NotFoundComponent, routes } from "./routes";

export type RouteRenderer = {
  component: Component;
  path: string;
  params?: Record<string, string>;
  query?: Record<string, string>;
  hash?: string;
  showDrawer: boolean;
  showHeader: boolean;
};

/**
 * Gets the current location of the browser.
 * @returns An object containing the path, query parameters, and hash.
 */
function getLocation() {
  return {
    path: location.pathname,
    query: Object.fromEntries(new URLSearchParams(location.search).entries()),
    hash: location.hash.split("#")[1] || "",
  };
}

/**
 * Gets the route component for the current location.
 * @param location - The location object containing path, query, and hash.
 * @returns The route component or null if not found.
 */
export function getRouteComponent(
  location: ReturnType<typeof getLocation>
): RouteRenderer | null {
  const path = location.path;
  const query = location.query;
  const hash = location.hash;

  // sort routes by path length to match the most specific route first
  const sortedRoutes = routes.sort((a, b) => b.path.length - a.path.length);
  for (const route of sortedRoutes) {
    const regexBuilder = route.path.replace(/\{([^\}]+)\}/g, "(?<$1>[^/]+)");
    const regex = new RegExp(`^${regexBuilder}$`);
    const match = regex.exec(path);
    if (match) {
      const params = match.groups || {};
      return {
        component: route.component,
        path,
        params,
        query,
        hash,
        showDrawer: route.drawer ?? true,
        showHeader: route.header ?? true,
      };
    }
  }

  // show 404 page if no route matches
  if (NotFoundComponent) {
    return {
      component: NotFoundComponent,
      path,
      params: {},
      query,
      hash,
      showDrawer: true,
      showHeader: true,
    };
  }
  return null;
}

/**
 * A Svelte store that tracks the current route.
 * It updates when the URL changes, and provides the current location.
 */
export const routeStore = readable(getLocation(), (set) => {
  const update = () => set(getLocation());
  const onLinkClicked = (ev: MouseEvent) => {
    // Only handle left-clicks without modifier keys
    if (
      ev.defaultPrevented ||
      ev.button !== 0 || // Not left-click
      ev.metaKey ||
      ev.ctrlKey ||
      ev.shiftKey ||
      ev.altKey
    )
      return;

    // Walk up the DOM tree to find the closest <a> element
    let target = ev.target as HTMLElement;
    while (target && target.tagName !== "A") {
      target = target.parentElement as HTMLElement;
    }

    if (target instanceof HTMLAnchorElement && target.href) {
      // Only intercept same-origin links
      const url = new URL(target.href);
      if (url.origin === location.origin) {
        ev.preventDefault();
        history.pushState({}, "", url.pathname + url.search + url.hash);
        set(getLocation());
      }
    }
  };
  addEventListener("popstate", update);
  addEventListener("hashchange", update);
  addEventListener("click", onLinkClicked);
  return () => {
    removeEventListener("popstate", update);
    removeEventListener("hashchange", update);
    removeEventListener("click", onLinkClicked);
  };
});

/**
 * A Svelte store that provides the current route component.
 * It derives from the routeStore and uses getRouteComponent to get the component.
 */
export const routeComponent = derived(routeStore, ($loc) =>
  getRouteComponent($loc)
);
