<script lang="ts">
  import { defaultTheme } from "@/theme";

  type Prop = {
    label: string;
    variant?: "defualt" | "success" | "warning" | "danger";
    onClick?: () => void;
  };
  const props: Prop = $props();

  let backgroundColor = $derived.by(() => {
    switch (props.variant) {
      case "success":
        return defaultTheme.colors.success;
      case "warning":
        return defaultTheme.colors.warning;
      case "danger":
        return defaultTheme.colors.danger;
    }
    return defaultTheme.colors.background.default;
  });
</script>

<button
  class="badge"
  class:cursor-pointer={!!props.onClick}
  style:--border-color={defaultTheme.colors.border}
  style:--background-color={backgroundColor}
  onclick={() => props.onClick()}
>
  {props.label}
</button>

<style lang="css">
  .badge {
    display: flex;
    padding: 10px 15px;
    border: 1px solid var(--border-color);
    background-color: var(--background-color);
    border-radius: 20px;
  }
</style>
