<script lang="ts">
  // What each role means, in plain words (spec PE-5): every role for admins (Figma 107:5980),
  // what a lead can do for directors (Figma 111:8915).
  import Check from "@lucide/svelte/icons/check";

  let { view, committee }: { view: "admin" | "director"; committee: string } = $props();

  const uid = $props.id();
</script>

<aside class="roles-panel" aria-labelledby={`roles-${uid}`}>
  {#if view === "admin"}
    <h2 id={`roles-${uid}`} class="roles-panel__title">What each role can do</h2>
    <dl class="roles-panel__roles">
      <div><dt>Admin</dt><dd>Everything, including roles. The exec board.</dd></div>
      <div>
        <dt>Director</dt>
        <dd>Their committee page and projects, and the ticker. Appoints leads and approves what they publish.</dd>
      </div>
      <div>
        <dt>Lead</dt>
        <dd>The projects they are added to, and the ticker. A director approves what they publish.</dd>
      </div>
      <div><dt>Member</dt><dd>Can sign in. Nothing to edit yet.</dd></div>
    </dl>
    <p class="roles-panel__footnote">
      Sign-in runs on Keycloak. Anyone with a ScottyLabs account can sign in; only the people listed here can edit.
    </p>
  {:else}
    <h2 id={`roles-${uid}`} class="roles-panel__title">What a lead can do</h2>
    <ul class="roles-panel__checks">
      <li><Check size={16} strokeWidth={2} aria-hidden="true" />Edit the projects you pick for them.</li>
      <li>
        <Check size={16} strokeWidth={2} aria-hidden="true" />Write ticker announcements. You approve each one before it goes live.
      </li>
      <li>
        <Check size={16} strokeWidth={2} aria-hidden="true" />They cannot edit the {committee} page or add other leads.
      </li>
    </ul>
    <p class="roles-panel__footnote">Admins appoint directors. Ask the exec board if a director needs to change.</p>
  {/if}
</aside>

<style>
  .roles-panel {
    box-sizing: border-box;
    display: flex;
    flex-direction: column;
    gap: 16px;
    padding: 20px;
    border: 1px solid var(--p-border);
    border-radius: var(--p-radius-xl);
    background: var(--p-surface);
    color: var(--p-text);
    font-family: var(--p-font);
  }
  .roles-panel__title {
    margin: 0;
    font-size: 0.75rem;
    font-weight: 500;
    line-height: 16px;
  }
  .roles-panel__roles {
    display: flex;
    flex-direction: column;
    gap: 16px;
    margin: 0;

    & dt {
      font-size: 0.75rem;
      font-weight: 500;
      line-height: 16px;
    }
    & dd {
      margin: 2px 0 0;
      color: var(--p-text-secondary);
      font-size: 0.875rem;
      line-height: 20px;
    }
  }
  .roles-panel__checks {
    display: flex;
    flex-direction: column;
    gap: 12px;
    margin: 0;
    padding: 0;
    list-style: none;

    & li {
      display: flex;
      gap: 8px;
      color: var(--p-text-secondary);
      font-size: 0.875rem;
      line-height: 20px;
    }
    & :global(svg) {
      flex: none;
      margin-top: 2px;
      color: var(--p-text);
    }
  }
  .roles-panel__footnote {
    margin: 0;
    padding-top: 16px;
    border-top: 1px solid var(--p-border);
    color: var(--p-text-tertiary);
    font-size: 0.75rem;
    line-height: 16px;
  }
</style>
