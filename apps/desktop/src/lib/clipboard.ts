import { writeText as writeNativeClipboard } from "@tauri-apps/plugin-clipboard-manager";

/**
 * Copies text via the native clipboard first; WebKitGTK denies
 * `navigator.clipboard.writeText` outside a user-gesture permission grant.
 * Rejects only when both paths fail.
 */
export async function copyToClipboard(text: string): Promise<void> {
  try {
    await writeNativeClipboard(text);
  } catch {
    await navigator.clipboard.writeText(text);
  }
}
