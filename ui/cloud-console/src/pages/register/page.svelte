<script lang="ts">
  import EmailVerify from "./email-verify.svelte";
  import RegisterUser from "./register-user.svelte";
  import SharedPattern from "@/components/login/shared-pattern.svelte";

  const { query } = $props();
</script>

<div class="justify-center" style:background-color="white" style:height="100%">
  <div
    class="column gap10"
    style:width="400px"
    style:margin-bottom="100px"
    style:z-index="2"
  >
    {#if query.code}
      <RegisterUser code={query.code} email={query.email} />
    {:else}
      <EmailVerify />
    {/if}
  </div>
</div>
<SharedPattern />
