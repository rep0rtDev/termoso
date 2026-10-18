import { useOpenFileRequests } from "@/app/navigation";
import { isThemeFile } from "@/app/deepLinks";
import { useState } from "react";
import {
  Box,
  Button,
  ButtonBase,
  IconButton,
  ToggleButton,
  ToggleButtonGroup,
  Tooltip,
  Typography,
} from "@mui/material";
import EditRoundedIcon from "@mui/icons-material/EditRounded";
import DeleteOutlineRoundedIcon from "@mui/icons-material/DeleteOutlineRounded";
import { open as openFile } from "@tauri-apps/plugin-dialog";
import { toast } from "@/components/Snackbar";
import * as ipc from "@/ipc/commands";
import { useColorSchemes, useDeleteColorScheme, useSaveColorScheme, useVaults } from "@/ipc/hooks";
import { errorMessage, type ColorSchemeData, type Uuid } from "@/ipc/types";
import { ThemeEditorDialog } from "./ThemeEditorDialog";
import { useColorScheme } from "@mui/material";
import CheckRoundedIcon from "@mui/icons-material/CheckRounded";
import { SectionCard } from "@/components/ui";
import {
  AUTO_THEME,
  customTheme,
  resolveTerminalTheme,
  terminalThemes,
  type TerminalTheme,
} from "@/terminal/themes";
import { tr } from "@/i18n";

type Filter = "all" | "dark" | "light";

/** Grid of colour scheme cards; `auto` follows the app theme with the Termoso pair. */
export function ThemeGallery({
  value,
  onChange,
}: {
  value: string;
  onChange: (id: string) => void;
}) {
  const [filter, setFilter] = useState<Filter>("all");
  const { mode, systemMode } = useColorScheme();
  const scheme = (mode === "system" ? systemMode : mode) === "light" ? "light" : "dark";
  const auto = resolveTerminalTheme(AUTO_THEME, scheme);
  const shown = terminalThemes.filter(
    (t) => filter === "all" || (filter === "dark" ? t.dark : !t.dark),
  );

  return (
    <SectionCard
      title={tr("Colour scheme")}
      action={
        <ToggleButtonGroup
          exclusive
          size="small"
          value={filter}
          onChange={(_, v: Filter | null) => v && setFilter(v)}
        >
          <ToggleButton value="all">{tr("All")}</ToggleButton>
          <ToggleButton value="dark">{tr("Dark")}</ToggleButton>
          <ToggleButton value="light">{tr("Light")}</ToggleButton>
        </ToggleButtonGroup>
      }
    >
      <Box
        sx={{
          display: "grid",
          gridTemplateColumns: "repeat(auto-fill, minmax(150px, 1fr))",
          gap: 1.25,
        }}
      >
        {filter === "all" && (
          <ThemeCard
            theme={auto}
            label={tr("Auto")}
            hint={tr("Follows app theme")}
            selected={value === AUTO_THEME}
            onClick={() => onChange(AUTO_THEME)}
          />
        )}
        {shown.map((t) => (
          <ThemeCard
            key={t.id}
            theme={t}
            label={t.name}
            selected={value === t.id}
            onClick={() => onChange(t.id)}
          />
        ))}
      </Box>
      <CustomSchemes value={value} onChange={onChange} filter={filter} />
    </SectionCard>
  );
}

/** The user's own schemes (vault `color_scheme` entities): import, create, edit, delete. */
function CustomSchemes({
  value,
  onChange,
  filter,
}: {
  value: string;
  onChange: (id: string) => void;
  filter: Filter;
}) {
  const schemes = useColorSchemes(null);
  const vaults = useVaults();
  const save = useSaveColorScheme();
  const remove = useDeleteColorScheme();
  const [editing, setEditing] = useState<
    { id: Uuid; vaultId: Uuid; data: ColorSchemeData } | null | "new"
  >(null);
  const unlocked = (vaults.data ?? []).filter((v) => v.unlocked);
  const targetVault = unlocked.find((v) => v.team_id === null)?.id ?? unlocked[0]?.id;

  const importFile = async () => {
    if (!targetVault) {
      toast(tr("Unlock a vault to save colour schemes"), "warning");
      return;
    }
    const picked = await openFile({
      multiple: false,
      directory: false,
      title: tr("Import colour scheme"),
      filters: [
        {
          name: tr("Colour scheme files"),
          extensions: ["itermcolors", "json", "toml", "yml", "yaml", "conf"],
        },
        { name: tr("All files"), extensions: ["*"] },
      ],
    });
    if (typeof picked !== "string") return;
    await importFromPath(picked);
  };

  const importFromPath = async (path: string) => {
    if (!targetVault) {
      toast(tr("Unlock a vault to save colour schemes"), "warning");
      return;
    }
    try {
      const out = await ipc.themeImportFile(path);
      for (const data of out.schemes) {
        await save.mutateAsync({ vaultId: targetVault, id: null, data });
      }
      toast(tr("Imported {count} scheme(s)", { count: out.schemes.length }), "success");
      for (const w of out.warnings) toast(w, "warning");
    } catch (e) {
      toast(errorMessage(e), "error");
    }
  };

  useOpenFileRequests(isThemeFile, (path) => void importFromPath(path));

  const list = (schemes.data ?? [])
    .map((e) => ({ entity: e, theme: customTheme(e) }))
    .filter(({ theme }) => filter === "all" || (filter === "dark") === theme.dark)
    .sort((a, b) => a.theme.name.localeCompare(b.theme.name));

  return (
    <Box sx={{ mt: 2.5 }}>
      <Box sx={{ display: "flex", alignItems: "center", gap: 1, mb: 1.25 }}>
        <Typography variant="subtitle2" sx={{ flex: 1 }}>
          {tr("My schemes")}
        </Typography>
        <Button size="small" onClick={() => void importFile()} disabled={save.isPending}>
          {tr("Import…")}
        </Button>
        <Button
          size="small"
          variant="outlined"
          onClick={() => {
            if (!targetVault) toast(tr("Unlock a vault to save colour schemes"), "warning");
            else setEditing("new");
          }}
        >
          {tr("New scheme")}
        </Button>
      </Box>
      {list.length === 0 ? (
        <Typography variant="body2" color="text.secondary">
          {tr(
            "Import a scheme from iTerm2, Windows Terminal, Alacritty, Kitty, Ghostty or Gogh, or build your own. Schemes are stored in the vault and sync to your other devices.",
          )}
        </Typography>
      ) : (
        <Box
          sx={{
            display: "grid",
            gridTemplateColumns: "repeat(auto-fill, minmax(150px, 1fr))",
            gap: 1.25,
          }}
        >
          {list.map(({ entity, theme }) => (
            <Box key={theme.id} sx={{ position: "relative" }}>
              <ThemeCard
                theme={theme}
                label={theme.name}
                selected={value === theme.id}
                onClick={() => onChange(theme.id)}
              />
              <Box
                sx={{
                  position: "absolute",
                  top: 4,
                  right: 4,
                  display: "flex",
                  gap: 0.25,
                  bgcolor: "background.paper",
                  borderRadius: 1,
                  opacity: 0.9,
                }}
              >
                <Tooltip title={tr("Edit")}>
                  <IconButton
                    size="small"
                    onClick={() =>
                      setEditing({ id: entity.id, vaultId: entity.vault_id, data: entity.data })
                    }
                  >
                    <EditRoundedIcon sx={{ fontSize: 16 }} />
                  </IconButton>
                </Tooltip>
                <Tooltip title={tr("Delete")}>
                  <IconButton
                    size="small"
                    disabled={remove.isPending}
                    onClick={() =>
                      remove.mutate(entity.id, {
                        onSuccess: () => {
                          if (value === entity.id) onChange(AUTO_THEME);
                          toast(tr("Scheme deleted"), "success");
                        },
                        onError: (e) => toast(errorMessage(e), "error"),
                      })
                    }
                  >
                    <DeleteOutlineRoundedIcon sx={{ fontSize: 16 }} />
                  </IconButton>
                </Tooltip>
              </Box>
            </Box>
          ))}
        </Box>
      )}
      <ThemeEditorDialog
        key={editing === null ? "closed" : editing === "new" ? "new" : editing.id}
        open={editing !== null}
        initial={editing === "new" || editing === null ? null : editing}
        vaults={vaults.data ?? []}
        onClose={() => setEditing(null)}
      />
    </Box>
  );
}

const SAMPLE: { color: number; text: string }[][] = [
  [
    { color: 2, text: "user" },
    { color: 7, text: "@" },
    { color: 4, text: "host" },
    { color: 7, text: " ~ " },
    { color: 5, text: "$ " },
    { color: 7, text: "ls -la" },
  ],
  [
    { color: 3, text: "drwxr-x " },
    { color: 6, text: "src " },
    { color: 1, text: "main.rs " },
    { color: 7, text: "README" },
  ],
];

export function ThemeCard({
  theme,
  label,
  hint,
  selected,
  compact = false,
  onClick,
}: {
  theme: TerminalTheme;
  label: string;
  hint?: string;
  selected: boolean;
  /** Shorter preview for narrow columns (terminal side panel). */
  compact?: boolean;
  onClick: () => void;
}) {
  if (compact) {
    return (
      <ButtonBase
        onClick={onClick}
        aria-pressed={selected}
        sx={{
          display: "flex",
          alignItems: "center",
          gap: 1.25,
          textAlign: "left",
          borderRadius: 1.5,
          px: 0.75,
          py: 0.5,
          bgcolor: selected ? "surface.highest" : "transparent",
          "&:hover": { bgcolor: "surface.highest" },
        }}
      >
        <Box
          sx={{
            width: 64,
            height: 40,
            flexShrink: 0,
            borderRadius: 1,
            bgcolor: theme.background,
            display: "flex",
            flexDirection: "column",
            justifyContent: "center",
            gap: "5px",
            px: 1,
          }}
        >
          {[
            { color: theme.ansi[2], w: "100%" },
            { color: theme.ansi[4], w: "70%" },
            { color: theme.foreground, w: "45%" },
          ].map((bar, i) => (
            <Box
              key={i}
              sx={{ height: 4, width: bar.w, borderRadius: 1, bgcolor: bar.color, opacity: 0.9 }}
            />
          ))}
        </Box>
        <Box sx={{ minWidth: 0, flex: 1 }}>
          <Typography
            variant="body2"
            noWrap
            sx={{ fontWeight: 500, color: selected ? "primary.main" : "text.primary" }}
          >
            {label}
          </Typography>
          <Typography variant="caption" color="text.secondary" noWrap sx={{ display: "block" }}>
            {hint ?? (theme.dark ? tr("Dark") : tr("Light"))}
          </Typography>
        </Box>
        {selected && <CheckRoundedIcon sx={{ fontSize: 18, color: "primary.main", mr: 0.5 }} />}
      </ButtonBase>
    );
  }

  return (
    <ButtonBase
      onClick={onClick}
      aria-pressed={selected}
      sx={{
        display: "flex",
        flexDirection: "column",
        alignItems: "stretch",
        textAlign: "left",
        borderRadius: 1.5,
        overflow: "hidden",
        bgcolor: selected ? "surface.strong" : "surface.highest",
        transition: "background-color 120ms",
        "&:hover": { bgcolor: "surface.strong" },
      }}
    >
      <Box
        sx={{
          height: 64,
          px: 1.25,
          py: 1,
          bgcolor: theme.background,
          color: theme.foreground,
          fontFamily: "monospace",
          fontSize: 10.5,
          lineHeight: 1.45,
          whiteSpace: "pre",
          overflow: "hidden",
          position: "relative",
        }}
      >
        {SAMPLE.map((line, i) => (
          <Box key={i} component="div">
            {line.map((seg, j) => (
              <Box key={j} component="span" sx={{ color: theme.ansi[seg.color] }}>
                {seg.text}
              </Box>
            ))}
          </Box>
        ))}
        <Box sx={{ display: "flex", gap: 0.5, mt: 0.5 }}>
          {theme.ansi.slice(0, 8).map((c, i) => (
            <Box key={i} sx={{ width: 8, height: 8, borderRadius: "2px", bgcolor: c }} />
          ))}
        </Box>
        {selected && (
          <Box
            sx={{
              position: "absolute",
              top: 6,
              right: 6,
              width: 18,
              height: 18,
              borderRadius: "50%",
              bgcolor: "primary.main",
              color: "primary.contrastText",
              display: "grid",
              placeItems: "center",
            }}
          >
            <CheckRoundedIcon sx={{ fontSize: 13 }} />
          </Box>
        )}
      </Box>
      <Box sx={{ px: 1.25, py: 0.75, minWidth: 0 }}>
        <Typography
          variant="body2"
          noWrap
          sx={{ fontWeight: 500, color: selected ? "primary.main" : "text.primary" }}
        >
          {label}
        </Typography>
        <Typography variant="caption" color="text.secondary" noWrap sx={{ display: "block" }}>
          {hint ?? (theme.dark ? tr("Dark") : tr("Light"))}
        </Typography>
      </Box>
    </ButtonBase>
  );
}
