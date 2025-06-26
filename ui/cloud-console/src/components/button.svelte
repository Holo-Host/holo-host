<script lang="ts">
  import { defaultTheme } from "../theme";

  type Prop = {
    children?: any;
    href?: string;
    onclick?: (event: MouseEvent) => void;
    variant?: "primary" | "secondary" | "danger";
    disabled?: boolean;
  };
  const { children, href, variant, onclick, disabled }: Prop = $props();

  const backgroundColor = $derived.by(() => {
    switch (variant) {
      default:
      case "primary":
        return defaultTheme.colors.background.primary;
      case "secondary":
        return defaultTheme.colors.background.default;
      case "danger":
        return defaultTheme.colors.background.danger;
    }
  });
  const textColor = $derived.by(() => {
    switch (variant) {
      default:
      case "primary":
        return defaultTheme.colors.text.white;
      case "secondary":
        return defaultTheme.colors.text.black;
      case "danger":
        return defaultTheme.colors.text.white;
    }
  });
</script>

<a
  class="button"
  class:disabled
  {href}
  onclick={disabled ? null : onclick}
  style:--background-color={backgroundColor}
  style:--text-color={textColor}
  style:--shadow={defaultTheme.shadow}
  style:--shadow-color={defaultTheme.colors.shadow}
  style:--disabled-background-color={defaultTheme.colors.background.disabled}
>
  {@render children()}
</a>

<style lang="css">
  .button {
    background-color: var(--background-color);
    color: var(--text-color);
    border: none;
    padding: 10px 20px;
    cursor: pointer;
    font-size: 16px;
    align-items: center;
    justify-content: center;
    text-align: center;
    font-weight: 500;
    font-size: 20px;
    transition: 0.3s ease;

    &:hover {
      box-shadow: var(--shadow) rgba(0, 0, 0, 0.3);
    }
  }

  .disabled {
    background-color: var(--disabled-background-color);
    cursor: default;

    &:hover {
      box-shadow: none;
    }
  }
</style>
