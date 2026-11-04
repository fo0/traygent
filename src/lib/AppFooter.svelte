<script lang="ts">
  import Icon from './Icon.svelte';
  import { dockerLine } from './agents';
  import { clockTime } from './format';
  import type { EnvStatus } from './types';

  let {
    demo,
    checkedAt,
    docker,
    refreshing,
    detecting,
    canRefresh,
    onRefresh,
  }: {
    demo: boolean;
    /** Last finished detection; undefined until one ran. */
    checkedAt: string | undefined;
    /** The Docker probe; undefined before the first snapshot. */
    docker: EnvStatus | undefined;
    refreshing: boolean;
    /** A detection runs: the refresh icon spins. */
    detecting: boolean;
    /** False while the first snapshot loads; true again after a load error. */
    canRefresh: boolean;
    onRefresh: () => void;
  } = $props();
  const stamp = $derived(
    checkedAt !== undefined ? `Stand ${clockTime(checkedAt)}` : '',
  );
  const dockerState = $derived(docker ? dockerLine(docker) : null);
</script>

<footer class="app-footer">
  <div
    class="connection"
    role="status"
    aria-label="Status"
    title={demo
      ? 'Demo-Modus: Vorschau im Browser. Alle Daten sind simuliert.'
      : 'Traygent auf diesem Rechner'}
  >
    <span class="connection-dot"></span>
    <span class="connection-copy"
      ><span class="demo-tag">{demo ? 'Demo' : 'Lokal'}</span
      >{#if dockerState}<span
          class="docker-state"
          data-state={dockerState.state}
          title={docker?.detail ?? docker?.version ?? undefined}
          ><Icon name="docker" size={12} />{dockerState.text}</span
        >{/if}</span
    >
  </div>
  <button
    class:spinning={detecting}
    onclick={() => onRefresh()}
    disabled={refreshing || !canRefresh}
    title={detecting
      ? 'Erkennung läuft …'
      : demo
        ? 'Demo neu erkennen'
        : 'Agenten und Docker neu erkennen'}
    aria-label={!refreshing && stamp ? `Aktualisieren · ${stamp}` : undefined}
    >{refreshing
      ? 'Aktualisiert …'
      : stamp
        ? stamp
        : canRefresh
          ? 'Erkennen'
          : 'Lädt …'}<Icon name="refresh" size={12} /></button
  >
</footer>
