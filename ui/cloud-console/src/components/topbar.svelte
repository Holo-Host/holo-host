<script lang="ts">
  import { logout } from "@/api";
  import {
    getSupportedLanguages,
    getTranslation,
    getTranslationWithLang,
    setLanguage,
  } from "../lang";
  import { defaultTheme } from "../theme";
  import Dropdown from "./dropdown.svelte";

  type Language = {
    label: string;
    value: string;
  };

  const languages: Language[] = getSupportedLanguages().map((lang) => ({
    label: getTranslationWithLang(lang, "topbar.language"),
    value: lang,
  }));

  type ProfileItem = {
    key: string;
    label: string;
  };
  const profileItems: ProfileItem[] = [
    { key: "settings", label: "Settings" },
    { key: "logout", label: "Logout" },
  ];

  function onProfileItemSelected(item: ProfileItem) {
    switch (item.key) {
      case "settings":
        location.href = "/settings";
        break;
      case "logout":
        logout();
        break;
    }
  }
</script>

<div
  class="top-bar"
  style:background-color={defaultTheme.colors.background.card}
  style:--shadow-color={defaultTheme.colors.shadow}
>
  <div class="search">
    <span
      class="icons-outlined search-icon"
      style:--color={defaultTheme.colors.text.subtext}>search</span
    >
    <input
      type="text"
      placeholder="Search by resource name or public IP"
      style:--text-color={defaultTheme.colors.text.black}
      style:--placeholder-color={defaultTheme.colors.text.subtext}
    />
  </div>
  <a
    class="top-bar-tool"
    href="#support"
    style:--text-color={defaultTheme.colors.text.black}
  >
    {getTranslation("topbar.support")}
  </a>
  <!-- Language Selector -->
  <Dropdown
    items={languages}
    onItemSelected={(item: Language) => setLanguage(item.value)}
  >
    <div class="language-selector">
      <span>{getTranslation("topbar.language")}</span>
      <span
        class="icons-outlined expand"
        style:--color={defaultTheme.colors.text.subtext}
      >
        expand_more
      </span>
    </div>
    {#snippet itemTemplate(item: Language)}
      <span>{item.label}</span>
    {/snippet}
  </Dropdown>
  <!-- Profile Dropdown -->
  <Dropdown
    items={profileItems}
    onItemSelected={(item: ProfileItem) => onProfileItemSelected(item)}
  >
    <div class="user-info">
      <span
        class="icons-outlined user-info-avatar"
        style:--color={defaultTheme.colors.text.white}
        style:--background-color={defaultTheme.colors.background.primary}
      >
        person
      </span>
      <span>
        ZA
        <span
          class="icons-outlined"
          style:--color={defaultTheme.colors.text.subtext}
        >
          expand_more
        </span>
      </span>
    </div>
    {#snippet itemTemplate(item: ProfileItem)}
      <span>{item.label}</span>
    {/snippet}
  </Dropdown>
</div>

<style lang="css">
  .top-bar {
    display: flex;
    flex-direction: row;
    align-items: center;
    height: 60px;
    padding: 0 20px;
    gap: 30px;
    box-shadow: 0px 2px 0px 0px var(--shadow-color);

    .top-bar-tool,
    div {
      height: 28px;
    }

    a {
      flex-direction: row;
      font-weight: 400;
      font-size: 16px;
      color: var(--text-color);
      align-items: center;
    }

    .user-info,
    .language-selector {
      align-items: center;
      gap: 10px;
      cursor: pointer;
      font-size: 16px;

      span {
        align-items: center;
        color: var(--color);
        background-color: var(--background-color);
      }
      .user-info-avatar {
        border-radius: 50%;
        font-size: 20px;
        padding: 3px;
      }
    }

    .search {
      flex-grow: 1;
      gap: 10px;

      .search-icon {
        color: var(--color);
      }

      input[type="text"] {
        flex-grow: 1;
        font-size: 18px;
        outline: none;
        border: none;
        color: var(--text-color);

        &::placeholder {
          color: var(--placeholder-color);
        }
      }
    }
  }
</style>
