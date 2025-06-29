<script lang="ts">
  import Button from "@/components/button.svelte";
  import Input from "@/components/input.svelte";
  import { login } from "@/api";
  import { z } from "zod";
  import SharedPattern from "@/components/login/shared-pattern.svelte";

  let email = $state("");
  let password = $state("");
  let isEmailValid = $state(false);
  let isPasswordValid = $state(false);

  async function onLogin() {
    login(email, password);
  }
</script>

<div class="login">
  <div class="column" style:width="400px" style:margin-top="100px">
    <Input
      type="email"
      label="email"
      placeholder="john.doe@example.com"
      bind:value={email}
      bind:isValid={isEmailValid}
      validator={z.string().email()}
    />
    <Input
      type="password"
      label="password"
      bind:value={password}
      bind:isValid={isPasswordValid}
      validator={z.string().min(8)}
    />
    <Button disabled={!isEmailValid || !isPasswordValid} onclick={onLogin}>
      Login
    </Button>
    <div class="grow justify-space-between" style:margin-top="10px">
      <a href="/forgot-password">Forgot Password</a>
      <a href="/register">Signup</a>
    </div>
  </div>
</div>

<SharedPattern />

<style lang="css">
  .login {
    background-color: white;
    justify-content: center;
    height: 100%;
  }
</style>
