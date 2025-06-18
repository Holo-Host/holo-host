<script lang="ts">
  import Card from "@/components/card.svelte";
  import Button from "@/components/button.svelte";
  import Badge from "@/components/badge.svelte";
  import Dropdown from "@/components/dropdown.svelte";
  import { deleteApiKey, getApiKeys } from "./data";
  import type { ApiKey } from "./types";
  import Table, { type Column } from "@/components/table.svelte";

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

  const data = getApiKeys();

  function onActionSelected(item: ApiKey, action: string) {
    switch (action) {
      case "delete":
        deleteApiKey(item);
        break;
    }
  }
</script>

<div class="page">
  <div class="header">
    <h1 class="header-title">API Tokens</h1>
    <Button href="/generate-token">Generate new token</Button>
  </div>
  <Card>
    <Table {columns} rows={data} />
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
</style>
