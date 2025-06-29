<script lang="ts">
  import * as z from "zod";
  import { requestNoAuth } from "@/api";
  import Button from "@/components/button.svelte";
  import Input from "@/components/input.svelte";

  let email = $state("");
  let isEmailValid = $state(false);
  let error = $state<string | null>(null);
  let verified = $state(false);
  let loading = $state(false);
  let code = $state("");
  let isCodeValid = $state(false);

  async function onVerifyEmail() {
    loading = true;
    const req = await requestNoAuth("/public/v1/auth/email-verify", {
      method: "post",
      body: JSON.stringify({
        email,
        check_account_exists: true,
        redirect_url: `${window.location.host}/register`,
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
  async function onVerifyCode() {
    loading = true;
    const req = await requestNoAuth(`/public/v1/auth/email-verify-check`, {
      method: "post",
      body: JSON.stringify({
        email,
        email_verification_code: code,
      }),
    });
    loading = false;
    if (!req.ok) {
      error = "failed to verify registration code";
      console.error("failed to verify registration code");
      return;
    }
    window.location.href = `/register?code=${code}`;
  }
</script>

<div class="column gap10 align-center" style:margin-top="100px">
  <h2 style:margin-bottom="20px">Sign up to HOLO</h2>
  {#if verified}
    <Input
      class="w100"
      type="text"
      label="Code"
      placeholder="123456"
      validator={z.string().min(6).max(6)}
      bind:value={code}
      bind:isValid={isCodeValid}
    />
    <Button disabled={!isCodeValid} onclick={onVerifyCode}>Continue</Button>
  {:else if loading}
    <span>Loading...</span>
  {:else}
    <Input
      class="w100"
      type="email"
      label="email"
      placeholder="john.doe@example.com"
      validator={z.string().email()}
      bind:value={email}
      bind:isValid={isEmailValid}
    />
    <Button class="w100" disabled={!isEmailValid} onclick={onVerifyEmail}>
      Verify Email
    </Button>
    <div class="grow justify-space-between w100" style:margin-top="10px">
      <a href="/forgot-password">Forgot Password</a>
      <a href="/login">Login</a>
    </div>
  {/if}
</div>
