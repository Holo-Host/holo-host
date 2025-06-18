<script lang="ts">
  import Button from "@/components/button.svelte";
  import Input from "@/components/input.svelte";
  import { login } from "@/api";
  import { z } from "zod";

  let email = $state("");
  let password = $state("");

  async function onLogin() {
    login(email, password);
  }
</script>

<div class="column gap10" style:margin-top="100px">
  <Input
    type="email"
    label="email"
    placeholder="john.doe@example.com"
    validator={z.string().email()}
    bind:value={email}
  />
  <Input type="password" label="password" bind:value={password} />
  <Button onclick={onLogin}>Login</Button>
</div>
