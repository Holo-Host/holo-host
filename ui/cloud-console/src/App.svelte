<script lang="ts">
  import Drawer from "./components/drawer.svelte";
  import Topbar from "./components/topbar.svelte";
  import { routeComponent } from "./router";
</script>

{#if $routeComponent}
  {#if $routeComponent.showDrawer}
    <Drawer path={$routeComponent.path} />
  {/if}
  {#if $routeComponent.showHeader}
    <div
      style="flex-direction: column; flex-grow: 1;"
      style:height="calc(100% - 60px)"
    >
      <Topbar />
      <div class="page-content">
        <svelte:component
          this={$routeComponent.component}
          params={$routeComponent.params}
          query={$routeComponent.query}
          hash={$routeComponent.hash}
        />
      </div>
    </div>
  {:else}
    <div class="page-content">
      <svelte:component
        this={$routeComponent.component}
        params={$routeComponent.params}
        query={$routeComponent.query}
        hash={$routeComponent.hash}
      />
    </div>
  {/if}
{/if}

<style lang="css">
  .page-content {
    flex-direction: column;
    min-height: 100%;
  }
</style>
