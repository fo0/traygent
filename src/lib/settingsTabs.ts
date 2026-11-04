import type { IconName } from './Icon.svelte';

/** The settings dialog's tabs, in order; the gear opens the first one. */
export const SETTINGS_TABS = [
  { id: 'agents', label: 'Agenten', icon: 'robot' },
  { id: 'environment', label: 'Umgebung', icon: 'monitor' },
  { id: 'github', label: 'GitHub', icon: 'github' },
  { id: 'app', label: 'App', icon: 'settings' },
  { id: 'updates', label: 'Updates', icon: 'refresh' },
] as const satisfies readonly { id: string; label: string; icon: IconName }[];
export type SettingsTab = (typeof SETTINGS_TABS)[number]['id'];
