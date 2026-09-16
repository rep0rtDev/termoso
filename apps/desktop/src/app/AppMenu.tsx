import { Box } from "@mui/material";
import { useState, type ReactNode } from "react";
import { ActionMenu, type MenuAction } from "@/components/ui";
import { LogoMark } from "@/components/Logo";
import { Keys } from "./Keys";
import { bindingsOf, useShortcuts, type Command } from "./shortcuts";

/** `null` = separator before the next entry. */
type Entry = string | null;

const MENU: { label: string; entries: Entry[] }[] = [
  {
    label: "File",
    entries: [
      "new.host",
      "new.group",
      "new.snippet",
      null,
      "tab.new",
      "tab.local",
      "tab.duplicate",
      "tab.close",
      null,
      "nav.settings",
      "window.quit",
    ],
  },
  {
    label: "Edit",
    entries: ["term.copy", "term.paste", "term.selectAll", null, "term.find", "term.clear"],
  },
  {
    label: "View",
    entries: [
      "palette.commands",
      "palette.jump",
      null,
      "nav.hosts",
      "nav.sftp",
      "nav.keychain",
      "nav.forwarding",
      "nav.snippets",
      "nav.knownHosts",
      "nav.logs",
      null,
      "term.zoomIn",
      "term.zoomOut",
      "term.zoomReset",
      "term.sidePanel",
      "term.askAi",
      null,
      "window.fullscreen",
    ],
  },
  {
    label: "Window",
    entries: [
      "pane.splitRight",
      "pane.splitDown",
      "pane.close",
      "pane.detach",
      "pane.broadcast",
      null,
      "tab.next",
      "tab.prev",
      null,
      "ws.viewMode",
      "ws.saveTemplate",
      "ws.close",
      null,
      "window.minimize",
      "window.maximize",
    ],
  },
  {
    label: "Help",
    entries: ["nav.docs", "nav.keyboard", "nav.themes", "nav.about"],
  },
];

function Label({ cmd, keys }: { cmd: Command; keys: string[] }): ReactNode {
  return (
    <Box sx={{ display: "flex", alignItems: "center", gap: 3, width: "100%" }}>
      <Box component="span" sx={{ flex: 1 }}>
        {cmd.title}
      </Box>
      {keys[0] && <Keys chord={keys[0]} />}
    </Box>
  );
}

/**
 * Application menu behind the logo in the top bar — File / Edit / View /
 * Window / Help built from the same command registry as the palette and the
 * keyboard settings, so labels, availability and shortcuts never drift.
 */
export function AppMenuButton() {
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);
  const commands = useShortcuts((s) => s.commands);
  const overrides = useShortcuts((s) => s.overrides);
  const byId = new Map(commands.map((c) => [c.id, c]));

  const items: MenuAction[] = MENU.map((menu) => {
    const list: MenuAction[] = [];
    let divider = false;
    for (const e of menu.entries) {
      if (e === null) {
        divider = true;
        continue;
      }
      const cmd = byId.get(e);
      if (!cmd) continue;
      const prev = list.at(-1);
      if (divider && prev) prev.divider = true;
      divider = false;
      list.push({
        label: <Label cmd={cmd} keys={bindingsOf(cmd, overrides)} />,
        disabled: cmd.enabled ? !cmd.enabled() : false,
        onClick: cmd.run,
      });
    }
    return { label: menu.label, items: list };
  });

  return (
    <>
      <Box
        component="button"
        aria-label="Application menu"
        aria-haspopup="menu"
        onClick={(e) => setAnchor(e.currentTarget)}
        sx={{
          all: "unset",
          display: "flex",
          alignItems: "center",
          px: 0.75,
          borderRadius: 1.5,
          cursor: "pointer",
          "& svg": { pointerEvents: "none" },
          "&:hover, &[aria-expanded='true']": { bgcolor: "action.hover" },
        }}
        aria-expanded={anchor ? "true" : "false"}
      >
        <LogoMark size={22} />
      </Box>
      <ActionMenu anchor={anchor} onClose={() => setAnchor(null)} items={items} />
    </>
  );
}
