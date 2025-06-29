<script lang="ts">
  import Card from "@/components/card.svelte";
  import Button from "@/components/button.svelte";
  import Badge from "@/components/badge.svelte";
  import Dropdown from "@/components/dropdown.svelte";
  import { deleteApiKey, getApiKeys } from "./data";
  import type { ApiKey } from "./types";
  import Table, { type Column } from "@/components/table.svelte";
  import Modal from "@/components/modal.svelte";
  import { defaultTheme } from "@/theme";

  const columns: Column<ApiKey>[] = [
    {
      key: "name",
      label: "Name",
    },
    {
      key: "permissions",
      label: "Permissions",
      renderer: PermissionRenderer,
    },
    {
      key: "expiresAt",
      label: "Expires At",
      width: "130px",
      renderer: ExpiryRenderer,
    },
    {
      key: "",
      width: "50px",
      renderer: ActionsRenderer,
    },
  ];
  let apikeyToDelete = $state<ApiKey | null>(null);

  let data: ApiKey[] = $state([]);
  $effect(() => {
    getApiKeys().then((d) => (data = d));
  });

  function onActionSelected(item: ApiKey, action: string) {
    switch (action) {
      case "delete":
        apikeyToDelete = item;
        break;
    }
  }
  function onDeleteApikey() {
    deleteApiKey(apikeyToDelete);
    data = data.filter((d) => d.id !== apikeyToDelete.id);
    apikeyToDelete = null;
  }
</script>

{#if !!apikeyToDelete}
  <Modal>
    <div
      class="column justify-center align-center gap20 text-center"
      style:margin="40px"
    >
      <div class="hex-container">
        <span class="icons-outlined">delete</span>
        <span class="hex" style:--color={defaultTheme.colors.background.danger}
        ></span>
      </div>
      <span>Are you sure you want to delete this API token?</span>
      <div class="justify-center align-center gap20" style:z-index="2">
        <Button variant="danger" onclick={onDeleteApikey}>Delete</Button>
        <Button variant="secondary" onclick={() => (apikeyToDelete = null)}>
          Cancel
        </Button>
      </div>
    </div>
  </Modal>
{/if}

<div class="page">
  <div class="header">
    <h1 class="header-title">API Tokens</h1>
    {#if data.length > 0}
      <Button href="/generate-token">Generate new token</Button>
    {/if}
  </div>
  <Card>
    {#if data.length > 0}
      <Table {columns} rows={data} />
    {:else}
      <div
        class="column justify-center align-center gap20"
        style:margin-top="100px"
        style:margin-bottom="100px"
      >
        <div class="hex-container">
          <span class="icons-outlined">key</span>
          <span
            class="hex"
            style:--color={defaultTheme.colors.background.primary}
          ></span>
        </div>
        <h2>Create a personal access token</h2>
        <p>
          Personal access tokens function like ordinary OAuth access tokens.
        </p>
        <Button href="/generate-token">Generate new token</Button>
      </div>
    {/if}
  </Card>
</div>

{#snippet PermissionRenderer(row: ApiKey)}
  <div class="flex gap10 wrap">
    {#each row.permissions as permission}
      <Badge label={permission} />
    {/each}
  </div>
{/snippet}

{#snippet ExpiryRenderer(row: ApiKey)}
  <Badge
    variant={row.expiresAt.getTime() > Date.now() ? "success" : "danger"}
    label={row.expiresAt.toLocaleString(navigator.language, {
      day: "numeric",
      month: "short",
      year: "numeric",
    })}
  />
{/snippet}

{#snippet ActionsRenderer(row: ApiKey)}
  <Dropdown
    items={["delete"]}
    onItemSelected={(action: string) => onActionSelected(row, action)}
  >
    <span class="icons-outlined">more_horiz</span>
  </Dropdown>
{/snippet}

<style lang="css">
  .header {
    .header-title {
      flex-grow: 1;
    }
  }

  .hex-container {
    display: block;
    margin: 0;
    height: 125px;
    text-align: left;

    .hex {
      position: relative;
      top: 0;
      color: var(--color);
    }
    .icons-outlined {
      position: relative;
      width: 0;
      height: 0;
      top: 20px;
      left: 47px;
      font-size: 60px;
      z-index: 1;
      color: white;
    }
  }
</style>
