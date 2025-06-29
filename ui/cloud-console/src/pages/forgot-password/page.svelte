<script lang="ts">
  import * as z from "zod";
  import { requestNoAuth } from "@/api";
  import Button from "@/components/button.svelte";
  import Input from "@/components/input.svelte";
  import SharedPattern from "@/components/login/shared-pattern.svelte";

  let loading = $state(false);
  let verified = $state(false);
  let done = $state(false);

  let email = $state("");
  let isEmailValid = $state(false);

  let code = $state("");
  let isCodeValid = $state(false);

  let password = $state("");
  let isPasswordValid = $state(false);
  let confirmPassword = $state("");
  let isConfirmPasswordValid = $state(false);

  async function onVerifyEmail() {
    loading = true;
    const req = await requestNoAuth(`/public/v1/auth/email-verify`, {
      method: "post",
      body: JSON.stringify({
        email,
        check_account_exists: false,
      }),
    });
    loading = false;
    if (!req.ok) {
      console.error("failed to verify email");
      return;
    }
    verified = true;
  }

  async function onResetPassword() {
    loading = true;
    const req = await requestNoAuth(`/public/v1/auth/forgot-password`, {
      method: "post",
      body: JSON.stringify({
        email,
        email_confirmation_code: code,
        new_password: password,
      }),
    });
    loading = false;
    if (!req.ok) {
      console.error("failed to reset password");
      return;
    }
    done = true;
  }
</script>

<div class="justify-center" style:background-color="white" style:height="100%">
  <div class="column gap10" style:margin-top="100px">
    {#if done}
      <span>Password updated successfully</span>
      <a href="/login">Login</a>
    {:else if loading}
      <span>Loading...</span>
    {:else}
      <Input
        type="email"
        label="email"
        placeholder="john.doe@example.com"
        bind:value={email}
        bind:isValid={isEmailValid}
        validator={z.string().email()}
      />
      {#if !verified}
        <Button disabled={!isEmailValid} onclick={onVerifyEmail}>
          Verify Email
        </Button>
      {:else}
        <Input
          type="text"
          label="Code"
          placeholder="123456"
          bind:value={code}
          bind:isValid={isCodeValid}
          validator={z.string().min(6).max(6)}
        />
        <Input
          type="password"
          label="Password"
          placeholder=""
          bind:value={password}
          bind:isValid={isPasswordValid}
          validator={z.string().min(8)}
        />
        <Input
          type="password"
          label="Renter Password"
          placeholder=""
          bind:value={confirmPassword}
          bind:isValid={isConfirmPasswordValid}
          validator={z.string().refine((val) => val === password, {
            message: "Password does not match",
          })}
        />
        <Button disabled={!isCodeValid} onclick={onResetPassword}>
          Update Password
        </Button>
      {/if}
      <div class="grow justify-space-between" style:margin-top="10px">
        <a href="/login">Login</a>
        <a href="/register">Signup</a>
      </div>
    {/if}
  </div>
</div>

<SharedPattern />
