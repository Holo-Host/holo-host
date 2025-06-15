<script lang="ts">
  import * as z from "zod";
  import Button from "@/components/button.svelte";
  import DatePicker from "@/components/date-picker.svelte";
  import Input from "@/components/input.svelte";
  import Modal from "@/components/modal.svelte";
  import Checkbox from "@/components/checkbox.svelte";

  type Prop = {
    visible: boolean;
  };
  let { visible = $bindable() }: Prop = $props();

  export const resources = ["api_key", "blob", "workload"];
  export const actions = ["create", "read", "update", "delete"];
  const permissions = $derived.by(() => {
    const permissions: string[] = ["all.all.self"];
    for (const resource of resources) {
      permissions.push(`${resource}.all.self`);
      for (const action of actions) {
        permissions.push(`${resource}.${action}.self`);
      }
    }
    return permissions;
  });
</script>

{#if visible}
  <Modal>
    <h2>Create a new personal access token</h2>
    <div class="flex column gap10">
      <Input label="Name" validator={z.string().min(3)} />
      <Input label="Permissions" autocomplete={permissions} />
      <span>Expires At</span>
      <DatePicker />
      <div class="flex gap10">
        <Button variant="secondary" onclick={() => (visible = false)}>
          Cancel
        </Button>
        <Button>Generate</Button>
      </div>
    </div>
  </Modal>
{/if}
