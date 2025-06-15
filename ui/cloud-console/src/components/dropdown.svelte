<script lang="ts">
  import type { Snippet } from "svelte";
  import { defaultTheme } from "../theme";

  type Prop<T> = {
    children: any;
    items: T[];
    itemTemplate?: Snippet<[T]>;
    onItemSelected?: (item: T) => void;
  };
  const props: Prop<unknown> = $props();
  let isOpen = $state(false);

  function handleItemClick(item) {
    isOpen = false;
    props.onItemSelected?.(item);
  }

  let dropdownEl: HTMLElement = $state(null);
  let dropdownMenuEl: HTMLElement = $state(null);

  $effect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (!isOpen) return;
      const target = event.target as HTMLElement;
      if (
        dropdownEl &&
        dropdownMenuEl &&
        (dropdownEl.contains(target) || dropdownMenuEl.contains(target))
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
      <div class="dropdown-menu" bind:this={dropdownMenuEl}>
        {#each props.items as item}
          <button
            style:--hover-color={defaultTheme.colors.background.secondary}
            class="dropdown-item"
            onclick={() => handleItemClick(item)}
          >
            {#if !!props.itemTemplate}
              {@render props.itemTemplate(item)}
            {:else}
              {item}
            {/if}
          </button>
        {/each}
      </div>
    </div>
  {/if}

  <button
    class="dropdown"
    bind:this={dropdownEl}
    onclick={() => (isOpen = !isOpen)}
  >
    {@render props.children()}
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
