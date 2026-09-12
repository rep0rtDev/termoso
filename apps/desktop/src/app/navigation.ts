// Top-level navigation: which top tab is showing (vaults home / SFTP / a
// terminal tab) and which sidebar section is open inside the vaults tab.

import { useEffect, useRef } from "react";
import { createStore, useStore } from "@/lib/store";
import { HOME_TAB, setActiveTab, terminalStore } from "@/terminal/store";
import type { Uuid } from "@/ipc/types";

export type Section =
  "hosts" | "keychain" | "forwarding" | "snippets" | "knownHosts" | "logs" | "settings";

/** `terminalStore.activeTabId` is the source of truth for the top tab; the SFTP
 *  tab is one more pseudo id alongside `HOME_TAB`. */
export const SFTP_TAB = "sftp";
/** The "New Tab" page opened by the `+` after the session tabs. */
export const NEW_TAB = "newtab";
/** The Serial connection page (Hosts toolbar → Serial); becomes a terminal on Connect. */
export const SERIAL_TAB = "serial";

interface NavState {
  section: Section;
  /** Settings sub-page (Settings is a page group like Termius' settings window). */
  settingsPage: SettingsPage;
  /** "New …" picked from the top bar; the section page consumes it. */
  pendingCreate: CreateKind | null;
  /** Host whose edit panel should open once the hosts page shows. */
  pendingEdit: Uuid | null;
  /** Host a new port-forwarding rule should be pre-filled with. */
  pendingForwardHost: Uuid | null;
  /** Something the Team / Vaults settings page should open as soon as it shows. */
  pendingSettingsIntent: SettingsIntent | null;
}

export type SettingsPage =
  | "team"
  | "account"
  | "vaults"
  | "general"
  | "terminal"
  | "keyboard"
  | "sftp"
  | "logs"
  | "updates"
  | "about";

export type SettingsIntent =
  { kind: "invite" } | { kind: "newVault"; teamId?: Uuid } | { kind: "vault"; id: Uuid };

export type CreateKind = "host" | "group" | "snippet";

export const navStore = createStore<NavState>({
  section: "hosts",
  settingsPage: "account",
  pendingCreate: null,
  pendingEdit: null,
  pendingForwardHost: null,
  pendingSettingsIntent: null,
});

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

/** Open Settings → Team with the invite dialog, or Settings → Vaults on a vault / new vault. */
export function goToSettingsWith(intent: SettingsIntent) {
  navStore.set((s) => ({
    ...s,
    section: "settings",
    settingsPage: intent.kind === "invite" ? "team" : "vaults",
    pendingSettingsIntent: intent,
  }));
  setActiveTab(HOME_TAB);
}

/** Run `onIntent` once for the pending settings intent of the given kinds. */
export function useSettingsIntent(
  kinds: SettingsIntent["kind"][],
  onIntent: (intent: SettingsIntent) => void,
) {
  const key = kinds.join(",");
  const cb = useRef(onIntent);
  useEffect(() => {
    cb.current = onIntent;
  });
  useEffect(() => {
    const take = () => {
      const intent = navStore.get().pendingSettingsIntent;
      if (!intent || !key.split(",").includes(intent.kind)) return;
      navStore.set((s) => ({ ...s, pendingSettingsIntent: null }));
      cb.current(intent);
    };
    const timer = setTimeout(take, 0);
    const unsubscribe = navStore.subscribe(take);
    return () => {
      clearTimeout(timer);
      unsubscribe();
    };
  }, [key]);
}

export function requestCreate(kind: CreateKind) {
  navStore.set((s) => ({
    ...s,
    section: kind === "snippet" ? "snippets" : "hosts",
    pendingCreate: kind,
  }));
  setActiveTab(HOME_TAB);
}

export function requestEditHost(id: Uuid) {
  navStore.set((s) => ({ ...s, section: "hosts", pendingEdit: id }));
  setActiveTab(HOME_TAB);
}

/** Open Port Forwarding with a new-rule dialog for `hostId`. */
export function requestForwardingRule(hostId: Uuid) {
  navStore.set((s) => ({ ...s, section: "forwarding", pendingForwardHost: hostId }));
  setActiveTab(HOME_TAB);
}

function usePendingHost(key: "pendingEdit" | "pendingForwardHost", onTake: (id: Uuid) => void) {
  const cb = useRef(onTake);
  useEffect(() => {
    cb.current = onTake;
  });
  useEffect(() => {
    const take = () => {
      const id = navStore.get()[key];
      if (!id) return;
      navStore.set((s) => ({ ...s, [key]: null }));
      cb.current(id);
    };
    const timer = setTimeout(take, 0);
    const unsubscribe = navStore.subscribe(take);
    return () => {
      clearTimeout(timer);
      unsubscribe();
    };
  }, [key]);
}

/** Open the edit panel for hosts requested via `requestEditHost`. */
export const useEditRequests = (onEdit: (id: Uuid) => void) =>
  usePendingHost("pendingEdit", onEdit);

/** Start a rule for the host requested via `requestForwardingRule`. */
export const useForwardRequests = (onRule: (hostId: Uuid) => void) =>
  usePendingHost("pendingForwardHost", onRule);

/**
 * Run `onCreate` for "New …" requests of the given kinds — both ones made
 * while the page is showing and one made just before it mounted.
 */
export function useCreateRequests(kinds: CreateKind[], onCreate: (kind: CreateKind) => void) {
  const key = kinds.join(",");
  const cb = useRef(onCreate);
  useEffect(() => {
    cb.current = onCreate;
  });
  useEffect(() => {
    const take = () => {
      const kind = navStore.get().pendingCreate;
      if (!kind || !key.split(",").includes(kind)) return;
      navStore.set((s) => ({ ...s, pendingCreate: null }));
      cb.current(kind);
    };
    const timer = setTimeout(take, 0);
    const unsubscribe = navStore.subscribe(take);
    return () => {
      clearTimeout(timer);
      unsubscribe();
    };
  }, [key]);
}

export function goToSftp() {
  setActiveTab(SFTP_TAB);
}

export function goToNewTab() {
  setActiveTab(NEW_TAB);
}

export function goToSerial() {
  setActiveTab(SERIAL_TAB);
}

export function goHome() {
  setActiveTab(HOME_TAB);
}

export const isHomeTab = (id: string) => id === HOME_TAB;
export const isSftpTab = (id: string) => id === SFTP_TAB;
export const isNewTab = (id: string) => id === NEW_TAB;
export const isSerialTab = (id: string) => id === SERIAL_TAB;

export function activeTopTab(): string {
  return terminalStore.get().activeTabId;
}
