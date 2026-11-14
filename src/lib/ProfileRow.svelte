<script lang="ts">
  import { untrack } from 'svelte';
  import CopyField from './CopyField.svelte';
  import Icon from './Icon.svelte';
  import { dateTime } from './format';
  import { rowAccount } from './profileRow';
  import { STANDARD_LABEL, profileFolder, renameProblem } from './profiles';
  import type { SettingsDraft } from './settingsDraft.svelte';
  import type { AgentId, ProfileStatus } from './types';

  // One profile in Settings → Agenten → „Profile“ (#37, #41): a single line —
  // name, account (or organization, when another row has the same account),
  // warning, „Anmelden“ while signed out, remove — that opens in place to its
  // details with „Neu anmelden“. A running hidden sign-in turns the action
  // into its cancel. An extra profile's name is a staged rename; id and
  // folder stay.
  let {
    agent,
    id,
    draft,
    status,
    statuses,
    open,
    busy,
    running,
    onToggle,
    onLogin,
    onCancel,
  }: {
    agent: AgentId;
    id: string;
    draft: SettingsDraft;
    /** Undefined until the snapshot lists the profile. */
    status: ProfileStatus | undefined;
    /** Every profile of the agent, to tell same-account rows apart. */
    statuses: readonly ProfileStatus[];
    open: boolean;
    /** An immediate action of the block runs. */
    busy: boolean;
    /** Its hidden sign-in runs. */
    running: boolean;
    onToggle: () => void;
    onLogin: () => void;
    onCancel: () => void;
  } = $props();

  const list = $derived(draft.value.profiles[agent] ?? []);
  /** Its place in the staged list; -1 for „Standard“. */
  const index = $derived(list.findIndex((item) => item.id === id));
  const label = $derived(list[index]?.label ?? STANDARD_LABEL);
  const shown = $derived(label || status?.label || id);
  const problem = $derived(index < 0 ? '' : renameProblem(list, index));
  const row = $derived(rowAccount(status, statuses));
  /** The detail shows the account itself, never the organization. */
  const account = $derived(
    status?.signedIn ? (status.account ?? 'Angemeldet') : row.text,
  );
  const blocked = $derived(!status || busy);
  const detail = $derived(`profile-detail-${agent}-${id}`);
  /** What was typed; the staged label is its trimmed copy. */
  let text = $state(untrack(() => label));

  function rename(value: string) {
    text = value;
    const item = list[index];
    if (item) item.label = value.trim();
  }
</script>

<li data-open={open ? 'true' : undefined}>
  <div class="profile-row">
    <button
      type="button"
      class="profile-toggle"
      aria-expanded={open}
      aria-controls={open ? detail : undefined}
      aria-label={`${shown} Details`}
      aria-describedby={problem ? `${detail}-why` : undefined}
      title={problem || (open ? 'Details schließen' : 'Details')}
      onclick={onToggle}
      ><Icon name="down" size={12} /><span
        class="profile-name"
        data-invalid={problem ? 'true' : undefined}>{shown}</span
      >{#if problem}<span id={`${detail}-why`} class="sr-only">{problem}</span
        >{/if}</button
    >
    <span
      class="profile-account"
      data-out={row.out ? 'true' : undefined}
      title={row.title}>{row.text}</span
    >
    {#if status?.syncError}<span
        class="profile-warn"
        role="img"
        aria-label={`Abgleich fehlgeschlagen: ${status.syncError}`}
        title={status.syncError}><Icon name="warning" size={13} /></span
      >{/if}
    {#if running}<button
        type="button"
        class="icon-button small spinning profile-cancel"
        aria-label="Anmeldung läuft"
        title="Abbrechen"
        onclick={onCancel}><Icon name="refresh" size={14} /></button
      >{:else if !status?.signedIn}<button
        type="button"
        class="text-button profile-login"
        aria-label={`${shown} anmelden`}
        aria-disabled={blocked ? 'true' : undefined}
        title="Im Browser anmelden"
        onclick={() => !blocked && onLogin()}>Anmelden</button
      >{/if}
    {#if index < 0}<span class="profile-slot" aria-hidden="true"
      ></span>{:else}<button
        type="button"
        class="icon-button small profile-remove"
        aria-label={`${shown} entfernen`}
        title="Entfernen"
        onclick={() => draft.removeProfile(agent, id)}
        ><Icon name="trash" size={14} /></button
      >{/if}
  </div>
  {#if open}<div id={detail} class="field-grid profile-detail">
      {#if index < 0}<span class="field-label">Name</span><span
          class="profile-value">{label}</span
        >{:else}<label for={`${detail}-name`}>Name</label>
        <div class="field-cell">
          <input
            id={`${detail}-name`}
            class="text-field"
            type="text"
            spellcheck="false"
            autocomplete="off"
            title="Ordner und Kennung bleiben"
            aria-invalid={problem ? 'true' : undefined}
            aria-describedby={problem ? `${detail}-error` : undefined}
            value={text}
            oninput={(event) => rename(event.currentTarget.value)}
          />
          {#if problem}<p id={`${detail}-error`} class="field-error">
              {problem}
            </p>{/if}
        </div>{/if}
      <span class="field-label">Konto</span><span
        class="profile-value profile-account-value"
        data-out={status?.signedIn ? undefined : 'true'}
        ><span>{account}</span>{#if status?.signedIn && !running}<button
            type="button"
            class="text-button"
            aria-label={`${shown} neu anmelden`}
            aria-disabled={blocked ? 'true' : undefined}
            title="Im Browser anmelden"
            onclick={() => !blocked && onLogin()}>Neu anmelden</button
          >{/if}</span
      >
      {#if status?.organization}<span class="field-label">Organisation</span
        ><span class="profile-value">{status.organization}</span>{/if}
      <span class="field-label">Ordner</span><CopyField
        value={status?.dir || profileFolder(agent, id)}
        label={`Ordner von ${shown}`}
      />
      {#if status?.syncedAt}<span class="field-label">Abgeglichen</span><span
          class="profile-value">{dateTime(status.syncedAt)}</span
        >{/if}
      {#if status?.syncError}<span class="field-label">Abgleich</span><span
          class="profile-value"
          data-warn="true">{status.syncError}</span
        >{/if}
    </div>{/if}
</li>
