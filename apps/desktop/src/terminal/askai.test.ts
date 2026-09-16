import { describe, expect, it } from "vitest";
import {
  MAX_PROMPT_CHARS,
  aiErrorText,
  contextLabel,
  providerLabel,
  remainingToday,
} from "./askai";

describe("aiErrorText", () => {
  it("maps the stable API kinds to wording and retryability", () => {
    expect(aiErrorText({ kind: "ai_not_enabled", message: "x" })).toEqual({
      text: "AI suggestions are turned off for this account.",
      retry: false,
    });
    expect(aiErrorText({ kind: "ai_quota_exceeded", message: "x" }).retry).toBe(false);
    expect(aiErrorText({ kind: "ai_busy", message: "x" }).retry).toBe(true);
    expect(aiErrorText({ kind: "ai_unavailable", message: "x" }).retry).toBe(true);
    expect(aiErrorText({ kind: "unauthorized", message: "x" }).retry).toBe(false);
  });

  it("never repeats provider payloads: unknown kinds fall back to the server message", () => {
    expect(aiErrorText({ kind: "api", message: "boom" })).toEqual({ text: "boom", retry: true });
    expect(aiErrorText(new Error("offline"))).toEqual({ text: "offline", retry: true });
  });
});

describe("labels", () => {
  it("joins provider and model, with a fallback", () => {
    expect(providerLabel({ provider: "Chutes", model: "GLM-4.7-Flash" })).toBe(
      "Chutes · GLM-4.7-Flash",
    );
    expect(providerLabel({ provider: "Chutes", model: null })).toBe("Chutes");
    expect(providerLabel({ provider: null, model: null })).toBe("AI provider");
  });

  it("describes what is sent without naming the host", () => {
    expect(contextLabel({ protocol: "ssh", shell: "zsh" })).toBe("host OS · zsh");
    expect(contextLabel({ protocol: "ssh", shell: null })).toBe("host OS");
    expect(contextLabel({ protocol: "local", shell: "bash" })).toBe("this computer · bash");
  });

  it("clamps the remaining quota at zero", () => {
    expect(remainingToday({ daily_quota: 50, used_today: 3 })).toBe(47);
    expect(remainingToday({ daily_quota: 50, used_today: 60 })).toBe(0);
    expect(MAX_PROMPT_CHARS).toBe(500);
  });
});
