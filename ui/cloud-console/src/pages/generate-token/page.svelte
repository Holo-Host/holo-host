<script lang="ts">
  import * as z from "zod";
  import Button from "@/components/button.svelte";
  import DatePicker from "@/components/date-picker.svelte";
  import Input from "@/components/input.svelte";
  import Badge from "@/components/badge.svelte";
  import Card from "@/components/card.svelte";
  import { defaultTheme } from "@/theme";
  import { request } from "@/api";

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

  let description = $state("");
  let expireAt = $state(new Date(Date.now() + 86400000 * 7));
  let permissionValue = $state("");
  let permissionsSelected = $state<string[]>(["all.all.self"]);
  function onPermissionAdd(value: string) {
    const perm = permissions.find((item) => item === value.trim());
    if (!perm) return;
    permissionsSelected.push(perm);
    description = "";
  }
  function onPermissionRemove(value: string) {
    permissionsSelected = permissionsSelected.filter((item) => item !== value);
  }
  function onPermissionKeyDown(e: KeyboardEvent) {
    if (e.key === "Enter") {
      if (description.split(".").length !== 3) return;

      permissionsSelected.push(description);
      description = "";
    }
  }

  function generateApiToken() {
    const permissionObj = permissions.map((perm) => {
      const [resource, action, owner] = perm.split(".");
      return {
        resource,
        action,
        owner,
      };
    });
    request("/protected/v1/apikey", {
      method: "post",
      body: JSON.stringify({
        description,
        expire_at: expireAt.getTime(),
        permissions: permissionObj,
        version: "v1",
      }),
    });
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
        <Input
          grow
          label="Name"
          validator={z
            .string()
            .min(3, { message: '"Name" must be at least 3 characters long' })}
          bind:value={description}
        />
        <div style:margin-top="25px">
          <DatePicker bind:value={expireAt} />
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
      <div
        class="flex column gap10"
        style:color={defaultTheme.colors.text.subtext}
      >
        <span>
          Permissions are structured as {"{resource}"}.{"{action}"}.{"{owner}"}
        </span>
        <span>
          Therefor, `workload.create.self` will let you create workloads that
          are owned by the logged in user.
        </span>
        <span>
          By default `all.all.self` refers to all permissions that the user has.
        </span>
      </div>
    </div>
    <div class="flex">
      <Button onclick={generateApiToken}>Generate</Button>
    </div>
  </Card>
</div>
