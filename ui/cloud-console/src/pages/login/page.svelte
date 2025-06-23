<script lang="ts">
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

<div class="column gap10" style:margin-top="100px">
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
</div>
