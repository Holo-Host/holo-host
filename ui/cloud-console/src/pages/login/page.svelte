<script lang="ts">
  import DiamondPattern from "@/components/diamond-pattern.svelte";
  import Button from "@/components/button.svelte";
  import Input from "@/components/input.svelte";
  import { login } from "@/api";
  import { z } from "zod";

  let email = $state("");
  let password = $state("");
  let isEmailValid = $state(false);
  let isPasswordValid = $state(false);

  async function onLogin() {
    login(email, password);
  }
</script>

<div class="login">
  <div
    class="column align-center gap10"
    style:width="400px"
    style:margin-top="100px"
    style:z-index="2"
  >
    <h2 style:margin-bottom="20px">Login to HOLO</h2>
    <Input
      class="w100"
      type="email"
      label="email"
      placeholder="john.doe@example.com"
      bind:value={email}
      bind:isValid={isEmailValid}
      validator={z.string().email()}
    />
    <Input
      class="w100"
      type="password"
      label="password"
      bind:value={password}
      bind:isValid={isPasswordValid}
      validator={z.string().min(8)}
    />
    <Button
      class="w100"
      disabled={!isEmailValid || !isPasswordValid}
      onclick={onLogin}
    >
      Login
    </Button>
    <div
      class="grow justify-space-between"
      style:width="100%"
      style:margin-top="10px"
    >
      <a href="/forgot-password">Forgot Password</a>
      <a href="/register">Signup</a>
    </div>
  </div>
</div>

<DiamondPattern />

<style lang="css">
  .login {
    background-color: white;
    justify-content: center;
    height: 100%;
  }
</style>
