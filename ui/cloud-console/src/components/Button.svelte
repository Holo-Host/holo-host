<script lang="ts">
  import { defaultTheme } from "../theme";
  const {
    children,
    href,
    onclick,
  }: {
    children?: any;
    href?: string;
    onclick?: (event: MouseEvent) => void;
  } = $props();
</script>

<a
  class="button"
  {href}
  {onclick}
  style:--background-color={defaultTheme.colors.background.primary}
  style:--text-color={defaultTheme.colors.text.white}
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
