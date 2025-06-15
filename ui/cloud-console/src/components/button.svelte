<script lang="ts">
  import { defaultTheme } from "../theme";

  type Prop = {
    children?: any;
    href?: string;
    onclick?: (event: MouseEvent) => void;
    variant?: "primary" | "secondary";
  };
  const { children, href, variant, onclick }: Prop = $props();

  const backgroundColor = $derived(
    variant === "secondary"
      ? defaultTheme.colors.background.default
      : defaultTheme.colors.background.primary
  );
  const textColor = $derived(
    variant === "secondary"
      ? defaultTheme.colors.text.black
      : defaultTheme.colors.text.white
  );
</script>

<a
  class="button"
  {href}
  {onclick}
  style:--background-color={backgroundColor}
  style:--text-color={textColor}
  style:--shadow={defaultTheme.shadow}
  style:--shadow-color={defaultTheme.colors.shadow}
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
</style>
