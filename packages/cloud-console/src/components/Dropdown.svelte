<script lang="ts">
  import { defaultTheme } from "../theme";

  const { children, items, itemTemplate, onItemSelected } = $props();
  let isOpen = $state(false);

  function handleItemClick(item) {
    isOpen = false;
    onItemSelected(item);
  }

  $effect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (!isOpen) return;
      const target = event.target as HTMLElement;
      const dropdown = document.querySelector(".dropdown");
      const dropdownMenu = document.querySelector(".dropdown-menu");
      if (
        target.className === "dropdown" ||
        dropdown.contains(target) ||
        dropdownMenu.contains(target)
      ) {
        return;
      }
      isOpen = false;
    };

    addEventListener("click", handleClickOutside);

    return () => {
      removeEventListener("click", handleClickOutside);
    };
  });
</script>

<div class="container">
  {#if isOpen}
    <div class="dropdown-container">
      <div class="dropdown-menu">
        {#each items as item}
          <button
            style:--hover-color={defaultTheme.colors.background.secondary}
            class="dropdown-item"
            onclick={() => handleItemClick(item)}
          >
            {@render itemTemplate(item)}
          </button>
        {/each}
      </div>
    </div>
  {/if}

  <button class="dropdown" onclick={() => (isOpen = !isOpen)}>
    {@render children()}
  </button>
</div>

<style lang="css">
  .dropdown {
    cursor: pointer;
  }
  .dropdown-container {
    display: block;
    position: relative;
    width: 0;
    height: 0;

    .dropdown-menu {
      display: flex;
      position: relative;
      margin-top: 30px;
      flex-direction: column;
      background: white;
      border: 1px solid #ccc;
      width: min-content;
      z-index: 1000;

      .dropdown-item {
        padding: 10px 20px;
        cursor: pointer;

        &:hover {
          background-color: var(--hover-color);
        }
      }
    }
  }
</style>
