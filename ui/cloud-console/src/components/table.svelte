<script module lang="ts">
  export type Column<T> = {
    key: string;
    label?: string;
    width?: string;
    renderer?: Snippet<[T]>;
  };
</script>

<script lang="ts">
  import { defaultTheme } from "@/theme";
  import type { Snippet } from "svelte";
  type Prop<T> = {
    columns: Column<T>[];
    rows: T[];
  };
  const props: Prop<unknown> = $props();
</script>

<table>
  <thead>
    <tr>
      {#each props.columns as column}
        <th
          style:width={column.width}
          style:--border-color={defaultTheme.colors.border}
          style:--background-color={defaultTheme.colors.background.default}
          >{column.label ?? column.key}</th
        >
      {/each}
    </tr>
  </thead>
  <tbody>
    {#each props.rows as row}
      <tr>
        {#each props.columns as column}
          <td
            style:--border-color={defaultTheme.colors.border}
            style:--background-color={defaultTheme.colors.background.card}
          >
            {#if !!column.renderer}
              {@render column.renderer(row)}
            {:else}
              {row[column.key]}
            {/if}
          </td>
        {/each}
      </tr>
    {/each}
  </tbody>
</table>

<style lang="css">
  table {
    border-spacing: 0;

    thead {
      font-weight: 500;
    }

    tr {
      th {
        border-top: 1px solid var(--border-color);
      }

      th,
      td {
        text-align: left;
        padding: 20px 30px;
        border-bottom: 1px solid var(--border-color);
        background-color: var(--background-color);

        &:first-child {
          border-left: 1px solid var(--border-color);
        }

        &:last-child {
          border-right: 1px solid var(--border-color);
        }
      }
    }
  }
</style>
