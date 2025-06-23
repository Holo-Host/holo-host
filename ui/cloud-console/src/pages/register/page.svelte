<script lang="ts">
  import EmailVerify from "./email-verify.svelte";
  import RegisterUser from "./register-user.svelte";

  const { query } = $props();
</script>

<div class="column gap10">
  {#if query.code}
    <RegisterUser code={query.code} email={query.email} />
  {:else}
    <EmailVerify />
  {/if}
</div>
