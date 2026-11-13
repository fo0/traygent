<script lang="ts">
  import AgentBadge from './AgentBadge.svelte';
  import Icon from './Icon.svelte';
  import RecentMeta from './RecentMeta.svelte';
  import {
    catalogEntry,
    choiceLabel,
    effortChoices,
    envInfo,
    folderName,
    parentPath,
    permissionChoice,
    sentOptions,
    shimStart,
    startState,
  } from './agents';
  import { landing } from './focus';
  import { dateTime } from './format';
  import { closedPopup, menu, openingFocus, type MenuFocus } from './menu';
  import { EXPOSED, exposedIp, portLabel } from './ports';
  import { repeatClick, repeatKey } from './rowStart';
  import type { AgentId, RecentSession, Snapshot } from './types';
  import { repoLabel } from './target';
  import { optionsProblem } from './validation';

  let {
    recent,
    snapshot,
    now,
    profile,
    profileColumn,
    launching,
    onRestart,
    onAdopt,
    onEdit,
    onOpenFolder,
    onCopyFolder,
    onRemove,
    onInstall,
  }: {
    recent: RecentSession;
    snapshot: Snapshot;
    /** The shared minute clock, for the relative time. */
    now: number;
    /** The label of the row's profile, `''` for „Standard“. */
    profile: string;
    /** Some row has a profile: every row keeps its column. */
    profileColumn: boolean;
    /** A start runs; the row's start waits for it. */
    launching: boolean;
    /** Starts exactly these options again, as a new session. */
    onRestart: (recent: RecentSession) => void;
    onAdopt: (recent: RecentSession) => void;
    /** „Bearbeiten …“: the options and ports the start sends. */
    onEdit: (recent: RecentSession) => void;
    onOpenFolder: (path: string) => void;
    /** „Pfad kopieren“: the folder path on the clipboard. */
    onCopyFolder: (path: string) => void;
    onRemove: (recent: RecentSession) => Promise<void>;
    onInstall: (agent: AgentId) => void;
  } = $props();
  const uid = $props.id();

  const entry = $derived(catalogEntry(snapshot, recent.agent));
  const label = $derived(entry?.label ?? recent.agent);
  const name = $derived(folderName(recent.folder));
  const parent = $derived(parentPath(recent.folder));
  const env = $derived(envInfo(recent.env));
  const repo = $derived(
    recent.repo ? `GitHub-Repository ${repoLabel(recent.repo)}` : '',
  );
  /** What a restart sends: an effort the model cannot use never shows. */
  const options = $derived(sentOptions(entry, recent.options));
  const permission = $derived(permissionChoice(entry, options.permission));
  /** Never hidden: a one-click restart names the arguments it passes. */
  const args = $derived(options.extraArgs.trim());
  const argsNote = $derived(args ? ` · Extra-Argumente: ${args}` : '');
  /**
   * A restart publishes exactly these; the chip shows the first host port, or
   * the first one the network reaches, with its IP.
   */
  const ports = $derived(recent.ports);
  const exposed = $derived(ports.find((entry) => exposedIp(entry) !== null));
  const portsTitle = $derived(
    `Ports: ${ports.join(', ')}${exposed ? ` · ${EXPOSED}` : ''}`,
  );
  const portsText = $derived(
    ports.length
      ? `${portLabel(exposed ?? ports[0] ?? '')}${ports.length > 1 ? ` +${ports.length - 1}` : ''}`
      : '',
  );
  const problem = $derived(
    optionsProblem(recent, shimStart(snapshot, recent.agent, recent.env)),
  );
  const start = $derived(startState(snapshot, recent));
  const target = $derived(
    start.kind === 'ready' && problem
      ? { kind: 'blocked' as const, reason: problem }
      : start,
  );
  /**
   * The options as symbols (owner request, #17): a chosen model as its
   * letter badge, the thinking level as signal bars wherever the agent sends
   * one with this model, „Standard“ as dim bars.
   */
  const modelLabel = $derived(choiceLabel(entry?.models ?? [], options.model));
  const thinks = $derived(effortChoices(entry, options.model).length > 0);
  const effortLabel = $derived(
    choiceLabel(entry?.efforts ?? [], options.effort),
  );
  /** Only options that differ from „Standard“ go into the start's tooltip. */
  const optionTitle = $derived(
    [
      options.model !== 'default' && `Modell: ${modelLabel}`,
      options.effort !== 'default' && `Denken: ${effortLabel}`,
    ]
      .filter((part) => !!part)
      .join(' · '),
  );
  /**
   * The start's hit area covers the row (owner request, #17), so the row's
   * own parts have no tooltips: the start's names everything the row shows.
   */
  const details = $derived(
    [
      recent.folder,
      repo,
      env.label,
      profile && `Profil: ${profile}`,
      optionTitle,
      `Freigabe: ${permission.label}`,
      ports.length && portsTitle,
      args && `Extra-Argumente: ${args}`,
      dateTime(recent.lastStartedAt),
    ]
      .filter((part) => !!part)
      .join(' · '),
  );
  const restartTitle = $derived(
    target.kind === 'install'
      ? `${label} ist lokal nicht installiert`
      : target.kind === 'blocked'
        ? target.reason
        : `${label} erneut starten · ${details}`,
  );

  let open = $state(false);
  let up = $state(false);
  let initial = $state<MenuFocus>('first');
  let trigger = $state<HTMLButtonElement>();

  /** Height of the row menu's five items, for the direction it opens. */
  const MENU_HEIGHT = 197;
  /** Opens upwards when the menu would run past the bottom of the list. */
  function openMenu(focus: MenuFocus) {
    const bottom = trigger?.closest('.content')?.getBoundingClientRect().bottom;
    const below = trigger?.getBoundingClientRect().bottom ?? 0;
    up = bottom !== undefined && below + MENU_HEIGHT > bottom;
    initial = focus;
    open = true;
  }
  /**
   * One start per press: the rest of a multi-click never starts again, and
   * a press that only closed a popup (a row menu, the ports popover) never
   * starts at all.
   */
  function restart(event: MouseEvent) {
    if (closedPopup(event) || repeatClick(event) || launching) return;
    if (target.kind === 'install') onInstall(recent.agent);
    else if (target.kind === 'ready') onRestart(recent);
  }
  function choose(action: () => void) {
    open = false;
    trigger?.focus();
    action();
  }
  /** Focus moves to the neighbouring row first; the removed row is gone. */
  async function remove() {
    // Focus leaves the menu first, so closing it never drops focus.
    trigger?.focus();
    open = false;
    const row = trigger?.closest('li');
    const next = (
      row?.nextElementSibling ?? row?.previousElementSibling
    )?.querySelector<HTMLElement>('.recent-more');
    await onRemove(recent);
    (next?.isConnected ? next : landing())?.focus();
  }
</script>

<li class="recent-row">
  <AgentBadge agent={recent.agent} {label} />
  <span class="recent-text"
    ><span class="recent-name">{name}</span>{#if parent}<span
        class="recent-parent"><bdi>{parent}</bdi></span
      >{/if}</span
  >
  <RecentMeta
    {repo}
    {env}
    {profile}
    {profileColumn}
    model={options.model}
    {modelLabel}
    {thinks}
    effort={options.effort}
    {effortLabel}
    {portsText}
    {portsTitle}
    exposed={!!exposed}
    {permission}
    lastStartedAt={recent.lastStartedAt}
    {now}
  />
  <!-- „⋯“, then the start at the far right. The start's hit area covers the
       row (owner request, #17); „⋯“ and the menu sit above it. A held key or
       the second click of a double-click never starts again. -->
  <span class="recent-actions">
    <button
      bind:this={trigger}
      type="button"
      class="icon-button small recent-more"
      aria-label={`Aktionen für ${name}`}
      title="Aktionen"
      aria-haspopup="menu"
      aria-expanded={open}
      onclick={() => (open ? (open = false) : openMenu('first'))}
      onkeydown={(event) => {
        const focus = openingFocus(event.key);
        if (!focus) return;
        event.preventDefault();
        openMenu(focus);
      }}><Icon name="more" size={16} /></button
    >
    <button
      type="button"
      class="recent-start"
      aria-label={`${label} in ${name} starten${argsNote}`}
      aria-describedby={`${uid}-folder`}
      title={restartTitle}
      aria-disabled={target.kind === 'blocked' || launching
        ? 'true'
        : undefined}
      onclick={restart}
      onkeydown={(event) => repeatKey(event) && event.preventDefault()}
      ><Icon name="play" size={12} /></button
    >
    <!-- The start's description: the folder alone. The long tooltip repeats
         what the row's labelled icons and the name already say. -->
    <span id={`${uid}-folder`} hidden>{recent.folder}</span>
    {#if open}
      <div
        class="menu row-menu"
        class:up
        role="menu"
        aria-label={`Aktionen für ${name}`}
        use:menu={{ trigger, initial, onClose: () => (open = false) }}
      >
        <button
          type="button"
          role="menuitem"
          class="menu-item"
          onclick={() => choose(() => onAdopt(recent))}
          ><Icon name="arrow" size={14} />In Composer übernehmen</button
        >
        <button
          type="button"
          role="menuitem"
          class="menu-item"
          onclick={() => choose(() => onEdit(recent))}
          ><Icon name="edit" size={14} />Bearbeiten …</button
        >
        <button
          type="button"
          role="menuitem"
          class="menu-item"
          onclick={() => choose(() => onOpenFolder(recent.folder))}
          ><Icon name="external" size={14} />Ordner öffnen</button
        >
        <button
          type="button"
          role="menuitem"
          class="menu-item"
          onclick={() => choose(() => onCopyFolder(recent.folder))}
          ><Icon name="copy" size={14} />Pfad kopieren</button
        >
        <button
          type="button"
          role="menuitem"
          class="menu-item danger"
          onclick={() => void remove()}
          ><Icon name="trash" size={14} />Entfernen</button
        >
      </div>
    {/if}
  </span>
</li>
