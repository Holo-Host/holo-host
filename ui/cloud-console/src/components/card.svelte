<script lang="ts">
  import { defaultTheme } from "../theme";
  const { children } = $props();
</script>

<div
  class="card"
  style:--background-color={defaultTheme.colors.background.card}
  style:--shadow-color={defaultTheme.colors.shadow}
  style:--shadow={defaultTheme.shadow}
>
  {@render children()}
</div>

<style lang="css">
  .card {
    display: flex;
    flex-direction: column;
    background-color: var(--background-color);
    padding: 30px;
    gap: 16px;
    box-shadow: var(--shadow) var(--shadow-color);
  }
</style>
