/**
 * The free, hosted Termoso server. Only ever contacted when the user picks it
 * on the sign-in screen — the app makes no network calls on its own.
 */
export const CLOUD_URL = "https://app.termoso.com";

export const CLOUD_HOST = CLOUD_URL.replace(/^https?:\/\//, "");

/** Trims whitespace and trailing slashes; returns null unless it is an http(s) URL. */
export function normalizeServerUrl(raw: string): string | null {
  const url = raw.trim().replace(/\/+$/, "");
  return /^https?:\/\/\S+$/.test(url) ? url : null;
}
