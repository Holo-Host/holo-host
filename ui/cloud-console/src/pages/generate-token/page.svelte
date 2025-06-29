<script lang="ts">
  import * as z from "zod";
  import Button from "@/components/button.svelte";
  import DatePicker from "@/components/date-picker.svelte";
  import Input from "@/components/input.svelte";
  import Badge from "@/components/badge.svelte";
  import Card from "@/components/card.svelte";
  import Modal from "@/components/modal.svelte";
  import { defaultTheme } from "@/theme";
  import { host, request } from "@/api";

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
  let isDescriptionValid = $state(true);
  let expireAt = $state(new Date(Date.now() + 86400000 * 7));
  let permissionValue = $state("");
  let permissionsSelected = $state<string[]>(["all.all.self"]);
  let generatedToken = $state("");
  let showGeneratedToken = $state(false);

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

  async function generateApiToken() {
    const permissionObj = permissionsSelected.map((perm) => {
      const [resource, action, owner] = perm.split(".");
      return {
        resource,
        action,
        owner,
      };
    });
    const req = await request("/protected/v1/apikey", {
      method: "post",
      body: JSON.stringify({
        description,
        expire_at: expireAt.getTime(),
        permissions: permissionObj,
        version: "v1",
      }),
    });
    const data = await req.json();
    generatedToken = data["api_key"];
  }
  async function onCopyGeneratedToken(event: MouseEvent) {
    await navigator.clipboard.writeText(generatedToken);
  }
</script>

{#if generatedToken !== ""}
  <Modal>
    <div class="flex gap10 column">
      <span style:color={defaultTheme.colors.text.danger}>
        Copy this token to access the HOLO API. For security this will not be
        shown again.
      </span>
      <Input
        class="grow"
        type={showGeneratedToken ? "text" : "password"}
        value={generatedToken}
        readonly
      />
      <Button onclick={onCopyGeneratedToken}>Copy</Button>
      <Button variant="secondary" onclick={() => (generatedToken = "")}>
        Close
      </Button>
    </div>
  </Modal>
{/if}

<div class="page">
  <div class="header">
    <h1 class="header-title">API Tokens</h1>
  </div>
  <Card>
    <h2>Create a new personal access token</h2>
    <span style:color={defaultTheme.colors.text.subtext}>
      Personal access tokens function like ordinary Oauth access tokens
    </span>
    <div class="flex column gap10">
      <div class="flex gap10 grow">
        <Input
          class="grow"
          label="Name"
          bind:value={description}
          bind:isValid={isDescriptionValid}
          validator={z
            .string()
            .min(3, { message: '"Name" must be at least 3 characters long' })}
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
          Define permissions as a matrix of {"{resource}.{action}.{owner}"}.
          Check API docs for more details. By default the permission is set to
          "all.all.self
        </span>
        <a href={host}>view api docs</a>
      </div>
    </div>
    <div class="flex">
      <Button disabled={!isDescriptionValid} onclick={generateApiToken}>
        Generate
      </Button>
    </div>
  </Card>
</div>
