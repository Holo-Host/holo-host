<script lang="ts">
  import Card from "./card.svelte";

  const { children } = $props();
</script>

<div class="backdrop">
  <div class="modal">
    <Card>{@render children()}</Card>
  </div>
</div>

<style lang="css">
  .backdrop {
    display: flex;
    position: fixed;
    align-items: center;
    justify-content: center;
    top: 0;
    left: 0;
    width: 100vw;
    height: 100vh;
    background-color: rgba(0, 0, 0, 0.6);
    z-index: 100;
  }
  .modal {
    display: block;
    max-height: calc(100vh - 200px);
    overflow-y: auto;
  }
</style>
