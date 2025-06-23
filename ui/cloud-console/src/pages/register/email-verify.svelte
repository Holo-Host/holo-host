<script lang="ts">
  import * as z from "zod";
  import { requestNoAuth } from "@/api";
  import Button from "@/components/button.svelte";
  import Input from "@/components/input.svelte";

  let email = $state("");
  let error = $state<string | null>(null);
  let verified = $state(false);
  let loading = $state(false);

  async function onVerifyEmail() {
    loading = true;
    const req = await requestNoAuth("/public/v1/auth/email-verify", {
      method: "post",
      body: JSON.stringify({
        email,
      }),
    });
    loading = false;
    if (!req.ok) {
      error = "failed to verify email";
      console.error("failed to verify email");
      return;
    }
    verified = true;
  }
</script>

<div class="column gap10" style:margin-top="100px">
  {#if verified}
    <span>Please check your email for a link.</span>
  {:else if loading}
    <span>Loading...</span>
  {:else}
    <Input
      type="email"
      label="email"
      placeholder="john.doe@example.com"
      validator={z.string().email()}
      bind:value={email}
    />
    <Button onclick={onVerifyEmail}>Verify Email</Button>
  {/if}
</div>
