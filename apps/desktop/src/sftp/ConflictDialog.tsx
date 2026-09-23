import { useState } from "react";
import {
  Box,
  Button,
  Checkbox,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  FormControlLabel,
  Stack,
  Typography,
} from "@mui/material";
import FolderRoundedIcon from "@mui/icons-material/FolderRounded";
import InsertDriveFileOutlinedIcon from "@mui/icons-material/InsertDriveFileOutlined";
import type { Conflict, Direction, FsEntry } from "@/ipc/types";
import { formatMtime, formatSize } from "./format";
import { tr, trn } from "@/i18n";

export interface ConflictDecision {
  conflict: Conflict;
  /** Use the same answer for every remaining conflict of this batch. */
  all: boolean;
}

export interface ConflictPrompt {
  direction: Direction;
  incoming: FsEntry;
  existing: FsEntry;
  /** Destination directory the item is going into. */
  dest: string;
  /** Whether the remote can append to a partial upload (SFTP yes, WebDAV no). */
  resumeUpload: boolean;
  /** Conflicts still queued after this one. */
  remaining: number;
  resolve: (decision: ConflictDecision | null) => void;
}

/**
 * "X already exists" — Replace / Skip / Rename, optionally for the whole
 * batch. For folders the answer applies to every file inside that clashes;
 * new files are always copied.
 */
export function ConflictDialog({ prompt }: { prompt: ConflictPrompt | null }) {
  const [all, setAll] = useState(false);
  if (!prompt) return null;
  const { incoming, existing, remaining } = prompt;
  const folder = incoming.kind === "dir";
  const canResume =
    !folder &&
    (prompt.direction === "download" || prompt.resumeUpload) &&
    incoming.size !== null &&
    existing.size !== null &&
    existing.size > 0 &&
    existing.size < incoming.size;
  const answer = (conflict: Conflict) => {
    prompt.resolve({ conflict, all });
    setAll(false);
  };
  const cancel = () => {
    prompt.resolve(null);
    setAll(false);
  };

  return (
    <Dialog open onClose={cancel} maxWidth="sm" fullWidth>
      <DialogTitle>
        {prompt.direction === "upload"
          ? folder
            ? tr("Folder already exists on the server")
            : tr("File already exists on the server")
          : folder
            ? tr("Folder already exists on this computer")
            : tr("File already exists on this computer")}
      </DialogTitle>
      <DialogContent>
        <Typography variant="body2" color="text.secondary" sx={{ mb: 2 }}>
          {folder
            ? tr(
                "Files that exist in both folders will be handled the way you choose; new files are always copied.",
              )
            : tr("Choose what to do with the existing file.")}
        </Typography>
        <Stack direction="row" spacing={1.5}>
          <Side title={tr("Existing")} entry={existing} dest={prompt.dest} />
          <Side title={tr("Incoming")} entry={incoming} dest={null} />
        </Stack>
        {remaining > 0 && (
          <FormControlLabel
            sx={{ mt: 1.5, ml: -0.75 }}
            control={
              <Checkbox size="small" checked={all} onChange={(e) => setAll(e.target.checked)} />
            }
            label={
              <Typography variant="body2">
                {trn(
                  remaining,
                  "Apply to all ({count} more conflict)",
                  "Apply to all ({count} more conflicts)",
                )}
              </Typography>
            }
          />
        )}
      </DialogContent>
      <DialogActions sx={{ px: 3, pb: 2, flexWrap: "wrap", gap: 0.5 }}>
        <Button color="inherit" onClick={cancel} sx={{ mr: "auto" }}>
          {tr("Cancel")}
        </Button>
        {canResume && (
          <Button color="inherit" onClick={() => answer("resume")}>
            {tr("Resume")}
          </Button>
        )}
        <Button color="inherit" onClick={() => answer("skip")}>
          {tr("Skip")}
        </Button>
        <Button color="inherit" onClick={() => answer("rename")}>
          {folder ? tr("Keep both") : tr("Rename")}
        </Button>
        <Button variant="contained" onClick={() => answer("replace")} autoFocus>
          {tr("Replace")}
        </Button>
      </DialogActions>
    </Dialog>
  );
}

function Side({ title, entry, dest }: { title: string; entry: FsEntry; dest: string | null }) {
  const Icon = entry.kind === "dir" ? FolderRoundedIcon : InsertDriveFileOutlinedIcon;
  return (
    <Box
      sx={{
        flex: 1,
        minWidth: 0,
        p: 1.5,
        borderRadius: 2,
        bgcolor: "surface.high",
      }}
    >
      <Typography variant="caption" color="text.disabled" sx={{ display: "block", mb: 0.75 }}>
        {title}
      </Typography>
      <Stack direction="row" spacing={1} sx={{ alignItems: "center", minWidth: 0 }}>
        <Icon fontSize="small" sx={{ color: "text.secondary", flexShrink: 0 }} />
        <Typography variant="body2" noWrap sx={{ fontWeight: 500 }}>
          {entry.name}
        </Typography>
      </Stack>
      <Typography
        variant="caption"
        color="text.secondary"
        sx={{ display: "block", mt: 0.75, fontVariantNumeric: "tabular-nums" }}
      >
        {entry.kind === "dir" ? tr("Folder") : formatSize(entry.size)}
        {entry.mtime !== null ? ` · ${formatMtime(entry.mtime)}` : ""}
      </Typography>
      {dest && (
        <Typography variant="caption" color="text.disabled" noWrap sx={{ display: "block" }}>
          {dest}
        </Typography>
      )}
    </Box>
  );
}
