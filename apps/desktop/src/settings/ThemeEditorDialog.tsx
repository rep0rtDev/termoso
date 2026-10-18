import { useState } from "react";
import {
  Box,
  Button,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  MenuItem,
  TextField,
  Typography,
} from "@mui/material";
import { toast } from "@/components/Snackbar";
import { useSaveColorScheme } from "@/ipc/hooks";
import type { ColorSchemeData, LocalVault, Uuid } from "@/ipc/types";
import { customTheme } from "@/terminal/themes";
import { tr } from "@/i18n";
import { errorMessage } from "@/ipc/types";
import { ThemeCard } from "./ThemeGallery";

const ANSI_LABELS = ["Black", "Red", "Green", "Yellow", "Blue", "Magenta", "Cyan", "White"];

export const BLANK_SCHEME: ColorSchemeData = {
  name: "",
  background: "#1e1e2e",
  foreground: "#cdd6f4",
  cursor: "#f5e0dc",
  selection: null,
  ansi: [
    "#45475a",
    "#f38ba8",
    "#a6e3a1",
    "#f9e2af",
    "#89b4fa",
    "#f5c2e7",
    "#94e2d5",
    "#bac2de",
    "#585b70",
    "#f38ba8",
    "#a6e3a1",
    "#f9e2af",
    "#89b4fa",
    "#f5c2e7",
    "#94e2d5",
    "#a6adc8",
  ],
};

const rgb = (hex: string) =>
  /^#[0-9a-f]{6}/i.test(hex) ? hex.slice(0, 7).toLowerCase() : "#000000";

function ColorField({
  label,
  value,
  onChange,
}: {
  label: string;
  value: string;
  onChange: (v: string) => void;
}) {
  return (
    <Box sx={{ display: "flex", alignItems: "center", gap: 1 }}>
      <Box
        component="input"
        type="color"
        value={rgb(value)}
        onChange={(e: React.ChangeEvent<HTMLInputElement>) => onChange(e.target.value)}
        aria-label={label}
        sx={{
          width: 28,
          height: 28,
          p: 0,
          border: "1px solid",
          borderColor: "divider",
          borderRadius: 1,
          bgcolor: "transparent",
          cursor: "pointer",
        }}
      />
      <TextField
        size="small"
        label={label}
        value={value}
        onChange={(e) => onChange(e.target.value.trim())}
        error={!/^#[0-9a-f]{6}([0-9a-f]{2})?$/i.test(value)}
        slotProps={{ htmlInput: { spellCheck: false, style: { fontFamily: "monospace" } } }}
        sx={{ flex: 1 }}
      />
    </Box>
  );
}

/**
 * Create or edit a user colour scheme; saved to the vault as a `color_scheme`
 * entity. Mount with a `key` per scheme: the form state is seeded from `initial`.
 */
export function ThemeEditorDialog({
  open,
  initial,
  vaults,
  onClose,
}: {
  open: boolean;
  /** `null` creates a new scheme. */
  initial: { id: Uuid; vaultId: Uuid; data: ColorSchemeData } | null;
  vaults: LocalVault[];
  onClose: () => void;
}) {
  const unlocked = vaults.filter((v) => v.unlocked);
  const defaultVault = unlocked.find((v) => v.team_id === null)?.id ?? unlocked[0]?.id ?? "";
  const [data, setData] = useState<ColorSchemeData>(initial?.data ?? BLANK_SCHEME);
  const [vaultId, setVaultId] = useState<Uuid>(initial?.vaultId ?? defaultVault);
  const save = useSaveColorScheme();

  const set = (patch: Partial<ColorSchemeData>) => setData((d) => ({ ...d, ...patch }));
  const setAnsi = (i: number, v: string) =>
    setData((d) => ({ ...d, ansi: d.ansi.map((c, j) => (j === i ? v : c)) }));
  const valid =
    data.name.trim().length > 0 &&
    [data.background, data.foreground, data.cursor, ...data.ansi].every((c) =>
      /^#[0-9a-f]{6}$/i.test(c),
    ) &&
    (!data.selection || /^#[0-9a-f]{6}([0-9a-f]{2})?$/i.test(data.selection)) &&
    vaultId !== "";

  const submit = () => {
    const payload: ColorSchemeData = {
      ...data,
      name: data.name.trim(),
      selection: data.selection === "" ? null : (data.selection ?? null),
    };
    save.mutate(
      { vaultId, id: initial?.id ?? null, data: payload },
      {
        onSuccess: () => {
          toast(tr("Scheme saved"), "success");
          onClose();
        },
        onError: (e) => toast(errorMessage(e), "error"),
      },
    );
  };

  const preview = customTheme({
    id: initial?.id ?? "preview",
    vault_id: vaultId,
    version: 0,
    updated_at: "",
    dirty: false,
    data: {
      ...data,
      background: rgb(data.background),
      foreground: rgb(data.foreground),
      cursor: rgb(data.cursor),
      ansi: data.ansi.map(rgb),
    },
  });

  return (
    <Dialog open={open} onClose={onClose} maxWidth="md" fullWidth>
      <DialogTitle>{initial ? tr("Edit scheme") : tr("New scheme")}</DialogTitle>
      <DialogContent>
        <Box
          sx={{
            display: "grid",
            gridTemplateColumns: { xs: "1fr", md: "minmax(0, 1fr) 260px" },
            gap: 3,
            pt: 1,
          }}
        >
          <Box sx={{ display: "flex", flexDirection: "column", gap: 2 }}>
            <Box sx={{ display: "flex", gap: 1.5 }}>
              <TextField
                label={tr("Scheme name")}
                value={data.name}
                onChange={(e) => set({ name: e.target.value })}
                autoFocus
                sx={{ flex: 1 }}
                size="small"
              />
              {!initial && unlocked.length > 1 && (
                <TextField
                  select
                  size="small"
                  label={tr("Vault")}
                  value={vaultId}
                  onChange={(e) => setVaultId(e.target.value)}
                  sx={{ minWidth: 180 }}
                >
                  {unlocked.map((v) => (
                    <MenuItem key={v.id} value={v.id}>
                      {v.name}
                    </MenuItem>
                  ))}
                </TextField>
              )}
            </Box>
            <Box
              sx={{
                display: "grid",
                gridTemplateColumns: "repeat(auto-fill, minmax(200px, 1fr))",
                gap: 1.5,
              }}
            >
              <ColorField
                label={tr("Background")}
                value={data.background}
                onChange={(v) => set({ background: v })}
              />
              <ColorField
                label={tr("Text")}
                value={data.foreground}
                onChange={(v) => set({ foreground: v })}
              />
              <ColorField
                label={tr("Cursor")}
                value={data.cursor}
                onChange={(v) => set({ cursor: v })}
              />
              <ColorField
                label={tr("Selection")}
                value={data.selection ?? `${rgb(data.foreground)}40`}
                onChange={(v) => set({ selection: v })}
              />
            </Box>
            {[0, 8].map((offset) => (
              <Box key={offset}>
                <Typography variant="overline" color="text.secondary">
                  {offset === 0 ? tr("Normal") : tr("Bright")}
                </Typography>
                <Box
                  sx={{
                    display: "grid",
                    gridTemplateColumns: "repeat(auto-fill, minmax(200px, 1fr))",
                    gap: 1.5,
                  }}
                >
                  {ANSI_LABELS.map((label, i) => (
                    <ColorField
                      key={label}
                      label={tr(label)}
                      value={data.ansi[offset + i] ?? "#000000"}
                      onChange={(v) => setAnsi(offset + i, v)}
                    />
                  ))}
                </Box>
              </Box>
            ))}
          </Box>
          <Box>
            <Typography variant="overline" color="text.secondary">
              {tr("Preview")}
            </Typography>
            <ThemeCard
              theme={preview}
              label={data.name.trim() || tr("New scheme")}
              selected={false}
              onClick={() => undefined}
            />
          </Box>
        </Box>
      </DialogContent>
      <DialogActions>
        <Button onClick={onClose}>{tr("Cancel")}</Button>
        <Button variant="contained" disabled={!valid || save.isPending} onClick={submit}>
          {tr("Save")}
        </Button>
      </DialogActions>
    </Dialog>
  );
}
