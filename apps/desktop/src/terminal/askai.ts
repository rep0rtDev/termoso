// Pure helpers behind the "Ask AI" side panel: error wording, labels, limits.

import type { AiStatus } from "@/ipc/types";
import { errorMessage, isDesktopError } from "@/ipc/types";
import { tr } from "@/i18n";

/** Server-side limit on the request text; mirrored so the field stops there. */
export const MAX_PROMPT_CHARS = 500;

/** Human wording for the AI error kinds `termoso-client` maps from the API. */
export function aiErrorText(e: unknown): { text: string; retry: boolean } {
  if (isDesktopError(e)) {
    switch (e.kind) {
      case "ai_not_enabled":
        return { text: tr("AI suggestions are turned off for this account."), retry: false };
      case "ai_quota_exceeded":
        return {
          text: tr("Today's quota is used up. It resets at midnight UTC."),
          retry: false,
        };
      case "ai_busy":
        return { text: tr("The AI provider is busy. Try again in a moment."), retry: true };
      case "ai_unavailable":
        return {
          text: tr("The AI provider did not answer. Nothing was inserted."),
          retry: true,
        };
      case "unauthorized":
        return { text: tr("Sign in again to use AI suggestions."), retry: false };
    }
  }
  return { text: errorMessage(e), retry: true };
}

/** `Chutes · GLM-4.7 …` style label for the provider on offer. */
export function providerLabel(s: Pick<AiStatus, "provider" | "model">): string {
  return [s.provider, s.model].filter((x): x is string => !!x).join(" · ") || tr("AI provider");
}

/** What leaves the machine for this pane, as shown to the user. */
export function contextLabel(pane: { protocol: string | null; shell: string | null }): string {
  const os = pane.protocol === "local" ? tr("this computer") : tr("host OS");
  return pane.shell ? `${os} · ${pane.shell}` : os;
}

export function remainingToday(s: Pick<AiStatus, "daily_quota" | "used_today">): number {
  return Math.max(0, s.daily_quota - s.used_today);
}
