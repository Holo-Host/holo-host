<script lang="ts">
  import * as z from "zod";
  import Button from "@/components/button.svelte";
  import Input from "@/components/input.svelte";
  import Dropdown from "@/components/dropdown.svelte";
  import { jurisdictions } from "@/data/jurisdiction";
  import { defaultTheme } from "@/theme";
  import { requestNoAuth } from "@/api";

  type Prop = {
    code: string;
    email?: string;
  };
  const props: Prop = $props();
  let registered = $state(false);
  let loading = $state(false);
  let email = $state(props.email ?? "");
  let givenNames = $state("");
  let familyName = $state("");
  let jurisdiction = $state("");
  let password = $state("");
  let confirmPassword = $state("");
  let isEmailValid = $state(false);
  let isGivenNamesValid = $state(false);
  let isFamilyNameValid = $state(false);
  let isPasswordValid = $state(false);
  let isConfirmPasswordValid = $state(false);
  const isFormValid = $derived(
    isEmailValid &&
      isGivenNamesValid &&
      isFamilyNameValid &&
      isPasswordValid &&
      isConfirmPasswordValid &&
      jurisdictions.includes(jurisdiction)
  );

  async function onRegisterUser() {
    const req = await requestNoAuth(`/public/v1/auth/register`, {
      method: "post",
      body: JSON.stringify({
        email,
        given_names: givenNames,
        family_name: familyName,
        jurisdiction,
        password,
        email_verification_code: props.code,
      }),
    });
    if (!req.ok) {
      console.error("failed to register user");
      return;
    }
    registered = true;
  }

  function onJurisdictionSelected(item: string) {
    jurisdiction = item;
  }
  function onSubmit(e: Event) {
    e.preventDefault();
    if (!isFormValid) return;
    onRegisterUser();
  }
</script>

<div
  class="column gap10 align-center"
  style:margin-top="100px"
  style:width="400px"
  style:max-width="400px"
>
  <h2 style:margin-bottom="20px">Sign up to HOLO</h2>
  <form onsubmit={onSubmit} class="w100">
    {#if registered}
      <span>
        You have successfully registered.
        <a href="/login">Login</a>
      </span>
    {:else if loading}
      <span>Loading...</span>
    {:else}
      <Input
        class="w100"
        type="text"
        label="Given Names"
        placeholder="John Smith"
        bind:value={givenNames}
        bind:isValid={isGivenNamesValid}
        validator={z.string().min(3, {
          message: '"Given Names" must be at least 3 characters long',
        })}
      />
      <Input
        class="w100"
        type="text"
        label="Family Name"
        placeholder="Doe"
        bind:value={familyName}
        bind:isValid={isFamilyNameValid}
        validator={z.string().min(3, {
          message: '"Given Names" must be at least 3 characters long',
        })}
      />
      <Input
        disabled
        class="w100"
        type="email"
        label="email"
        placeholder="john.doe@example.com"
        bind:value={email}
        bind:isValid={isEmailValid}
        validator={z.string().email({ message: "Invalid email" })}
      />
      <div class="w100" style:text-align="left">
        <span>Select Jurisdiction</span>
      </div>
      <Dropdown
        class="w100"
        items={jurisdictions}
        onItemSelected={onJurisdictionSelected}
        filterFocusItems={(item: string) => item !== "Unknown"}
      >
        <div
          class="dropdown w100"
          style:margin-bottom="20px"
          style:--border-color={defaultTheme.colors.border}
          style:--background-color={defaultTheme.colors.background.card}
          style:--text-color={defaultTheme.colors.text.black}
        >
          {#if jurisdiction === ""}
            <span
              style:color={defaultTheme.colors.text.subtext}
              style:width="100%"
            >
              Select Jurisdiction
            </span>
          {:else}
            <span>{jurisdiction}</span>
          {/if}
        </div>
        {#snippet itemTemplate(item: string)}
          <span>{item}</span>
        {/snippet}
      </Dropdown>
      <Input
        class="w100"
        type="password"
        label="Password"
        bind:value={password}
        bind:isValid={isPasswordValid}
        validator={z
          .string()
          .min(8, { message: '"Password" must be at least 8 characters long' })}
      />
      <Input
        class="w100"
        type="password"
        label="Confirm Password"
        bind:value={confirmPassword}
        bind:isValid={isConfirmPasswordValid}
        validator={z.string().refine((val) => val === password, {
          message: "Password does not match",
        })}
      />
      <Button
        type="submit"
        class="w100"
        disabled={!isFormValid}
        onclick={onRegisterUser}
      >
        Register
      </Button>
      <div class="grow justify-space-between w100" style:margin-top="10px">
        <a href="/forgot-password">Forgot Password</a>
        <a href="/login">Login</a>
      </div>
    {/if}
  </form>
</div>

<style lang="css">
  .dropdown {
    width: 100%;
    height: 20px;
    font-size: 18px;
    padding-left: 20px;
    padding-top: 10px;
    padding-bottom: 10px;
    border: 1px solid var(--border-color);
    background-color: var(--background-color);
    color: var(--text-color);
  }
</style>
