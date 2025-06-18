<script lang="ts">
  import * as z from "zod";
  import Button from "@/components/button.svelte";
  import DatePicker from "@/components/date-picker.svelte";
  import Input from "@/components/input.svelte";
  import Badge from "@/components/badge.svelte";
  import Card from "@/components/card.svelte";

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

  let permissionValue = $state("");
  let permissionsSelected = $state<string[]>(["all.all.self"]);
  function onPermissionAdd(value: string) {
    const perm = permissions.find((item) => item === value.trim());
    if (!perm) return;
    permissionsSelected.push(perm);
    permissionValue = "";
  }
  function onPermissionRemove(value: string) {
    permissionsSelected = permissionsSelected.filter((item) => item !== value);
  }
  function onPermissionKeyDown(e: KeyboardEvent) {
    if (e.key === "Enter") {
      if (permissionValue.split(".").length !== 3) return;

      permissionsSelected.push(permissionValue);
      permissionValue = "";
    }
  }
</script>

<div class="page">
  <div class="header">
    <h1 class="header-title">API Tokens</h1>
  </div>
  <Card>
    <h2>Create a new personal access token</h2>
    <div class="flex column gap10">
      <div class="flex gap10 grow">
        <Input grow label="Name" validator={z.string().min(3)} />
        <div style:margin-top="25px">
          <DatePicker value={new Date(Date.now() + 86400000 * 7)} />
        </div>
      </div>
      <div class="flex wrap gap10">
        {#each permissionsSelected as perm}
          <Badge label={perm} onClick={() => onPermissionRemove(perm)} />
        {/each}
      </div>
      <Input
        label="Permissions"
        autocomplete={permissions}
        onChange={onPermissionAdd}
        onKeyDown={onPermissionKeyDown}
        bind:value={permissionValue}
      />
      <span>
        Permissions are structured as {"{resource}"}.{"{action}"}.{"{owner}"}
        Therefor, `workload.create.self` will let you create workloads that are owned
        by the logged in user. By default `all.all.self` refers to all permissions
        that the user has.
      </span>
    </div>
    <div class="flex">
      <Button>Generate</Button>
    </div>
  </Card>
</div>
