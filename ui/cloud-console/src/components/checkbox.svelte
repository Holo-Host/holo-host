<script lang="ts">
  type Prop = {
    value?: boolean;
    label?: string;
    onChange?: (value: boolean) => void;
  };
  let { value = $bindable(false), ...props }: Prop = $props();

  function onChange(e: Event) {
    const target = e.target as HTMLInputElement;
    if (!target) return;
    props.onChange?.(target.checked);
  }
</script>

<div class="flex gap5">
  <input type="checkbox" bind:checked={value} onchange={onChange} />
  <span>{props.label}</span>
</div>
