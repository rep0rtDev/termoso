// Top-level navigation: which top tab is showing (vaults home / SFTP / a
// terminal tab) and which sidebar section is open inside the vaults tab.

import { createStore, useStore } from "@/lib/store";
import { HOME_TAB, setActiveTab, terminalStore } from "@/terminal/store";

export type Section =
  "hosts" | "keychain" | "forwarding" | "snippets" | "knownHosts" | "logs" | "settings";

/** `terminalStore.activeTabId` is the source of truth for the top tab; the SFTP
 *  tab is one more pseudo id alongside `HOME_TAB`. */
export const SFTP_TAB = "sftp";

interface NavState {
  section: Section;
  /** Settings sub-page (Settings is a page group like Termius' settings window). */
  settingsPage: SettingsPage;
}

export type SettingsPage = "account" | "general" | "terminal" | "logs" | "updates" | "about";

export const navStore = createStore<NavState>({ section: "hosts", settingsPage: "account" });

export const useNav = <S>(selector: (s: NavState) => S) => useStore(navStore, selector);

export function goToSection(section: Section) {
  navStore.set((s) => (s.section === section ? s : { ...s, section }));
  setActiveTab(HOME_TAB);
}

export function goToSettings(page?: SettingsPage) {
  navStore.set((s) => ({ ...s, section: "settings", settingsPage: page ?? s.settingsPage }));
  setActiveTab(HOME_TAB);
}

export function setSettingsPage(page: SettingsPage) {
  navStore.set((s) => (s.settingsPage === page ? s : { ...s, settingsPage: page }));
}

export function goToSftp() {
  setActiveTab(SFTP_TAB);
}

export function goHome() {
  setActiveTab(HOME_TAB);
}

export const isHomeTab = (id: string) => id === HOME_TAB;
export const isSftpTab = (id: string) => id === SFTP_TAB;

export function activeTopTab(): string {
  return terminalStore.get().activeTabId;
}
