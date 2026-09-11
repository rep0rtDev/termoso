// Monospace fonts shipped with the app (all OFL / Apache / MIT) plus the
// Symbols Nerd Font glyph range as a trailing fallback, so prompts from
// starship / powerlevel10k / oh-my-posh render without a patched font.
// Settings store the display name; any installed system font works too.

import "@fontsource-variable/jetbrains-mono";
import "@fontsource-variable/fira-code";
import "@fontsource/fira-mono/400.css";
import "@fontsource/fira-mono/700.css";
import "@fontsource-variable/source-code-pro";
import "@fontsource-variable/cascadia-code";
import "@fontsource-variable/inconsolata";
import "@fontsource/ubuntu-mono/400.css";
import "@fontsource/ubuntu-mono/700.css";
import "@fontsource/dejavu-mono/400.css";
import "@fontsource/dejavu-mono/700.css";
import "@fontsource/pt-mono/400.css";
import "@fontsource/anonymous-pro/400.css";
import "@fontsource/anonymous-pro/700.css";
import "@fontsource-variable/roboto-mono";
import "@fontsource/ibm-plex-mono/400.css";
import "@fontsource/ibm-plex-mono/600.css";
import "@fontsource/cousine/400.css";
import "@fontsource/cousine/700.css";
import "@fontsource-variable/geist-mono";
import "@fontsource/intel-one-mono/400.css";
import "@fontsource/intel-one-mono/700.css";
import "@fontsource-variable/victor-mono";
import "@fontsource/space-mono/400.css";
import "@fontsource/space-mono/700.css";
import "@fontsource/commit-mono/400.css";
import "@fontsource/commit-mono/700.css";
import "@fontsource/monaspace-neon/400.css";
import "@fontsource/monaspace-neon/700.css";
import "@fontsource-variable/red-hat-mono";
import "@fontsource-variable/noto-sans-mono";
import "./nerdfont.css";

export interface TerminalFont {
  /** Display name, stored in settings. */
  name: string;
  /** CSS family the bundle registers under (variable builds get a suffix). */
  family: string;
  /** Has programming ligatures (xterm renders them with the ligatures addon only). */
  ligatures?: boolean;
}

const defaultFont: TerminalFont = {
  name: "JetBrains Mono",
  family: "JetBrains Mono Variable",
  ligatures: true,
};

export const bundledFonts: readonly TerminalFont[] = [
  defaultFont,
  { name: "Fira Code", family: "Fira Code Variable", ligatures: true },
  { name: "Fira Mono", family: "Fira Mono" },
  { name: "Cascadia Code", family: "Cascadia Code Variable", ligatures: true },
  { name: "Source Code Pro", family: "Source Code Pro Variable" },
  { name: "IBM Plex Mono", family: "IBM Plex Mono" },
  { name: "Roboto Mono", family: "Roboto Mono Variable" },
  { name: "Inconsolata", family: "Inconsolata Variable" },
  { name: "Ubuntu Mono", family: "Ubuntu Mono" },
  { name: "DejaVu Sans Mono", family: "DejaVu Mono" },
  { name: "Cousine", family: "Cousine" },
  { name: "PT Mono", family: "PT Mono" },
  { name: "Anonymous Pro", family: "Anonymous Pro" },
  { name: "Geist Mono", family: "Geist Mono Variable" },
  { name: "Intel One Mono", family: "Intel One Mono" },
  { name: "Commit Mono", family: "Commit Mono" },
  { name: "Monaspace Neon", family: "Monaspace Neon", ligatures: true },
  { name: "Red Hat Mono", family: "Red Hat Mono Variable" },
  { name: "Noto Sans Mono", family: "Noto Sans Mono Variable" },
  { name: "Victor Mono", family: "Victor Mono Variable", ligatures: true },
  { name: "Space Mono", family: "Space Mono" },
];

export const NERD_FONT_FAMILY = "Symbols Nerd Font Mono";

const systemStack = ["ui-monospace", "SFMono-Regular", "Menlo", "Consolas", "monospace"];

const quote = (f: string) => (/^[\w-]+$/.test(f) ? f : `'${f}'`);

export function bundledFont(name: string | null | undefined): TerminalFont | undefined {
  const key = name?.trim().toLowerCase();
  if (!key) return undefined;
  return bundledFonts.find((f) => f.name.toLowerCase() === key || f.family.toLowerCase() === key);
}

/** CSS font-family for xterm: the chosen face, Nerd Font symbols, then system monospace. */
export function terminalFontStack(name: string | null | undefined): string {
  const bundled = bundledFont(name);
  const chosen = bundled ? [bundled.family] : name?.trim() ? [name.trim()] : [];
  const families = [...chosen, ...(bundled ? [] : [defaultFont.family]), NERD_FONT_FAMILY];
  return [...families.map(quote), ...systemStack].join(", ");
}
