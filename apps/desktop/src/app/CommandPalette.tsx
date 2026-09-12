import { Box, Dialog, InputBase, Typography } from "@mui/material";
import SearchRoundedIcon from "@mui/icons-material/SearchRounded";
import KeyboardCommandKeyRoundedIcon from "@mui/icons-material/KeyboardCommandKeyRounded";
import TabRoundedIcon from "@mui/icons-material/TabRounded";
import DashboardCustomizeRoundedIcon from "@mui/icons-material/DashboardCustomizeRounded";
import HistoryRoundedIcon from "@mui/icons-material/HistoryRounded";
import BoltRoundedIcon from "@mui/icons-material/BoltRounded";
import TerminalRoundedIcon from "@mui/icons-material/TerminalRounded";
import ArrowForwardRoundedIcon from "@mui/icons-material/ArrowForwardRounded";
import AddRoundedIcon from "@mui/icons-material/AddRounded";
import WebAssetRoundedIcon from "@mui/icons-material/WebAssetRounded";
import { useEffect, useMemo, useRef, useState, type KeyboardEvent, type ReactNode } from "react";
import { useHistory, useHosts } from "@/ipc/hooks";
import type { HostCard } from "@/ipc/types";
import { HostAvatar } from "@/hosts/HostAvatar";
import { looksLikeTarget, parseQuickConnect, quickFromHistory, quickLabel } from "@/hosts/links";
import { openTerminal, setActiveTab, useTerminal } from "@/terminal/store";
import { openTemplate, useWorkspaces } from "@/terminal/workspaces";
import { sizes } from "@/theme/theme";
import { Keys } from "./Keys";
import { bindingsOf, useShortcuts } from "./shortcuts";
import { closePalette, usePalette, type PaletteMode } from "./commands";

interface Item {
  key: string;
  group: string;
  title: string;
  subtitle?: string;
  /** Extra search terms that are matched but never displayed. */
  keywords?: string;
  icon: ReactNode;
  keys?: string[];
  run: () => void;
}

const MAX_ITEMS = 40;

/** 0 = no match; higher is better. Prefix > word start > substring > subsequence. */
function score(text: string, q: string): number {
  const t = text.toLowerCase();
  if (!q) return 1;
  if (t.startsWith(q)) return 100 - t.length / 100;
  const at = t.indexOf(q);
  if (at >= 0) return (t[at - 1] === " " ? 80 : 60) - at / 100;
  let i = 0;
  for (const ch of t) if (ch === q[i]) i++;
  return i === q.length ? 20 - t.length / 100 : 0;
}

function rank(items: Item[], q: string): Item[] {
  const needle = q.trim().toLowerCase();
  if (!needle) return items.slice(0, MAX_ITEMS);
  const scored = items
    .map((it) => ({
      it,
      s: score(`${it.title} ${it.subtitle ?? ""} ${it.keywords ?? ""}`, needle),
    }))
    .filter((x) => x.s > 0)
    .sort((a, b) => b.s - a.s)
    .slice(0, MAX_ITEMS);

  // Keep each group contiguous, groups ordered by their best hit.
  const best = new Map<string, number>();
  for (const x of scored) if (!best.has(x.it.group)) best.set(x.it.group, x.s);
  return scored
    .sort((a, b) => (best.get(b.it.group) ?? 0) - (best.get(a.it.group) ?? 0) || b.s - a.s)
    .map((x) => x.it);
}

const PLACEHOLDER: Record<PaletteMode, string> = {
  commands: "Type a command…",
  jump: "Jump to a host, tab or workspace…  (type > for commands)",
};

/**
 * Quick launcher shared by "Command palette" and "Jump to": a search box over
 * either every enabled command or everything you can switch to / connect to.
 * `>` at the start of a Jump query flips to commands, as in editors.
 */
export function CommandPalette() {
  const open = usePalette((s) => s.open);
  return open ? <PaletteDialog mode={open} /> : null;
}

function PaletteDialog({ mode: initial }: { mode: PaletteMode }) {
  const [query, setQueryState] = useState("");
  const [index, setIndex] = useState(0);
  const listRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const setQuery = (v: string) => {
    setQueryState(v);
    setIndex(0);
  };

  const mode: PaletteMode = initial === "jump" && query.startsWith(">") ? "commands" : initial;
  const q = mode !== initial ? query.slice(1) : query;

  const commands = useCommandItems();
  const jump = useJumpItems(q);
  const items = useMemo(
    () => rank(mode === "commands" ? commands : jump, q),
    [mode, commands, jump, q],
  );

  const current = Math.min(index, Math.max(items.length - 1, 0));
  useEffect(() => {
    listRef.current
      ?.querySelector<HTMLElement>(`[data-index="${current}"]`)
      ?.scrollIntoView({ block: "nearest" });
  }, [current]);

  const pick = (it: Item | undefined) => {
    if (!it) return;
    closePalette();
    it.run();
  };
  const onKey = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setIndex((i) => (items.length ? (i + 1) % items.length : 0));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setIndex((i) => (items.length ? (i - 1 + items.length) % items.length : 0));
    } else if (e.key === "Enter") {
      e.preventDefault();
      pick(items[current]);
    } else if (e.key === "Escape") {
      e.preventDefault();
      closePalette();
    }
  };

  return (
    <Dialog
      open
      onClose={closePalette}
      maxWidth={false}
      sx={{ "& .MuiDialog-container": { alignItems: "flex-start", pt: "9vh" } }}
      slotProps={{
        transition: { onEntered: () => inputRef.current?.focus() },
        paper: {
          sx: {
            width: 600,
            maxWidth: "calc(100vw - 48px)",
            bgcolor: "surface.high",
            borderRadius: 2.5,
            overflow: "hidden",
            boxShadow: "0 16px 48px rgba(0,0,0,0.38)",
          },
        },
      }}
    >
      <Box
        sx={{
          display: "flex",
          alignItems: "center",
          gap: 1,
          height: 48,
          px: 1.75,
          borderBottom: 1,
          borderColor: "border.light",
        }}
      >
        {mode === "commands" ? (
          <KeyboardCommandKeyRoundedIcon sx={{ fontSize: 18, color: "text.secondary" }} />
        ) : (
          <SearchRoundedIcon sx={{ fontSize: 20, color: "text.secondary" }} />
        )}
        <InputBase
          autoFocus
          inputRef={inputRef}
          fullWidth
          value={query}
          placeholder={PLACEHOLDER[initial]}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={onKey}
          inputProps={{ "aria-label": PLACEHOLDER[initial], spellCheck: false }}
          sx={{ fontSize: 14, "& input": { p: 0, height: "100%" } }}
        />
      </Box>
      <Box ref={listRef} sx={{ maxHeight: "52vh", overflowY: "auto", py: 0.75 }}>
        {items.length === 0 && (
          <Typography
            variant="body2"
            color="text.secondary"
            sx={{ px: 2, py: 2.5, textAlign: "center" }}
          >
            {mode === "commands" ? "No matching commands" : "Nothing matches"}
          </Typography>
        )}
        {items.map((it, i) => {
          const header = it.group !== items[i - 1]?.group ? it.group : null;
          return (
            <Box key={it.key}>
              {header && (
                <Typography
                  variant="caption"
                  sx={{
                    display: "block",
                    px: 2,
                    pt: i === 0 ? 0.5 : 1.25,
                    pb: 0.5,
                    color: "text.secondary",
                    fontWeight: 600,
                  }}
                >
                  {header}
                </Typography>
              )}
              <Row item={it} active={i === current} index={i} onHover={setIndex} onPick={pick} />
            </Box>
          );
        })}
      </Box>
    </Dialog>
  );
}

function Row({
  item,
  active,
  index,
  onHover,
  onPick,
}: {
  item: Item;
  active: boolean;
  index: number;
  onHover: (i: number) => void;
  onPick: (it: Item) => void;
}) {
  const chord = item.keys?.[0];
  return (
    <Box
      data-index={index}
      onMouseMove={() => onHover(index)}
      onClick={() => onPick(item)}
      sx={{
        mx: 0.75,
        px: 1.25,
        height: 40,
        display: "flex",
        alignItems: "center",
        gap: 1.25,
        borderRadius: 1.5,
        cursor: "pointer",
        bgcolor: active ? "action.selected" : "transparent",
      }}
    >
      <Box
        sx={{
          width: sizes.tileSmall,
          display: "flex",
          justifyContent: "center",
          color: "text.secondary",
          flexShrink: 0,
        }}
      >
        {item.icon}
      </Box>
      <Box sx={{ flex: 1, minWidth: 0, display: "flex", alignItems: "baseline", gap: 1 }}>
        <Typography variant="body2" noWrap sx={{ fontWeight: 500 }}>
          {item.title}
        </Typography>
        {item.subtitle && (
          <Typography variant="caption" color="text.secondary" noWrap>
            {item.subtitle}
          </Typography>
        )}
      </Box>
      {chord !== undefined && <Keys chord={chord} />}
    </Box>
  );
}

function useCommandItems(): Item[] {
  const commands = useShortcuts((s) => s.commands);
  const overrides = useShortcuts((s) => s.overrides);
  return useMemo(
    () =>
      commands
        .filter((c) => !c.enabled || c.enabled())
        .map((c) => ({
          key: c.id,
          group: c.group,
          title: c.title,
          keywords: c.keywords,
          icon: <GroupIcon group={c.group} />,
          keys: bindingsOf(c, overrides),
          run: c.run,
        })),
    [commands, overrides],
  );
}

function GroupIcon({ group }: { group: string }) {
  switch (group) {
    case "Tabs":
    case "Panes":
      return <TabRoundedIcon sx={{ fontSize: 18 }} />;
    case "Terminal":
      return <TerminalRoundedIcon sx={{ fontSize: 18 }} />;
    case "Workspace":
      return <DashboardCustomizeRoundedIcon sx={{ fontSize: 18 }} />;
    case "Create":
      return <AddRoundedIcon sx={{ fontSize: 18 }} />;
    case "Window":
      return <WebAssetRoundedIcon sx={{ fontSize: 18 }} />;
    case "Navigation":
      return <ArrowForwardRoundedIcon sx={{ fontSize: 18 }} />;
    default:
      return <KeyboardCommandKeyRoundedIcon sx={{ fontSize: 16 }} />;
  }
}

function useJumpItems(q: string): Item[] {
  const tabs = useTerminal((s) => s.tabs);
  const panes = useTerminal((s) => s.panes);
  const hosts = useHosts(null);
  const templates = useWorkspaces((s) => s.templates);
  const history = useHistory();

  return useMemo(() => {
    const hostById = new Map((hosts.data ?? []).map((h) => [h.id, h]));
    const out: Item[] = [];

    for (const t of tabs) {
      const pane = panes[t.activePaneId];
      const host = pane?.hostId ? hostById.get(pane.hostId) : undefined;
      const title = t.name ?? pane?.title ?? "Terminal";
      const extra = t.paneIds.length > 1 ? ` (+${t.paneIds.length - 1})` : "";
      out.push({
        key: `tab:${t.id}`,
        group: "Open tabs",
        title: t.name === null ? `${title}${extra}` : title,
        subtitle: t.name !== null ? "Workspace" : pane?.subtitle,
        icon: host ? (
          <HostAvatar host={host} size={sizes.tileSmall} />
        ) : (
          <TabRoundedIcon sx={{ fontSize: 18 }} />
        ),
        run: () => setActiveTab(t.id),
      });
    }

    for (const h of hosts.data ?? []) out.push(hostItem(h));

    for (const tpl of templates) {
      out.push({
        key: `tpl:${tpl.id}`,
        group: "Workspaces",
        title: tpl.name,
        subtitle: "Workspace template",
        icon: <DashboardCustomizeRoundedIcon sx={{ fontSize: 18 }} />,
        run: () => void openTemplate(tpl.id),
      });
    }

    const seen = new Set<string>();
    for (const it of history.data ?? []) {
      if (it.data.host_id || seen.has(it.data.target)) continue;
      seen.add(it.data.target);
      const target = quickFromHistory(it.data.target, it.data.protocol);
      if (!target) continue;
      out.push({
        key: `recent:${it.data.target}`,
        group: "Recent",
        title: quickLabel(target),
        subtitle: "Quick connect",
        icon: <HistoryRoundedIcon sx={{ fontSize: 18 }} />,
        run: () => openTerminal(target),
      });
    }

    const quick = looksLikeTarget(q.trim()) ? parseQuickConnect(q) : null;
    if (quick) {
      out.unshift({
        key: "quick",
        group: "Connect",
        title: quickLabel(quick),
        subtitle: quick.protocol === "telnet" ? "Quick connect · Telnet" : "Quick connect · SSH",
        icon: <BoltRoundedIcon sx={{ fontSize: 18 }} />,
        run: () => openTerminal(quick),
      });
    }
    out.push({
      key: "local",
      group: "Local",
      title: "Local terminal",
      subtitle: "New shell on this machine",
      icon: <TerminalRoundedIcon sx={{ fontSize: 18 }} />,
      run: () => openTerminal({ kind: "local" }),
    });
    return out;
  }, [tabs, panes, hosts.data, templates, history.data, q]);
}

function hostItem(h: HostCard): Item {
  return {
    key: `host:${h.id}`,
    group: "Hosts",
    title: h.label,
    subtitle: `${h.username ? `${h.username}@` : ""}${h.address}${h.tags.length ? ` · ${h.tags.join(", ")}` : ""}`,
    icon: <HostAvatar host={h} size={sizes.tileSmall} />,
    run: () => openTerminal({ kind: "host", host_id: h.id }),
  };
}
