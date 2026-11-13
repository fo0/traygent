<script lang="ts">
  import { onDestroy } from 'svelte';
  import Dropdown from './Dropdown.svelte';
  import Icon from './Icon.svelte';
  import { dateTime, errorMessage } from './format';
  import type { ProfileActions } from './profileActions.svelte';
  import {
    DEFAULT_PROFILE,
    STANDARD_LABEL,
    agentProfiles,
    profileFolder,
    profileId,
  } from './profiles';
  import type { SettingsDraft } from './settingsDraft.svelte';
  import type { AgentCatalogEntry, Snapshot } from './types';

  // „Profile“ of one agent (#33): the default and the list are staged with
  // „Speichern“; „Anmelden“ and „Jetzt synchronisieren“ act at once.
  let {
    entry,
    snapshot,
    draft,
    demo,
    actions,
  }: {
    entry: AgentCatalogEntry;
    snapshot: Snapshot;
    draft: SettingsDraft;
    demo: boolean;
    actions: ProfileActions;
  } = $props();

  const OUT = 'Nicht angemeldet';
  const HELP = 'Eigene Anmeldung je Profil, alles andere wird abgeglichen';
  const agent = $derived(entry.id);
  const statuses = $derived(agentProfiles(snapshot, agent));
  const rows = $derived([
    { id: DEFAULT_PROFILE, label: STANDARD_LABEL },
    ...(draft.value.profiles[agent] ?? []),
  ]);
  const problem = $derived(draft.problem(`agents.${agent}.profiles`));

  let name = $state('');
  /** Why the typed name cannot be added; cleared on every keystroke. */
  let typed = $state('');
  let busy = $state(false);
  let note = $state('');
  let failure = $state('');
  let timer: ReturnType<typeof setTimeout> | undefined;
  onDestroy(() => clearTimeout(timer));

  /** A success note that goes away after 3 s (UX rule 7). */
  function done(text: string) {
    note = text;
    clearTimeout(timer);
    timer = setTimeout(() => (note = ''), 3000);
  }
  async function act(run: () => Promise<unknown>, text: string) {
    if (busy) return;
    busy = true;
    note = failure = '';
    try {
      if ((await run()) !== false) done(text);
    } catch (cause) {
      failure = errorMessage(cause);
    } finally {
      busy = false;
    }
  }
  function add() {
    if (!name.trim()) return;
    typed = draft.addProfile(agent, name);
    if (!typed) name = '';
  }
  function statusTitle(id: string) {
    const status = statuses.find((item) => item.id === id);
    if (!status) return 'Erst speichern';
    return [
      status.account ?? OUT,
      status.organization,
      status.dir,
      status.syncedAt && `Abgeglichen ${dateTime(status.syncedAt)}`,
      status.syncError,
    ]
      .filter(Boolean)
      .join(' · ');
  }
</script>

<section class="settings-group profiles" aria-labelledby="profiles-heading">
  <!-- The immediate sync sits level with the heading, not in a row of its own. -->
  <div class="profiles-head">
    <h3 id="profiles-heading">
      Profile<span class="info-tip" role="img" aria-label={HELP} title={HELP}
        ><Icon name="info" size={13} /></span
      >
    </h3>
    <!-- Off via aria-disabled: it keeps focus while the sync runs. --><button
      type="button"
      class="text-button profile-sync"
      aria-disabled={busy || actions.syncing ? 'true' : undefined}
      onclick={() =>
        void act(
          () => actions.sync(),
          demo ? 'Profile synchronisiert (Demo).' : 'Profile synchronisiert.',
        )}><Icon name="refresh" size={13} />Jetzt synchronisieren</button
    >
  </div>
  <div class="field-grid">
    <label for="profile-default">Standard-Profil</label>
    <Dropdown
      id="profile-default"
      label="Standard-Profil"
      variant="field"
      items={rows}
      value={draft.value.agentProfile[agent] ?? DEFAULT_PROFILE}
      onChange={(id) => (draft.value.agentProfile[agent] = id)}
    />
  </div>
  <ul class="profile-list" aria-label="Profile">
    {#each rows as row (row.id)}
      {@const status = statuses.find((item) => item.id === row.id)}
      {@const saved = draft.savedProfile(agent, row.id) && !!status}
      <li>
        <span class="profile-name">{row.label}</span>
        <span
          class="profile-account"
          data-out={status?.signedIn ? undefined : 'true'}
          title={statusTitle(row.id)}
          >{status?.signedIn ? (status.account ?? 'Angemeldet') : OUT}</span
        >
        {#if status?.syncError}<span
            class="profile-warn"
            role="img"
            aria-label={`Abgleich fehlgeschlagen: ${status.syncError}`}
            title={status.syncError}><Icon name="warning" size={13} /></span
          >{/if}
        <button
          type="button"
          class="text-button profile-login"
          aria-label={`${row.label} anmelden`}
          aria-disabled={!saved || busy ? 'true' : undefined}
          title={saved ? 'Öffnet ein Terminal zum Anmelden' : 'Erst speichern'}
          onclick={() =>
            saved &&
            void act(
              () => actions.login(agent, row.id),
              demo ? 'Demo: kein Terminal geöffnet.' : 'Anmeldung geöffnet.',
            )}>Anmelden</button
        >
        {#if row.id === DEFAULT_PROFILE}<span
            class="profile-slot"
            aria-hidden="true"
          ></span>{:else}<button
            type="button"
            class="icon-button small profile-remove"
            aria-label={`${row.label} entfernen`}
            title="Entfernen"
            onclick={() => draft.removeProfile(agent, row.id)}
            ><Icon name="trash" size={14} /></button
          >{/if}
      </li>
    {/each}
  </ul>
  <div class="port-add profile-add">
    <input
      bind:value={name}
      class="text-field"
      type="text"
      aria-label="Neues Profil"
      placeholder="Neues Profil"
      title={name.trim()
        ? profileFolder(agent, profileId(name))
        : 'Name, z. B. Team'}
      spellcheck="false"
      autocomplete="off"
      aria-invalid={typed ? 'true' : undefined}
      aria-describedby={typed || problem ? 'profile-error' : undefined}
      oninput={() => (typed = '')}
      onkeydown={(event) => {
        if (event.key !== 'Enter') return;
        event.preventDefault();
        event.stopPropagation();
        add();
      }}
    /><button
      type="button"
      class="icon-button small"
      aria-label="Profil hinzufügen"
      title="Hinzufügen (Enter)"
      onclick={add}><Icon name="plus" size={14} /></button
    >
  </div>
  {#if typed || problem}<p id="profile-error" class="field-error">
      {typed || problem}
    </p>{/if}
  {#if note}<p class="install-note" role="status">
      <Icon name="check" size={13} />{note}
    </p>{/if}
  {#if failure}<div class="alert danger" role="alert">
      <Icon name="warning" size={16} /><span>{failure}</span>
    </div>{/if}
</section>
