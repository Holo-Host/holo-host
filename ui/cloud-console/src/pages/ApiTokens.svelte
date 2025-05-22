<script lang="ts">
  import Card from "../components/Card.svelte";
  import Button from "../components/Button.svelte";
</script>

<div class="page">
  <div class="header">
    <h1 class="header-title">API Tokens</h1>
    <Button onclick={() => console.log("test")}>Generate new token</Button>
  </div>
  <Card>Test</Card>
</div>

<style lang="css">
  .header {
    .header-title {
      flex-grow: 1;
    }
  }
</style>
