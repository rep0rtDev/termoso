// Queue of prompts raised by Rust while a session connects (host key, password,
// passphrase, PIN, keyboard-interactive). Answers go straight back to Rust;
// nothing typed in the UI is retained here.

import { createStore, useStore } from "@/lib/store";
import { onPrompt, onPromptClosed, promptAnswer } from "@/ipc/commands";
import type { PromptAnswer, PromptEvent, Uuid } from "@/ipc/types";
import { setActiveTab, terminalStore } from "@/terminal/store";

interface PromptState {
  queue: PromptEvent[];
}

export const promptStore = createStore<PromptState>({ queue: [] });

export const usePrompts = <S>(selector: (s: PromptState) => S) => useStore(promptStore, selector);

/** First open prompt of a session (the one its connection view answers inline). */
export const promptForSession = (s: PromptState, sessionId: Uuid) =>
  s.queue.find((p) => p.session_id === sessionId) ?? null;

export function answerPrompt(id: Uuid, answer: PromptAnswer) {
  promptStore.set((s) => ({ queue: s.queue.filter((p) => p.id !== id) }));
  void promptAnswer(id, answer);
}

let started: Promise<void> | null = null;

/** Subscribe to the Rust prompt events once for the app's lifetime (started on import). */
function startPromptListener(): Promise<void> {
  if (started) return started;
  started = Promise.all([
    onPrompt((p) => {
      promptStore.set((s) => (s.queue.some((x) => x.id === p.id) ? s : { queue: [...s.queue, p] }));
      const tab = terminalStore.get().tabs.find((t) => t.paneIds.includes(p.session_id));
      if (tab) setActiveTab(tab.id);
    }),
    onPromptClosed((c) =>
      promptStore.set((s) => ({ queue: s.queue.filter((x) => x.id !== c.id) })),
    ),
  ]).then(() => undefined);
  return started;
}

void startPromptListener();
