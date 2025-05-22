<script lang="ts">
  import { defaultTheme } from "../theme";
  import { getTranslation } from "../lang";

  const { path } = $props();

  const entries = [
    {
      name: getTranslation("drawer.dashboard"),
      icon: "dashboard",
      href: "/",
    },
    {
      name: getTranslation("drawer.constellations"),
      icon: "laptop_windows",
      href: "/constellations",
    },
    {
      name: getTranslation("drawer.api-tokens"),
      icon: "key",
      href: "/api-tokens",
    },
    {
      name: getTranslation("drawer.billing"),
      icon: "payments",
      href: "/billing",
    },
    {
      name: getTranslation("drawer.settings"),
      icon: "settings",
      href: "/settings",
    },
  ];
</script>

<div
  class="drawer"
  style:background-color={defaultTheme.colors.background.card}
>
  <div class="header">
    <img src="/icons/drawer/logo.svg" alt="logo" width="56" />
    <img src="/icons/drawer/holo.svg" alt="holo" width="114" />
  </div>
  <div class="entries">
    {#each entries as entry (entry.name)}
      <a
        href={entry.href}
        class="entry"
        style:background-color={path === entry.href
          ? defaultTheme.colors.background.secondary
          : ""}
        style:color={path === entry.href
          ? defaultTheme.colors.text.primary
          : defaultTheme.colors.text.subtext}
      >
        <span class="icons-outlined"> {entry.icon} </span>
        <span>
          {entry.name}
        </span>
      </a>
    {/each}
  </div>
</div>

<style lang="css">
  .drawer {
    display: flex;
    justify-content: start;
    flex-direction: column;
    width: 300px;
    height: 100vh;
    padding-right: 30px;
    gap: 20px;

    .header {
      margin-top: 30px;
      margin-bottom: 40px;
      margin-left: 30px;
      gap: 20px;
    }
  }
  .entries {
    display: flex;
    flex-direction: column;
    gap: 20px;

    .entry {
      display: flex;
      flex-direction: row;
      align-items: center;
      justify-content: start;
      text-decoration: none;
      font-size: 16px;
      font-weight: 500;
      padding-left: 30px;
      padding-top: 20px;
      padding-bottom: 20px;
      gap: 10px;
      transition:
        background-color 0.3s,
        color 0.3s;
    }
  }
</style>
