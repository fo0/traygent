import type { SettingsTab } from './settingsTabs';
import type { AgentId } from './types';

export type ModalKind = 'settings' | 'recent';

/**
 * Which dialog is open and where focus returns afterwards. Dialogs own only
 * their form state; running operations live in the Panel and outlive them.
 */
export class Modals {
  kind = $state<ModalKind | null>(null);
  settingsTab = $state<SettingsTab>('agents');
  /** The agent the „Agenten“ tab opens at, e.g. the one to install. */
  settingsAgent = $state<AgentId | null>(null);
  /** Opened by an „Installieren …“: the dialog closes once the install started. */
  installFlow = $state(false);
  /** The id of the „Zuletzt“ entry „Bearbeiten …“ opened. */
  recentId = $state<string | null>(null);
  /** Element that opened the first of possibly chained dialogs. */
  opener: HTMLElement | null = null;

  returnFocus = () => this.opener;
  close = () => {
    this.kind = null;
  };
  /** Opens or replaces the dialog; a replacement keeps the first opener. */
  show = (kind: ModalKind) => {
    if (!this.kind)
      this.opener =
        document.activeElement instanceof HTMLElement
          ? document.activeElement
          : null;
    this.kind = kind;
  };
  openSettings = (
    initial: SettingsTab = 'agents',
    agent: AgentId | null = null,
  ) => {
    this.settingsTab = initial;
    this.settingsAgent = agent;
    this.installFlow = false;
    this.show('settings');
  };
  /** „Bearbeiten …“ of a „Zuletzt“ row. */
  openRecent = (id: string) => {
    this.recentId = id;
    this.show('recent');
  };
  /** „Installieren …“ of the composer or a row: Agenten at that agent. */
  openInstall = (agent: AgentId) => {
    this.openSettings('agents', agent);
    this.installFlow = true;
  };
}
