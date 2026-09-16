import {
  Box,
  Button,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  IconButton,
  Tooltip,
  Typography,
} from "@mui/material";
import RestartAltRoundedIcon from "@mui/icons-material/RestartAltRounded";
import LinkOffRoundedIcon from "@mui/icons-material/LinkOffRounded";
import { useEffect, useMemo, useRef, useState, type KeyboardEvent } from "react";
import { Keys } from "@/app/Keys";
import { chordFromEvent, isBindable, serializeChord } from "@/app/keymap";
import {
  bindingsOf,
  conflictsFor,
  setRecording,
  useShortcuts,
  type Command,
} from "@/app/shortcuts";
import { SearchField, SectionCard } from "@/components/ui";
import { omit } from "@/lib/store";
import type { Settings } from "@/ipc/types";
import { IS_MAC } from "@/lib/platform";

const MOD = IS_MAC ? "Cmd" : "Ctrl";
const ALT = IS_MAC ? "Option" : "Alt";
const SUPER = IS_MAC ? "Ctrl" : "Super";

interface Props {
  s: Settings;
  update: (patch: Partial<Settings>) => void;
}

const GROUP_ORDER = ["Navigation", "Tabs", "Panes", "Terminal", "Workspace", "Create", "Window"];

/**
 * Settings → Keyboard: every command with its current chord. Click a row to
 * record a new one; conflicts with another command are shown before saving,
 * and the other command is unbound if you go ahead. Ctrl+1…9 tab selection
 * is fixed and listed for reference only.
 */
export function KeyboardPage({ s, update }: Props) {
  const commands = useShortcuts((st) => st.commands);
  const [filter, setFilter] = useState("");
  const [editing, setEditing] = useState<Command | null>(null);

  const groups = useMemo(() => {
    const q = filter.trim().toLowerCase();
    const out = new Map<string, Command[]>();
    for (const c of commands) {
      if (q && !`${c.title} ${c.keywords ?? ""}`.toLowerCase().includes(q)) continue;
      out.set(c.group, [...(out.get(c.group) ?? []), c]);
    }
    return GROUP_ORDER.filter((g) => out.has(g)).map((g) => [g, out.get(g) ?? []] as const);
  }, [commands, filter]);

  const changed = Object.keys(s.shortcuts).length;
  const setBinding = (id: string, chord: string | null, unbindConflicts: string[] = []) => {
    const next = { ...s.shortcuts };
    for (const other of unbindConflicts) next[other] = "";
    if (chord !== null) next[id] = chord;
    update({ shortcuts: chord === null ? omit(next, id) : next });
  };

  return (
    <>
      <SectionCard
        title="Keyboard shortcuts"
        action={
          <Box sx={{ display: "flex", alignItems: "center", gap: 1 }}>
            <SearchField value={filter} onChange={setFilter} placeholder="Filter" width={200} />
            <Button
              variant="text"
              size="small"
              disabled={changed === 0}
              onClick={() => update({ shortcuts: {} })}
            >
              Reset all
            </Button>
          </Box>
        }
      >
        <Typography variant="body2" color="text.secondary">
          Click a shortcut to change it. Shortcuts need {MOD}, {ALT} or {SUPER} (or an F-key) so
          they never swallow what you type into a terminal. {MOD}+1 … {MOD}+9 always switch to the
          tab in that position.
        </Typography>
        {groups.map(([group, list]) => (
          <Box key={group}>
            <Typography
              variant="caption"
              sx={{ display: "block", color: "text.secondary", fontWeight: 600, mb: 0.5 }}
            >
              {group}
            </Typography>
            {list.map((c) => (
              <ShortcutRow
                key={c.id}
                command={c}
                bindings={bindingsOf(c, s.shortcuts)}
                overridden={c.id in s.shortcuts}
                onEdit={() => setEditing(c)}
                onReset={() => setBinding(c.id, null)}
                onUnbind={() => setBinding(c.id, "")}
              />
            ))}
          </Box>
        ))}
        {groups.length === 0 && (
          <Typography variant="body2" color="text.secondary">
            No commands match “{filter}”.
          </Typography>
        )}
      </SectionCard>
      {editing && (
        <RecordDialog
          command={editing}
          overrides={s.shortcuts}
          onClose={() => setEditing(null)}
          onSave={(chord, conflicts) => {
            setBinding(editing.id, chord, conflicts);
            setEditing(null);
          }}
        />
      )}
    </>
  );
}

function ShortcutRow({
  command,
  bindings,
  overridden,
  onEdit,
  onReset,
  onUnbind,
}: {
  command: Command;
  bindings: string[];
  overridden: boolean;
  onEdit: () => void;
  onReset: () => void;
  onUnbind: () => void;
}) {
  return (
    <Box
      sx={{
        display: "flex",
        alignItems: "center",
        gap: 1,
        height: 36,
        px: 1,
        mx: -1,
        borderRadius: 1.5,
        "&:hover": { bgcolor: "action.hover" },
        "&:hover .row-actions": { opacity: 1 },
      }}
    >
      <Typography variant="body2" sx={{ flex: 1, minWidth: 0 }} noWrap>
        {command.title}
      </Typography>
      <Box
        component="button"
        onClick={onEdit}
        aria-label={`Change shortcut for ${command.title}`}
        sx={{
          all: "unset",
          cursor: "pointer",
          display: "flex",
          alignItems: "center",
          gap: 0.75,
          minWidth: 120,
          justifyContent: "flex-end",
          borderRadius: 1,
          px: 0.5,
          py: 0.25,
          "&:hover, &:focus-visible": { bgcolor: "action.selected" },
        }}
      >
        {bindings.length === 0 ? (
          <Typography variant="caption" color="text.disabled">
            Not bound
          </Typography>
        ) : (
          bindings.map((b) => <Keys key={b} chord={b} />)
        )}
      </Box>
      <Box className="row-actions" sx={{ display: "flex", opacity: 0, width: 56 }}>
        <Tooltip title="Unbind">
          <span>
            <IconButton size="small" disabled={bindings.length === 0} onClick={onUnbind}>
              <LinkOffRoundedIcon sx={{ fontSize: 16 }} />
            </IconButton>
          </span>
        </Tooltip>
        <Tooltip title="Reset to default">
          <span>
            <IconButton size="small" disabled={!overridden} onClick={onReset}>
              <RestartAltRoundedIcon sx={{ fontSize: 16 }} />
            </IconButton>
          </span>
        </Tooltip>
      </Box>
    </Box>
  );
}

function RecordDialog({
  command,
  overrides,
  onClose,
  onSave,
}: {
  command: Command;
  overrides: Record<string, string>;
  onClose: () => void;
  onSave: (chord: string, unbind: string[]) => void;
}) {
  const [chord, setChord] = useState<string | null>(null);
  const [rejected, setRejected] = useState(false);
  const box = useRef<HTMLDivElement>(null);

  const conflicts = chord ? conflictsFor(chord, command.id, overrides) : [];

  const onKey = (e: KeyboardEvent<HTMLDivElement>) => {
    e.preventDefault();
    e.stopPropagation();
    if (e.key === "Escape" && !e.ctrlKey && !e.altKey && !e.metaKey) {
      onClose();
      return;
    }
    if (e.key === "Enter" && chord && !e.ctrlKey && !e.altKey && !e.metaKey && !e.shiftKey) {
      onSave(
        chord,
        conflicts.map((c) => c.id),
      );
      return;
    }
    const next = chordFromEvent(e.nativeEvent);
    if (!next) return;
    if (!isBindable(next)) {
      setRejected(true);
      return;
    }
    setRejected(false);
    setChord(serializeChord(next));
  };

  useEffect(() => {
    setRecording(true);
    return () => setRecording(false);
  }, []);

  return (
    <Dialog
      open
      onClose={onClose}
      maxWidth="xs"
      fullWidth
      slotProps={{ transition: { onEntered: () => box.current?.focus() } }}
    >
      <DialogTitle>{command.title}</DialogTitle>
      <DialogContent>
        <Box
          ref={box}
          tabIndex={0}
          onKeyDown={onKey}
          sx={{
            mt: 0.5,
            height: 64,
            borderRadius: 2,
            bgcolor: "surface.lowest",
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            outline: "2px solid",
            outlineColor: "primary.main",
          }}
        >
          {chord ? (
            <Keys chord={chord} />
          ) : (
            <Typography variant="body2" color="text.secondary">
              Press the new shortcut…
            </Typography>
          )}
        </Box>
        <Typography
          variant="caption"
          sx={{ display: "block", mt: 1, color: rejected ? "warning.main" : "text.secondary" }}
        >
          {rejected
            ? `Add ${MOD}, ${ALT} or ${SUPER} — plain keys would be typed into the terminal.`
            : conflicts.length > 0
              ? `Already used by ${conflicts.map((c) => `“${c.title}”`).join(", ")} — saving will unbind it.`
              : "Enter saves, Esc cancels."}
        </Typography>
      </DialogContent>
      <DialogActions>
        <Button onClick={onClose}>Cancel</Button>
        <Button
          variant="contained"
          disabled={!chord}
          onClick={() =>
            chord &&
            onSave(
              chord,
              conflicts.map((c) => c.id),
            )
          }
        >
          {conflicts.length > 0 ? "Replace" : "Save"}
        </Button>
      </DialogActions>
    </Dialog>
  );
}
