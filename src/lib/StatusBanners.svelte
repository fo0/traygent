<script module lang="ts">
  export type NoticeTone = 'subtle' | 'warning' | 'danger';
</script>

<script lang="ts">
  import Icon from './Icon.svelte';
  import NoticeBanner from './NoticeBanner.svelte';
  import { focusAfterNotice, neighbourNotice } from './focus';
  import type { Notice } from './notices.svelte';
  import type { UpdateInfo } from './types';

  let {
    update,
    demo,
    error,
    notice,
    notices,
    paused = false,
    onShowUpdate,
    onDismissNotice,
  }: {
    update: UpdateInfo | null;
    demo: boolean;
    error: string;
    /**
     * The snapshot's persistent warning, e.g. a read-only state file. It
     * stays as long as the native side sends it; nothing closes it.
     */
    notice?: string | undefined;
    /** Panel notices, oldest first. */
    notices: Notice[];
    /** A dialog hides the notices: none closes on its own meanwhile. */
    paused?: boolean;
    onShowUpdate: () => void;
    onDismissNotice: (id: number) => void;
  } = $props();
  /** The version whose banner was closed this session; a newer one shows again. */
  let dismissedUpdate = $state('');
  /** Like a notice: focus moves on first, so closing never drops it to <body>. */
  function dismissUpdate(event: MouseEvent, version: string) {
    const banner = (event.currentTarget as HTMLElement).closest<HTMLElement>(
      '.notice-banner',
    );
    if (banner?.contains(document.activeElement))
      focusAfterNotice(neighbourNotice(banner));
    dismissedUpdate = version;
  }
</script>

{#if update && !demo && dismissedUpdate !== update.version}<div
    class="alert subtle notice-banner"
    role="status"
  >
    <Icon name="refresh" size={16} /><span
      >Update {update.version} verfügbar</span
    ><button class="text-button" onclick={() => onShowUpdate()}>Ansehen</button
    ><button
      class="icon-button small"
      aria-label="Hinweis schließen"
      title="Hinweis schließen"
      onclick={(event) => dismissUpdate(event, update.version)}
      ><Icon name="close" size={13} /></button
    >
  </div>{/if}
{#if error}<div class="alert danger" role="alert">
    <Icon name="warning" size={16} /><span>{error}</span>
  </div>{/if}
{#if notice}<div class="alert warning" role="status">
    <Icon name="warning" size={16} /><span>{notice}</span>
  </div>{/if}
{#each notices as notice (notice.id)}
  <NoticeBanner
    text={notice.text}
    tone={notice.tone}
    actionLabel={notice.action?.label}
    onAction={notice.action?.run}
    navigates={false}
    timeout={notice.timeout}
    {paused}
    onDismiss={() => onDismissNotice(notice.id)}
  />
{/each}
