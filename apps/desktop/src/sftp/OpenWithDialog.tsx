import { useMemo, useState } from "react";
import {
  Autocomplete,
  Button,
  Checkbox,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  FormControlLabel,
  Stack,
  TextField,
  Typography,
} from "@mui/material";
import { Field } from "@/components/ui";
import { useAppInfo, useSaveSettings, useSettings } from "@/ipc/hooks";
import type { FsEntry } from "@/ipc/types";
import { extensionOf } from "./format";
import { tr } from "@/i18n";

export { extensionOf };

/** Well-known editors per platform, offered alongside remembered ones. */
export const SUGGESTED_APPS: Record<string, string[]> = {
  linux: ["code", "gedit", "kate", "subl", "mousepad", "gimp"],
  windows: ["notepad", "code", "notepad++", "sublime_text"],
  macos: ["Visual Studio Code", "TextEdit", "Sublime Text", "BBEdit"],
};

/** Remembered applications first, then platform suggestions, without duplicates. */
export function appSuggestions(assoc: Record<string, string>, platform: string | undefined) {
  const seen = new Set<string>();
  const out: string[] = [];
  for (const a of [
    ...Object.values(assoc),
    ...(SUGGESTED_APPS[platform ?? "linux"] ?? SUGGESTED_APPS.linux ?? []),
  ]) {
    if (!seen.has(a)) {
      seen.add(a);
      out.push(a);
    }
  }
  return out;
}

interface Props {
  entry: FsEntry | null;
  onCancel: () => void;
  onConfirm: (app: string) => void;
}

/**
 * Pick the application a file opens in: a command name on PATH, a full
 * path, or (macOS) an application name. Optionally remembered per extension.
 */
export function OpenWithDialog({ entry, onCancel, onConfirm }: Props) {
  const info = useAppInfo();
  const settings = useSettings();
  const save = useSaveSettings();
  const ext = entry ? extensionOf(entry.name) : "";
  const assoc = useMemo(() => settings.data?.sftpOpenWith ?? {}, [settings.data]);
  const [app, setApp] = useState(assoc[ext] ?? "");
  const [remember, setRemember] = useState(false);

  const options = useMemo(
    () => appSuggestions(assoc, info.data?.platform),
    [assoc, info.data?.platform],
  );

  const valid = app.trim().length > 0;
  const confirm = () => {
    const chosen = app.trim();
    if (!chosen) return;
    if (remember && settings.data) {
      save.mutate({ ...settings.data, sftpOpenWith: { ...assoc, [ext]: chosen } });
    }
    onConfirm(chosen);
  };

  return (
    <Dialog open={entry !== null} onClose={onCancel} maxWidth="xs" fullWidth>
      <DialogTitle>{tr("Open with…")}</DialogTitle>
      <DialogContent>
        <Typography variant="body2" color="text.secondary" sx={{ mb: 2 }} noWrap>
          {entry?.name}
        </Typography>
        <Stack
          component="form"
          onSubmit={(e) => {
            e.preventDefault();
            confirm();
          }}
        >
          <Field label={tr("Application")}>
            <Autocomplete
              freeSolo
              autoHighlight
              options={options}
              inputValue={app}
              onInputChange={(_, v) => setApp(v)}
              renderInput={(params) => (
                <TextField
                  {...params}
                  autoFocus
                  placeholder={tr("Command, path or application name")}
                  margin="dense"
                />
              )}
            />
          </Field>
          <FormControlLabel
            sx={{ mt: 1, ml: -0.75 }}
            control={
              <Checkbox
                size="small"
                checked={remember}
                onChange={(e) => setRemember(e.target.checked)}
              />
            }
            label={
              <Typography variant="body2">
                {ext
                  ? tr("Always use for .{ext}", { ext })
                  : tr("Always use for files without extension")}
              </Typography>
            }
          />
        </Stack>
      </DialogContent>
      <DialogActions sx={{ px: 3, pb: 2 }}>
        <Button color="inherit" onClick={onCancel}>
          {tr("Cancel")}
        </Button>
        <Button variant="contained" disabled={!valid} onClick={confirm}>
          {tr("Open")}
        </Button>
      </DialogActions>
    </Dialog>
  );
}
