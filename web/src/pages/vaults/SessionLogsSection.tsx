import { useEffect, useMemo, useState } from "react";
import {
  Alert,
  Box,
  Button,
  Chip,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  FormControl,
  IconButton,
  InputLabel,
  MenuItem,
  Select,
  Stack,
  Switch,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableRow,
  TextField,
  Tooltip,
  Typography,
} from "@mui/material";
import CommentOutlinedIcon from "@mui/icons-material/CommentOutlined";
import DeleteOutlineRoundedIcon from "@mui/icons-material/DeleteOutlineRounded";
import LockOpenRoundedIcon from "@mui/icons-material/LockOpenRounded";
import PushPinOutlinedIcon from "@mui/icons-material/PushPinOutlined";
import PushPinRoundedIcon from "@mui/icons-material/PushPinRounded";
import { useInfiniteQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { errorMessage } from "@/api/client";
import { logsApi, vaultsApi } from "@/api/endpoints";
import { queryKeys } from "@/api/hooks";
import type { LogMeta, SessionLog, Vault } from "@/api/types";
import { useAuthState } from "@/auth/store";
import { UnlockCancelled } from "@/auth/unlock";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { EmptyState } from "@/components/EmptyState";
import { Loading } from "@/components/Loading";
import { Section } from "@/components/Section";
import { useSnackbar } from "@/components/Snackbar";
import { UserCell } from "@/components/UserCell";
import { formatBytes, formatDateTime } from "@/components/format";
import { decryptLabeled, loadCrypto } from "@/crypto";
import { openMyVaultKey } from "@/vaults/keys";
import { authorsOf, durationOf, sortLogs } from "./logs";

const MAX_NOTE_CHARS = 2000;

/**
 * Team recordings shared through a vault: the policy switch (managers), the
 * list with authors, pins and comments (everyone), and the details that only
 * appear after the vault key is opened in this browser. The server holds
 * ciphertext only, so what a session was — host, command line, output — is
 * decrypted here or not shown at all.
 */
export function SessionLogsSection({ vault, manager }: { vault: Vault; manager: boolean }) {
  const qc = useQueryClient();
  const snack = useSnackbar();
  const { session, privateKey } = useAuthState();
  const me = session?.user.id;
  const canAnnotate = vault.my_role !== "viewer";

  const policy = useMutation({
    mutationFn: (on: boolean) => vaultsApi.update(vault.id, { session_logging: on }),
    onSuccess: async (_v, on) => {
      await qc.invalidateQueries({ queryKey: queryKeys.vault(vault.id) });
      await qc.invalidateQueries({ queryKey: queryKeys.vaults });
      snack.notify(
        on ? "Members' sessions in this vault are now recorded" : "Recording turned off",
      );
    },
    onError: (e) => snack.error(errorMessage(e)),
  });

  const logs = useInfiniteQuery({
    queryKey: queryKeys.vaultLogs(vault.id),
    queryFn: ({ pageParam }) => vaultsApi.logs(vault.id, pageParam),
    initialPageParam: 0,
    getNextPageParam: (last) => (last.has_more ? last.since : undefined),
  });
  const all = useMemo(
    () => (logs.data?.pages ?? []).flatMap((p) => p.logs).filter((l) => !l.deleted),
    [logs.data],
  );

  // Vault key: opened silently when the account is already unlocked in this tab,
  // otherwise on request (prompts for the password).
  const [vaultKey, setVaultKey] = useState<string | null>(null);
  const [keyError, setKeyError] = useState<string | null>(null);
  const holdKey = Boolean(vault.sealed_key);
  useEffect(() => {
    if (!holdKey || !privateKey || vaultKey) return;
    let cancelled = false;
    openMyVaultKey(vault)
      .then((k) => {
        if (!cancelled) setVaultKey(k);
      })
      .catch((e: unknown) => {
        if (!cancelled) setKeyError(errorMessage(e));
      });
    return () => {
      cancelled = true;
    };
  }, [holdKey, privateKey, vault, vaultKey]);
  const unlock = useMutation({
    mutationFn: () => openMyVaultKey(vault),
    onSuccess: (k) => {
      setKeyError(null);
      setVaultKey(k);
    },
    onError: (e) => {
      if (!(e instanceof UnlockCancelled)) snack.error(errorMessage(e));
    },
  });

  const [metas, setMetas] = useState<Record<string, LogMeta | null>>({});
  useEffect(() => {
    if (!vaultKey) return;
    const pending = all.filter((l) => !(l.id in metas));
    if (pending.length === 0) return;
    let cancelled = false;
    void loadCrypto().then(() => {
      if (cancelled) return;
      const next: Record<string, LogMeta | null> = {};
      for (const l of pending) next[l.id] = decryptMeta(vaultKey, l);
      setMetas((m) => ({ ...m, ...next }));
    });
    return () => {
      cancelled = true;
    };
  }, [all, metas, vaultKey]);

  const [authorId, setAuthorId] = useState<string>("");
  const authors = useMemo(() => authorsOf(all), [all]);
  const list = useMemo(
    () => sortLogs(authorId ? all.filter((l) => l.user_id === authorId) : all),
    [all, authorId],
  );

  const invalidate = () => qc.invalidateQueries({ queryKey: queryKeys.vaultLogs(vault.id) });
  const annotate = useMutation({
    mutationFn: ({ id, patch }: { id: string; patch: { pinned?: boolean; note?: string } }) =>
      logsApi.update(id, patch),
    onSuccess: async () => {
      await invalidate();
    },
    onError: (e) => snack.error(errorMessage(e)),
  });
  const del = useMutation({
    mutationFn: (id: string) => logsApi.delete(id),
    onSuccess: async () => {
      setDeleting(null);
      await invalidate();
      snack.notify("Recording deleted for every member");
    },
    onError: (e) => snack.error(errorMessage(e)),
  });
  const [editing, setEditing] = useState<SessionLog | null>(null);
  const [deleting, setDeleting] = useState<SessionLog | null>(null);

  return (
    <Section
      title="Session logs"
      description="Recordings members make on their devices, encrypted with the vault key and shared with the vault. Editors can pin recordings and leave a comment; the server never sees terminal content."
      actions={
        holdKey && !vaultKey ? (
          <Button
            variant="outlined"
            startIcon={<LockOpenRoundedIcon />}
            disabled={unlock.isPending}
            onClick={() => unlock.mutate()}
          >
            Show details
          </Button>
        ) : undefined
      }
    >
      <Stack spacing={2}>
        <Box sx={{ display: "flex", alignItems: "center", gap: 2 }}>
          <Box sx={{ flex: 1, minWidth: 0 }}>
            <Typography variant="body2">Record members' sessions</Typography>
            <Typography variant="caption" color="text.secondary">
              {manager
                ? "Every terminal session to a host of this vault is recorded on the member's device and uploaded encrypted. Members still see their own recordings when this is off."
                : "Only vault managers change this."}
            </Typography>
          </Box>
          <Tooltip title={manager ? "" : "Only vault managers change this"}>
            <span>
              <Switch
                checked={vault.session_logging}
                disabled={!manager || policy.isPending}
                onChange={(e) => policy.mutate(e.target.checked)}
                slotProps={{ input: { "aria-label": "Record members' sessions" } }}
              />
            </span>
          </Tooltip>
        </Box>

        {keyError && <Alert severity="warning">{keyError}</Alert>}
        {!holdKey && (
          <Alert severity="info">
            Session details stay encrypted until a manager seals the vault key to you.
          </Alert>
        )}

        {logs.isPending ? (
          <Loading />
        ) : logs.isError ? (
          <Alert severity="error">{errorMessage(logs.error)}</Alert>
        ) : all.length === 0 ? (
          <EmptyState
            title="No recordings yet"
            description={
              vault.session_logging
                ? "Recordings appear here after members finish a session on a host of this vault."
                : "Turn on recording above, or members can record their own sessions from the app settings."
            }
          />
        ) : (
          <>
            {authors.length > 1 && (
              <FormControl size="small" sx={{ maxWidth: 320 }}>
                <InputLabel id="log-author">Author</InputLabel>
                <Select
                  labelId="log-author"
                  label="Author"
                  value={authorId}
                  onChange={(e) => setAuthorId(e.target.value)}
                >
                  <MenuItem value="">Everyone</MenuItem>
                  {authors.map((a) => (
                    <MenuItem key={a.user_id} value={a.user_id}>
                      {a.display_name ?? a.email}
                    </MenuItem>
                  ))}
                </Select>
              </FormControl>
            )}
            <Table size="small">
              <TableHead>
                <TableRow>
                  <TableCell>Author</TableCell>
                  <TableCell>Session</TableCell>
                  <TableCell>When</TableCell>
                  <TableCell align="right">Size</TableCell>
                  <TableCell align="right" />
                </TableRow>
              </TableHead>
              <TableBody>
                {list.map((l) => {
                  const meta = metas[l.id];
                  const canDelete = manager || l.user_id === me;
                  return (
                    <TableRow key={l.id} hover>
                      <TableCell sx={{ minWidth: 200 }}>
                        <UserCell
                          userId={l.user_id}
                          email={l.author?.email ?? "Former member"}
                          displayName={l.author?.display_name}
                          avatar={l.author?.avatar_tag}
                          you={l.user_id === me}
                        />
                      </TableCell>
                      <TableCell sx={{ minWidth: 220 }}>
                        <Stack direction="row" spacing={0.75} sx={{ alignItems: "center" }}>
                          {l.pinned && (
                            <Tooltip title="Pinned">
                              <PushPinRoundedIcon color="primary" sx={{ fontSize: 16 }} />
                            </Tooltip>
                          )}
                          <Typography variant="body2" noWrap sx={{ fontWeight: 500 }}>
                            {meta ? meta.label : "Encrypted recording"}
                          </Typography>
                          {!l.completed && (
                            <Chip size="small" variant="outlined" label="Uploading" />
                          )}
                        </Stack>
                        <Typography
                          variant="caption"
                          color="text.secondary"
                          noWrap
                          sx={{ display: "block" }}
                        >
                          {meta
                            ? `${meta.protocol} · ${meta.target} · ${durationOf(meta)}`
                            : !vaultKey
                              ? "Details are decrypted only in your browser"
                              : meta === null
                                ? "Could not decrypt (recorded with another key version?)"
                                : "Decrypting…"}
                        </Typography>
                        {l.note && (
                          <Typography
                            variant="body2"
                            sx={{ mt: 0.5, whiteSpace: "pre-wrap", color: "text.secondary" }}
                          >
                            {l.note}
                          </Typography>
                        )}
                      </TableCell>
                      <TableCell sx={{ whiteSpace: "nowrap" }}>
                        {formatDateTime(meta?.started_at ?? l.created_at)}
                      </TableCell>
                      <TableCell align="right" sx={{ whiteSpace: "nowrap" }}>
                        {formatBytes(l.size_bytes)}
                      </TableCell>
                      <TableCell align="right" sx={{ whiteSpace: "nowrap" }}>
                        {canAnnotate && (
                          <>
                            <Tooltip title={l.pinned ? "Unpin" : "Pin"}>
                              <IconButton
                                size="small"
                                disabled={annotate.isPending}
                                onClick={() =>
                                  annotate.mutate({ id: l.id, patch: { pinned: !l.pinned } })
                                }
                              >
                                {l.pinned ? (
                                  <PushPinRoundedIcon fontSize="small" />
                                ) : (
                                  <PushPinOutlinedIcon fontSize="small" />
                                )}
                              </IconButton>
                            </Tooltip>
                            <Tooltip title={l.note ? "Edit comment" : "Comment"}>
                              <IconButton size="small" onClick={() => setEditing(l)}>
                                <CommentOutlinedIcon fontSize="small" />
                              </IconButton>
                            </Tooltip>
                          </>
                        )}
                        {canDelete && (
                          <Tooltip title="Delete for everyone">
                            <IconButton size="small" onClick={() => setDeleting(l)}>
                              <DeleteOutlineRoundedIcon fontSize="small" />
                            </IconButton>
                          </Tooltip>
                        )}
                      </TableCell>
                    </TableRow>
                  );
                })}
              </TableBody>
            </Table>
            {logs.hasNextPage && (
              <Box>
                <Button
                  size="small"
                  color="inherit"
                  disabled={logs.isFetchingNextPage}
                  onClick={() => void logs.fetchNextPage()}
                >
                  Load more
                </Button>
              </Box>
            )}
          </>
        )}
      </Stack>

      <NoteDialog
        log={editing}
        busy={annotate.isPending}
        onClose={() => setEditing(null)}
        onSave={(note) => {
          if (!editing) return;
          annotate.mutate(
            { id: editing.id, patch: { note } },
            { onSuccess: () => setEditing(null) },
          );
        }}
      />
      <ConfirmDialog
        open={deleting !== null}
        title="Delete recording?"
        confirmLabel="Delete"
        danger
        busy={del.isPending}
        onCancel={() => setDeleting(null)}
        onConfirm={() => {
          if (deleting) del.mutate(deleting.id);
        }}
      >
        The recording is removed from the server for every member of “{vault.name}”; copies already
        downloaded to members' devices are deleted on their next sync. This cannot be undone.
      </ConfirmDialog>
    </Section>
  );
}

function decryptMeta(vaultKey: string, log: SessionLog): LogMeta | null {
  try {
    const json: unknown = JSON.parse(decryptLabeled(vaultKey, ["log", log.id], log.meta));
    return isLogMeta(json) ? json : null;
  } catch {
    return null;
  }
}

function isLogMeta(v: unknown): v is LogMeta {
  if (typeof v !== "object" || v === null) return false;
  const o = v as Record<string, unknown>;
  return (
    typeof o.label === "string" &&
    typeof o.target === "string" &&
    typeof o.protocol === "string" &&
    typeof o.started_at === "string"
  );
}

function NoteDialog({
  log,
  busy,
  onClose,
  onSave,
}: {
  log: SessionLog | null;
  busy: boolean;
  onClose: () => void;
  onSave: (note: string) => void;
}) {
  return (
    <Dialog open={log !== null} onClose={busy ? undefined : onClose} maxWidth="sm" fullWidth>
      {log && <NoteForm key={log.id} log={log} busy={busy} onClose={onClose} onSave={onSave} />}
    </Dialog>
  );
}

function NoteForm({
  log,
  busy,
  onClose,
  onSave,
}: {
  log: SessionLog;
  busy: boolean;
  onClose: () => void;
  onSave: (note: string) => void;
}) {
  const [note, setNote] = useState(log.note);
  const tooLong = note.length > MAX_NOTE_CHARS;
  return (
    <>
      <DialogTitle>Comment</DialogTitle>
      <DialogContent>
        <TextField
          autoFocus
          fullWidth
          multiline
          minRows={3}
          maxRows={10}
          placeholder="What happened in this session, what to look at…"
          value={note}
          onChange={(e) => setNote(e.target.value)}
          error={tooLong}
          helperText={`${note.length} / ${MAX_NOTE_CHARS} · visible to every member of the vault`}
          sx={{ mt: 1 }}
        />
      </DialogContent>
      <DialogActions sx={{ px: 3, pb: 2 }}>
        {log.note && (
          <Button color="error" disabled={busy} onClick={() => onSave("")} sx={{ mr: "auto" }}>
            Remove
          </Button>
        )}
        <Button onClick={onClose} color="inherit">
          Cancel
        </Button>
        <Button
          variant="contained"
          disabled={busy || tooLong || note.trim() === log.note}
          onClick={() => onSave(note.trim())}
        >
          Save
        </Button>
      </DialogActions>
    </>
  );
}
