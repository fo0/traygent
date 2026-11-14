<script lang="ts">
  import { onDestroy } from 'svelte';
  import Dropdown from './Dropdown.svelte';
  import Icon from './Icon.svelte';
  import ProfileRow from './ProfileRow.svelte';
  import ProfileShareFields from './ProfileShareFields.svelte';
  import { errorMessage } from './format';
  import type { ProfileActions } from './profileActions.svelte';
  import {
    DEFAULT_PROFILE,
    STANDARD_LABEL,
    agentProfiles,
    profileFolder,
    profileId,
    renameProblem,
  } from './profiles';
  import type { SettingsDraft } from './settingsDraft.svelte';
  import type { AgentCatalogEntry, Snapshot } from './types';

  // „Profile“ of one agent (#33, #37): adding a profile saves it at once, so
  // it signs in right away; the default, removals and renames are staged
  // with „Speichern“; „Anmelden“ and „Jetzt synchronisieren“ act at once.
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

  const HELP =
    'Eigene Anmeldung je Profil, alles andere wird abgeglichen. Weitere Ordner und Dateien werden wie die eingebauten abgeglichen.';
  const agent = $derived(entry.id);
  const statuses = $derived(agentProfiles(snapshot, agent));
  const extra = $derived(draft.value.profiles[agent] ?? []);
  const rows = $derived([
    { id: DEFAULT_PROFILE, label: STANDARD_LABEL },
    ...extra,
  ]);
  /** Invalid renames, each named as its row shows it. */
  const renames = $derived(
    extra.flatMap((item, index) => {
      const why = renameProblem(extra, index);
      const saved = statuses.find((status) => status.id === item.id);
      return why
        ? [
            {
              id: item.id,
              text: `${item.label || saved?.label || item.id}: ${why}`,
            },
          ]
        : [];
    }),
  );
  // An open row shows its rename problem itself; a closed one's shows here,
  // so a blocked „Speichern“ always has a visible reason.
  const problem = $derived(
    renames.find((rename) => rename.id !== opened)?.text ??
      (renames.length ? '' : draft.problem(`agents.${agent}.profiles`)),
  );

  let name = $state('');
  /** Why the typed name was not added; cleared on every keystroke. */
  let typed = $state('');
  /** The one profile whose details are open. */
  let opened = $state('');
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
  /** Saves the typed profile at once; a refusal keeps the name typed. */
  async function add() {
    if (!name.trim() || draft.busy) return;
    const result = await draft.addProfile(agent, name, actions.save);
    if (result === null) return;
    typed = result;
    if (!result) name = '';
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
      title="Automatisch alle 15 Minuten"
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
      <ProfileRow
        {agent}
        id={row.id}
        {draft}
        status={statuses.find((item) => item.id === row.id)}
        open={opened === row.id}
        {busy}
        onToggle={() => (opened = opened === row.id ? '' : row.id)}
        onLogin={() =>
          void act(
            () => actions.login(agent, row.id),
            demo ? 'Demo: kein Terminal geöffnet.' : 'Anmeldung geöffnet.',
          )}
      />
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
      readonly={draft.adding}
      aria-busy={draft.adding ? 'true' : undefined}
      aria-invalid={typed ? 'true' : undefined}
      aria-describedby={typed || problem ? 'profile-error' : undefined}
      oninput={() => (typed = '')}
      onkeydown={(event) => {
        if (event.key !== 'Enter') return;
        event.preventDefault();
        event.stopPropagation();
        void add();
      }}
    /><button
      type="button"
      class="icon-button small"
      class:spinning={draft.adding}
      aria-label={draft.adding
        ? 'Profil wird hinzugefügt'
        : 'Profil hinzufügen'}
      aria-disabled={draft.busy ? 'true' : undefined}
      title={draft.adding ? 'Wird hinzugefügt …' : 'Hinzufügen (Enter)'}
      onclick={() => void add()}
      ><Icon name={draft.adding ? 'refresh' : 'plus'} size={14} /></button
    >
  </div>
  {#if typed || problem}<p id="profile-error" class="field-error">
      {typed || problem}
    </p>{/if}
  <!-- Nothing to share with before a second profile exists. -->
  {#if draft.value.profiles[agent]?.length}<ProfileShareFields
      {agent}
      {draft}
    />{/if}
  {#if note}<p class="install-note" role="status">
      <Icon name="check" size={13} />{note}
    </p>{/if}
  {#if failure}<div class="alert danger" role="alert">
      <Icon name="warning" size={16} /><span>{failure}</span>
    </div>{/if}
</section>
