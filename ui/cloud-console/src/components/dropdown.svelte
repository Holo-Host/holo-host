<script lang="ts">
  import type { Snippet } from "svelte";
  import { defaultTheme } from "../theme";
  import { computePosition } from "@floating-ui/dom";
  //todo: gray out drawer menu options
  type Prop<T> = {
    children: any;
    items: T[];
    itemFocusKey?: string;
    filterFocusItems?: (item: T) => boolean;
    itemTemplate?: Snippet<[T]>;
    onItemSelected?: (item: T) => void;
    class?: string;
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
    if (dropdownEl && dropdownMenuEl) {
      computePosition(dropdownEl, dropdownMenuEl).then(({ x, y }) => {
        Object.assign(dropdownMenuEl.style, {
          left: `${x}px`,
          top: `${y}px`,
        });
      });
    }
  });

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

  $effect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (!isOpen) return;
      const key = event.key.toLowerCase();

      const matchedItemIndex = props.items.findIndex((item) => {
        if (props.filterFocusItems && !props.filterFocusItems(item))
          return false;
        if (typeof item === "object" && !props.itemFocusKey) {
          console.error(
            "for dropdown items with object type, itemFocusKey must be specified"
          );
          return false;
        }
        if (typeof item === "string")
          switch (typeof item) {
            case "string":
              return item.toLowerCase().startsWith(key);
            case "object":
              if (!item) return false;
              if (!(props.itemFocusKey in item)) return false;
              const itemValue = item[props.itemFocusKey] as string;
              if (itemValue !== "string") return false;
              return itemValue.toLowerCase().startsWith(key);
          }

        return false;
      });

      if (matchedItemIndex === -1) return;

      const items = dropdownMenuEl.querySelectorAll(".dropdown-item");
      const matchedItem = items[matchedItemIndex] as HTMLElement;
      matchedItem.focus();
    };

    addEventListener("keydown", handleKeyDown);
    return () => removeEventListener("keydown", handleKeyDown);
  });
</script>

<div class={`container ${props.class ?? ""}`}>
  <button
    class="dropdown"
    bind:this={dropdownEl}
    onclick={() => (isOpen = !isOpen)}
  >
    {@render props.children()}
  </button>

  {#if isOpen}
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
  {/if}
</div>

<style lang="css">
  .dropdown {
    cursor: pointer;
    flex-grow: 1;
  }
  .dropdown-menu {
    display: flex;
    position: absolute;
    width: min-content;
    top: 0;
    left: 0;
    max-height: 300px;

    margin-top: 30px;
    flex-direction: column;
    background: white;
    border: 1px solid #ccc;
    z-index: 1000;
    overflow-y: auto;

    .dropdown-item {
      padding: 10px 20px;
      cursor: pointer;

      &:hover,
      &:focus {
        background-color: var(--hover-color);
      }
    }
  }
</style>
