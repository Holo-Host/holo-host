<script lang="ts">
  import * as z from "zod";
  import { defaultTheme } from "@/theme";

  type BaseProp = {
    id?: string;
    placeholder?: string;
    label?: string;
    grow?: boolean;
    onKeyDown?: (e: KeyboardEvent) => void;
    validator?: z.ZodType;
    isValid?: boolean;
  };

  type TextProp = BaseProp & {
    type?: "text" | "password" | "email";
    value?: string;
    autocomplete?: string[];
    onChange?: (value: string) => void;
  };

  type NumberProp = BaseProp & {
    type?: "number";
    value?: number;
    onChange?: (value: number) => void;
  };

  type Prop = TextProp | NumberProp;
  let {
    value = $bindable(),
    isValid = $bindable(false),
    ...props
  }: Prop = $props();
  let hasChangedInput = $state(false);
  const validationError = $derived.by(() => {
    if (!hasChangedInput) return false;
    if (!props.validator) return false;
    if (value === null) return false;
    try {
      props.validator.parse(value);
      return false;
    } catch (e) {
      if (e instanceof z.ZodError) {
        if (e.errors.length > 0) {
          return e.errors[0].message;
        }
        return "unknown error";
      }
      return "unknown error";
    }
  });
  let isFocused = $state(false);
  const autocomplete = $derived.by(() => {
    if (props.type === "number") return [];
    if (!props.autocomplete) return [];

    let val = String(value);
    for (const item of props.autocomplete) {
      val = val.replaceAll(item, "");
    }
    val = val.trim();

    return props.autocomplete
      .filter((item) => item !== value && item.startsWith(val))
      .slice(0, 4);
  });

  function onInput(e: Event) {
    const target = e.target as HTMLInputElement;
    if (!target) return;
    hasChangedInput = true;
    isValid = validationError === false;
    switch (props.type) {
      case "number":
        props.onChange?.(Number(target.value));
        break;
      default:
        props.onChange?.(target.value);
        break;
    }
  }
  function onFocus() {
    isFocused = true;
  }
  function onFocusOut() {
    isFocused = false;
  }
  function onAutocompleteSelected(e: MouseEvent, item: string) {
    e.preventDefault();
    if (props.type === "number") return;
    value = item;
    props.onChange?.(item);
  }
</script>

<div class="flex column gap5" class:grow={props.grow}>
  <label for={props.id}>
    {props.label}
  </label>
  <input
    id={props.id}
    name={props.id}
    type={props.type}
    placeholder={props.placeholder}
    oninput={onInput}
    onfocus={onFocus}
    onfocusout={onFocusOut}
    onkeydown={props.onKeyDown}
    style:--border-color={defaultTheme.colors.border}
    style:--error-border-color={defaultTheme.colors.danger}
    class:error={!!validationError}
    bind:value
  />
  {#if props.type !== "number" && isFocused && props.autocomplete && autocomplete.length > 0}
    <div class="autocomplete-container">
      <div
        class="autocomplete"
        style:--border-color={defaultTheme.colors.border}
        style:align-items="start"
      >
        {#each autocomplete as item}
          <button
            class="cursor-pointer autocomplete-item"
            style:--hover-background-color={defaultTheme.colors.background
              .primary}
            onmousedown={(e) => onAutocompleteSelected(e, item)}
          >
            {item}
          </button>
        {/each}
      </div>
    </div>
  {/if}

  {#if !!validationError}
    <span class="error" style:--text-color={defaultTheme.colors.text.danger}>
      {validationError}
    </span>
  {:else}
    <span>&nbsp;</span>
  {/if}
</div>

<style lang="css">
  .error {
    color: var(--text-color);
  }
  input {
    font-size: 20px;
    padding: 10px 20px;
    border: 1px solid var(--border-color);
  }
  .error {
    border: 1px solid var(--error-border-color);
  }

  .autocomplete-container {
    display: block;
    position: relative;
    width: 0;
    height: 0;

    .autocomplete {
      display: flex;
      flex-direction: column;
      border: 1px solid var(--border-color);
      background-color: white;
      width: fit-content;
      top: 0;
      left: 0;

      .autocomplete-item {
        width: 100%;
        align-items: start;
        padding-left: 20px;
        padding-right: 20px;
        padding-top: 10px;
        padding-bottom: 10px;

        &:hover {
          background-color: var(--hover-background-color);
          color: white;
        }
      }
    }
  }
</style>
