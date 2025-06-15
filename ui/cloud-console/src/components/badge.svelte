<script lang="ts">
  import { defaultTheme } from "@/theme";

  type Prop = {
    label: string;
    variant?: "defualt" | "success" | "warning" | "danger";
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

<div
  class="badge"
  style:--border-color={defaultTheme.colors.border}
  style:--background-color={backgroundColor}
>
  {props.label}
</div>

<style lang="css">
  .badge {
    display: flex;
    padding: 10px 15px;
    border: 1px solid var(--border-color);
    background-color: var(--background-color);
    border-radius: 20px;
  }
</style>
