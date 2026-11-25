<script lang="ts">
  // Admin-only settings (spec PR-8): the project's address, with a redirect from the old one,
  // and whether it is the featured project on the Projects page.
  import { isApiError, patch } from "../../api/client.ts";
  import type { ProjectEditorResponse } from "../../api/types.ts";
  import { getToasts } from "../../portal/toasts.svelte.ts";
  import { Button, Field, Switch, TextInput } from "../../portal/ui/index.ts";

  let {
    projectId,
    name,
    slug,
    featured,
    onchange,
  }: {
    projectId: string;
    name: string;
    slug: string;
    featured: boolean;
    onchange: (response: ProjectEditorResponse) => void;
  } = $props();

  const toasts = getToasts();
  let address = $state("");
  let addressError = $state<string | null>(null);
  let saving = $state(false);
  let featuring = $state(false);

  $effect(() => {
    address = slug;
  });

  const changed = $derived(address.trim() !== "" && address.trim() !== slug);

  async function saveAddress(event: SubmitEvent): Promise<void> {
    event.preventDefault();
    if (!changed) return;
    saving = true;
    addressError = null;
    try {
      const response = await patch<ProjectEditorResponse>(`/projects/${projectId}/settings`, {
        slug: address.trim(),
      });
      onchange(response);
      toasts.success(
        `${name} now lives at /projects/${response.project.slug}/. The old address leads there.`,
      );
    } catch (error) {
      addressError = isApiError(error)
        ? (error.field("slug") ?? error.message)
        : "We couldn\u2019t change the address. Try again.";
    } finally {
      saving = false;
    }
  }

  async function setFeatured(next: boolean): Promise<void> {
    featuring = true;
    try {
      const response = await patch<ProjectEditorResponse>(`/projects/${projectId}/settings`, {
        featured: next,
      });
      onchange(response);
      toasts.success(next ? `${name} is featured on the Projects page.` : `${name} is no longer featured.`);
    } catch (error) {
      toasts.error(isApiError(error) ? error.message : "We couldn\u2019t change that. Try again.");
    } finally {
      featuring = false;
    }
  }
</script>

<section class="settings" aria-labelledby="settings-heading">
  <div class="settings__intro">
    <h2 class="settings__heading" id="settings-heading">Settings</h2>
    <p class="settings__text">Only admins see these.</p>
  </div>
  <form class="settings__address" onsubmit={saveAddress} novalidate>
    <Field
      label="Address"
      help="Lowercase letters, numbers and hyphens. The old address keeps working and leads here."
      error={addressError}
    >
      <div class="address">
        <span class="address__prefix" aria-hidden="true">/projects/</span>
        <TextInput bind:value={address} autocomplete="off" spellcheck={false} />
      </div>
    </Field>
    <div>
      <Button type="submit" variant="outline" loading={saving} disabled={!changed}>Change address</Button>
    </div>
  </form>
  <Switch
    checked={featured}
    label="Feature on the Projects page"
    description="Shows it at the top of the Projects page, in place of the project featured now."
    disabled={featuring}
    onchange={(next) => void setFeatured(next)}
  />
</section>

<style>
  .settings {
    display: flex;
    flex-direction: column;
    gap: 18px;
    color: var(--p-text);
  }
  .settings__intro {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .settings__heading {
    margin: 0;
    font-size: 1.25rem;
    font-weight: 500;
    line-height: 28px;
  }
  .settings__text {
    margin: 0;
    color: var(--p-text-secondary);
    font-size: 0.875rem;
    line-height: 20px;
  }
  .settings__address {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .address {
    display: flex;
    align-items: center;
    gap: 6px;

    & > :global(:last-child) {
      flex: 1;
    }
  }
  .address__prefix {
    color: var(--p-text-secondary);
    font-family: var(--p-mono);
    font-size: 0.8125rem;
  }
</style>
