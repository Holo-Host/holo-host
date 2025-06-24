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
</script>

<div
  class="column gap10"
  style:margin-top="100px"
  style:width="400px"
  style:margin-bottom="100px"
>
  {#if registered}
    <span>You have successfully registered.</span>
  {:else if loading}
    <span>Loading...</span>
  {:else}
    <Input
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
      type="email"
      label="email"
      placeholder="john.doe@example.com"
      bind:value={email}
      bind:isValid={isEmailValid}
      validator={z.string().email({ message: "Invalid email" })}
    />
    <span>Select Jurisdiction</span>
    <Dropdown
      items={jurisdictions}
      onItemSelected={onJurisdictionSelected}
      filterFocusItems={(item: string) => item !== "Unknown"}
    >
      <div
        class="dropdown"
        style:--border-color={defaultTheme.colors.border}
        style:--background-color={defaultTheme.colors.background.card}
        style:--text-color={defaultTheme.colors.text.black}
      >
        {#if jurisdiction === ""}
          <span style:color={defaultTheme.colors.text.subtext}>
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
      type="password"
      label="Password"
      bind:value={password}
      bind:isValid={isPasswordValid}
      validator={z
        .string()
        .min(8, { message: '"Password" must be at least 8 characters long' })}
    />
    <Input
      type="password"
      label="Confirm Password"
      bind:value={confirmPassword}
      bind:isValid={isConfirmPasswordValid}
      validator={z.string().refine((val) => val === password, {
        message: "Password does not match",
      })}
    />
    <Button disabled={!isFormValid} onclick={onRegisterUser}>Register</Button>
  {/if}
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
