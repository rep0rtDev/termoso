import { useEffect, useState } from "react";
import {
  Alert,
  Box,
  Button,
  Chip,
  Divider,
  Drawer,
  FormControlLabel,
  IconButton,
  InputAdornment,
  Stack,
  Switch,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TablePagination,
  TableRow,
  TextField,
  Tooltip,
  Typography,
} from "@mui/material";
import CloseRoundedIcon from "@mui/icons-material/CloseRounded";
import SearchRoundedIcon from "@mui/icons-material/SearchRounded";
import { keepPreviousData, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { errorMessage } from "@/api/client";
import { adminApi } from "@/api/endpoints";
import { queryKeys } from "@/api/hooks";
import type { AdminUpdateUserRequest, AdminUser } from "@/api/types";
import { useAuthState } from "@/auth/store";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { EmptyState } from "@/components/EmptyState";
import { Loading } from "@/components/Loading";
import { PageHeader } from "@/components/PageHeader";
import { Section } from "@/components/Section";
import { useSnackbar } from "@/components/Snackbar";
import { UserCell } from "@/components/UserCell";
import { formatDateTime, formatRelative } from "@/components/format";

function useDebounced(value: string, ms = 300): string {
  const [v, setV] = useState(value);
  useEffect(() => {
    const t = window.setTimeout(() => setV(value), ms);
    return () => window.clearTimeout(t);
  }, [value, ms]);
  return v;
}

export function AdminUsersPage() {
  const [search, setSearch] = useState("");
  const q = useDebounced(search.trim());
  const [page, setPage] = useState(0);
  const [limit, setLimit] = useState(25);
  const [selected, setSelected] = useState<string | null>(null);
  const offset = page * limit;
  const users = useQuery({
    queryKey: queryKeys.adminUsers(q, offset, limit),
    queryFn: () => adminApi.users({ q, offset, limit }),
    placeholderData: keepPreviousData,
  });

  return (
    <>
      <PageHeader
        title="Users"
        subtitle="Everyone registered on this server. The server never sees passwords or vault contents, so there is nothing to reset except MFA and sessions."
        actions={
          <TextField
            size="small"
            placeholder="Search by email or name"
            value={search}
            onChange={(e) => {
              setSearch(e.target.value);
              setPage(0);
            }}
            slotProps={{
              input: {
                startAdornment: (
                  <InputAdornment position="start">
                    <SearchRoundedIcon fontSize="small" />
                  </InputAdornment>
                ),
              },
            }}
            sx={{ minWidth: 280 }}
          />
        }
      />
      <Section
        title={users.data ? `${users.data.total.toLocaleString()} users` : "Users"}
        disablePadding
      >
        {users.isPending ? (
          <Loading />
        ) : users.isError ? (
          <Box sx={{ p: 3 }}>
            <Alert severity="error">{errorMessage(users.error)}</Alert>
          </Box>
        ) : users.data.items.length === 0 ? (
          <EmptyState title="No users match" />
        ) : (
          <>
            <Table size="small">
              <TableHead>
                <TableRow>
                  <TableCell>User</TableCell>
                  <TableCell>Status</TableCell>
                  <TableCell align="right">Devices</TableCell>
                  <TableCell>Last seen</TableCell>
                  <TableCell>Joined</TableCell>
                </TableRow>
              </TableHead>
              <TableBody>
                {users.data.items.map((u) => (
                  <TableRow
                    key={u.id}
                    hover
                    sx={{ cursor: "pointer" }}
                    onClick={() => setSelected(u.id)}
                    selected={selected === u.id}
                  >
                    <TableCell>
                      <UserCell email={u.email} displayName={u.display_name} />
                    </TableCell>
                    <TableCell>
                      <Stack direction="row" spacing={0.5} useFlexGap sx={{ flexWrap: "wrap" }}>
                        {u.is_admin && <Chip size="small" color="primary" label="Admin" />}
                        {u.disabled && <Chip size="small" color="error" label="Disabled" />}
                        {!u.email_verified && (
                          <Chip
                            size="small"
                            variant="outlined"
                            color="warning"
                            label="Unverified"
                          />
                        )}
                        {u.mfa_enabled && <Chip size="small" variant="outlined" label="MFA" />}
                      </Stack>
                    </TableCell>
                    <TableCell align="right">{u.devices}</TableCell>
                    <TableCell>{formatRelative(u.last_seen_at)}</TableCell>
                    <TableCell>{formatDateTime(u.created_at)}</TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
            <TablePagination
              component="div"
              count={users.data.total}
              page={page}
              onPageChange={(_e, p) => setPage(p)}
              rowsPerPage={limit}
              onRowsPerPageChange={(e) => {
                setLimit(Number(e.target.value));
                setPage(0);
              }}
              rowsPerPageOptions={[25, 50, 100]}
            />
          </>
        )}
      </Section>
      <UserDrawer id={selected} onClose={() => setSelected(null)} />
    </>
  );
}

function UserDrawer({ id, onClose }: { id: string | null; onClose: () => void }) {
  const qc = useQueryClient();
  const snack = useSnackbar();
  const { session } = useAuthState();
  const isMe = id !== null && id === session?.user.id;
  const user = useQuery({
    queryKey: ["admin", "user", id],
    queryFn: () => adminApi.user(id ?? ""),
    enabled: id !== null,
  });
  const [confirm, setConfirm] = useState<"revoke" | "reset-mfa" | "delete" | null>(null);

  const refresh = async () => {
    await qc.invalidateQueries({ queryKey: ["admin", "user", id] });
    await qc.invalidateQueries({ queryKey: ["admin", "users"] });
    await qc.invalidateQueries({ queryKey: queryKeys.adminStats });
  };
  const update = useMutation({
    mutationFn: (req: AdminUpdateUserRequest) => adminApi.updateUser(id ?? "", req),
    onSuccess: refresh,
    onError: (e) => snack.error(errorMessage(e)),
  });
  const action = useMutation({
    mutationFn: async (kind: "revoke" | "reset-mfa" | "delete") => {
      const uid = id ?? "";
      if (kind === "revoke") await adminApi.revokeSessions(uid);
      else if (kind === "reset-mfa") await adminApi.resetMfa(uid);
      else await adminApi.deleteUser(uid);
      return kind;
    },
    onSuccess: async (kind) => {
      setConfirm(null);
      if (kind === "delete") {
        onClose();
        snack.notify("User deleted");
      } else {
        snack.notify(
          kind === "revoke"
            ? "All sessions revoked"
            : "MFA reset; the user can sign in with password only",
        );
      }
      await refresh();
    },
    onError: (e) => snack.error(errorMessage(e)),
  });

  const u: AdminUser | undefined = user.data;
  return (
    <Drawer
      anchor="right"
      open={id !== null}
      onClose={onClose}
      slotProps={{ paper: { sx: { width: { xs: "100%", sm: 420 }, p: 3 } } }}
    >
      <Stack direction="row" sx={{ alignItems: "center", justifyContent: "space-between", mb: 2 }}>
        <Typography variant="h6">User</Typography>
        <IconButton onClick={onClose} aria-label="Close">
          <CloseRoundedIcon />
        </IconButton>
      </Stack>
      {user.isPending && <Loading />}
      {user.isError && <Alert severity="error">{errorMessage(user.error)}</Alert>}
      {u && (
        <Stack spacing={2.5}>
          <UserCell email={u.email} displayName={u.display_name} you={isMe} />
          <Stack spacing={0.5}>
            <Row label="Joined" value={formatDateTime(u.created_at)} />
            <Row label="Last seen" value={formatRelative(u.last_seen_at)} />
            <Row label="Devices" value={String(u.devices)} />
            <Row label="MFA" value={u.mfa_enabled ? "Enabled" : "Off"} />
          </Stack>
          <Divider />
          <Stack>
            <FormControlLabel
              control={
                <Switch
                  checked={u.is_admin}
                  disabled={isMe || update.isPending}
                  onChange={(e) => update.mutate({ is_admin: e.target.checked })}
                />
              }
              label="Server administrator"
            />
            <FormControlLabel
              control={
                <Switch
                  checked={u.email_verified}
                  disabled={update.isPending}
                  onChange={(e) => update.mutate({ email_verified: e.target.checked })}
                />
              }
              label="Email verified"
            />
            <FormControlLabel
              control={
                <Switch
                  color="error"
                  checked={u.disabled}
                  disabled={isMe || update.isPending}
                  onChange={(e) => update.mutate({ disabled: e.target.checked })}
                />
              }
              label="Disabled (cannot sign in; sessions revoked)"
            />
          </Stack>
          <Divider />
          <Stack spacing={1}>
            <Button
              variant="outlined"
              onClick={() => setConfirm("revoke")}
              disabled={u.devices === 0}
            >
              Sign out all devices
            </Button>
            <Tooltip title={u.mfa_enabled ? "" : "The user has no MFA configured"}>
              <span>
                <Button
                  variant="outlined"
                  fullWidth
                  onClick={() => setConfirm("reset-mfa")}
                  disabled={!u.mfa_enabled}
                >
                  Reset MFA
                </Button>
              </span>
            </Tooltip>
            <Button
              variant="outlined"
              color="error"
              onClick={() => setConfirm("delete")}
              disabled={isMe}
            >
              Delete user
            </Button>
          </Stack>
        </Stack>
      )}
      <ConfirmDialog
        open={confirm === "revoke"}
        title="Sign out all devices?"
        confirmLabel="Sign out"
        busy={action.isPending}
        onCancel={() => setConfirm(null)}
        onConfirm={() => action.mutate("revoke")}
      >
        Every session of {u?.email} is revoked. They must sign in again on each device.
      </ConfirmDialog>
      <ConfirmDialog
        open={confirm === "reset-mfa"}
        title="Reset two-factor authentication?"
        confirmLabel="Reset MFA"
        danger
        busy={action.isPending}
        onCancel={() => setConfirm(null)}
        onConfirm={() => action.mutate("reset-mfa")}
      >
        TOTP, security keys and backup codes of {u?.email} are removed and all sessions revoked.
        Verify the request out of band first.
      </ConfirmDialog>
      <ConfirmDialog
        open={confirm === "delete"}
        title="Delete user?"
        confirmLabel="Delete"
        danger
        busy={action.isPending}
        onCancel={() => setConfirm(null)}
        onConfirm={() => action.mutate("delete")}
      >
        {u?.email} and all their encrypted data, devices and session logs are removed permanently.
        Users who own teams cannot be deleted.
      </ConfirmDialog>
    </Drawer>
  );
}

function Row({ label, value }: { label: string; value: string }) {
  return (
    <Stack direction="row" sx={{ justifyContent: "space-between" }}>
      <Typography variant="body2" color="text.secondary">
        {label}
      </Typography>
      <Typography variant="body2">{value}</Typography>
    </Stack>
  );
}
