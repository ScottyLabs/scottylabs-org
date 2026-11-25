<script lang="ts">
  // A call to action's buttons: the main one, and an optional second one. Links may go to other
  // sites (https), to an email address (mailto:) or to a page on this site (/sponsors/).
  import Plus from "@lucide/svelte/icons/plus";
  import type { LinkButton } from "../../api/types.ts";
  import { Button } from "../../portal/ui/index.ts";
  import { LIMITS } from "../limits.ts";
  import { JOIN_URL, SLACK_URL } from "../links.ts";
  import TextField from "./TextField.svelte";

  let {
    primary,
    secondary,
    problems,
    onchange,
  }: {
    primary: LinkButton | null;
    secondary: LinkButton | null;
    problems: Readonly<Record<string, string>>;
    onchange: (buttons: { primary: LinkButton | null; secondary: LinkButton | null }, key: string | null) => void;
  } = $props();

  const LINK_HELP = "A full link starting with https://, an email address as mailto:, or a page on this site such as /sponsors/.";
</script>

{#snippet button(which: "primary" | "secondary", current: LinkButton | null, title: string)}
  <fieldset class="cta-fields">
    <legend class="cta-fields__legend">{title}</legend>
    {#if current === null}
      <Button
        variant="outline"
        size="sm"
        icon={Plus}
        onclick={() =>
          onchange(
            {
              primary,
              secondary,
              [which]: which === "primary" ? { label: "Apply", url: JOIN_URL } : { label: "Join our Slack", url: SLACK_URL },
            },
            null,
          )}
      >
        Add a button
      </Button>
    {:else}
      <TextField
        label="Label"
        value={current.label}
        max={LIMITS.buttonLabel}
        error={problems[`${which}.label`]}
        onchange={(label) => onchange({ primary, secondary, [which]: { ...current, label } }, `${which}:label`)}
      />
      <TextField
        label="Link"
        type="url"
        value={current.url}
        help={LINK_HELP}
        error={problems[`${which}.url`]}
        onchange={(url) => onchange({ primary, secondary, [which]: { ...current, url } }, `${which}:url`)}
      />
      <Button variant="subtle" size="sm" onclick={() => onchange({ primary, secondary, [which]: null }, null)}>
        Remove this button
      </Button>
    {/if}
  </fieldset>
{/snippet}

{@render button("primary", primary, "Main button")}
{@render button("secondary", secondary, "Second button")}

<style>
  .cta-fields {
    display: flex;
    flex-direction: column;
    align-items: stretch;
    gap: 10px;
    margin: 0;
    padding: 0;
    border: 0;

    & :global(.p-button--subtle) {
      align-self: flex-start;
    }
  }
  .cta-fields__legend {
    margin-bottom: 8px;
    padding: 0;
    color: var(--p-text);
    font-size: 0.875rem;
    font-weight: 500;
    line-height: 20px;
  }
</style>
