import { useState, type SubmitEvent } from "react";
import {
  Alert,
  Box,
  Button,
  Chip,
  Dialog,
  DialogActions,
  DialogContent,
  DialogContentText,
  DialogTitle,
  IconButton,
  List,
  ListItem,
  ListItemText,
  Paper,
  Stack,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableRow,
  TextField,
  Tooltip,
  Typography,
} from "@mui/material";
import DeleteOutlineRoundedIcon from "@mui/icons-material/DeleteOutlineRounded";
import KeyRoundedIcon from "@mui/icons-material/KeyRounded";
import { QRCodeSVG } from "qrcode.react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  startRegistration,
  type PublicKeyCredentialCreationOptionsJSON,
} from "@simplewebauthn/browser";
import { errorMessage } from "@/api/client";
import { accountApi } from "@/api/endpoints";
import { queryKeys, useServerInfo } from "@/api/hooks";
import type { WebauthnCredentialInfo } from "@/api/types";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { CopyField } from "@/components/CopyField";
import { EmptyState } from "@/components/EmptyState";
import { Loading } from "@/components/Loading";
import { PageHeader } from "@/components/PageHeader";
import { Section } from "@/components/Section";
import { useSnackbar } from "@/components/Snackbar";
import { formatDateTime, formatRelative, titleCase } from "@/components/format";
import { monoFontFamily } from "@/theme/theme";

export function SecurityPage() {
  const mfa = useQuery({ queryKey: queryKeys.mfa, queryFn: accountApi.mfa });
  const info = useServerInfo();
  if (mfa.isPending) return <Loading />;
  if (mfa.isError) return <Alert severity="error">{errorMessage(mfa.error)}</Alert>;
  return (
    <>
      <PageHeader
        title="Security"
        subtitle="Two-factor authentication and recent account activity."
      />
      <TotpSection enabled={mfa.data.totp_enabled} />
      {info.data?.features.webauthn && (
        <WebauthnSection credentials={mfa.data.webauthn_credentials} />
      )}
      {mfa.data.totp_enabled && <BackupCodesSection remaining={mfa.data.backup_codes_remaining} />}
      <SecurityEventsSection />
    </>
  );
}

function CodesDialog({ codes, onClose }: { codes: string[] | null; onClose: () => void }) {
  return (
    <Dialog open={codes !== null} onClose={onClose} maxWidth="xs" fullWidth>
      <DialogTitle>Backup codes</DialogTitle>
      <DialogContent sx={{ display: "grid", gap: 2 }}>
        <DialogContentText>
          Each code works once. Store them like a password — they bypass your authenticator.
        </DialogContentText>
        <Paper
          variant="outlined"
          sx={{
            p: 2,
            display: "grid",
            gridTemplateColumns: "1fr 1fr",
            gap: 1,
            fontFamily: monoFontFamily,
            fontSize: "0.95rem",
            bgcolor: "background.default",
          }}
        >
          {(codes ?? []).map((c) => (
            <Box key={c} sx={{ userSelect: "all" }}>
              {c}
            </Box>
          ))}
        </Paper>
        <Button
          variant="outlined"
          onClick={() => void navigator.clipboard.writeText((codes ?? []).join("\n"))}
        >
          Copy all
        </Button>
      </DialogContent>
      <DialogActions sx={{ px: 3, pb: 2 }}>
        <Button variant="contained" onClick={onClose}>
          Done
        </Button>
      </DialogActions>
    </Dialog>
  );
}

function TotpSection({ enabled }: { enabled: boolean }) {
  const qc = useQueryClient();
  const snack = useSnackbar();
  const [setupOpen, setSetupOpen] = useState(false);
  const [disableOpen, setDisableOpen] = useState(false);
  const [code, setCode] = useState("");
  const [codes, setCodes] = useState<string[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  const setup = useMutation({
    mutationFn: accountApi.totpSetup,
    onError: (e) => setError(errorMessage(e)),
  });
  const confirm = useMutation({
    mutationFn: () => accountApi.totpConfirm(code.trim()),
    onSuccess: async (r) => {
      setSetupOpen(false);
      setCode("");
      setCodes(r.codes);
      await qc.invalidateQueries({ queryKey: queryKeys.mfa });
      await qc.invalidateQueries({ queryKey: queryKeys.account });
    },
    onError: (e) => setError(errorMessage(e)),
  });
  const disable = useMutation({
    mutationFn: () => accountApi.totpDisable(code.trim()),
    onSuccess: async () => {
      setDisableOpen(false);
      setCode("");
      await qc.invalidateQueries({ queryKey: queryKeys.mfa });
      await qc.invalidateQueries({ queryKey: queryKeys.account });
      snack.notify("Authenticator app disabled");
    },
    onError: (e) => setError(errorMessage(e)),
  });

  const openSetup = () => {
    setError(null);
    setCode("");
    setSetupOpen(true);
    setup.mutate();
  };

  return (
    <Section
      title="Authenticator app"
      description="Time-based one-time codes (TOTP) from any authenticator app."
      actions={
        enabled ? (
          <Button
            variant="outlined"
            color="error"
            onClick={() => {
              setError(null);
              setCode("");
              setDisableOpen(true);
            }}
          >
            Disable
          </Button>
        ) : (
          <Button variant="contained" onClick={openSetup}>
            Set up
          </Button>
        )
      }
    >
      <Chip
        size="small"
        color={enabled ? "success" : "default"}
        variant={enabled ? "filled" : "outlined"}
        label={enabled ? "Enabled" : "Not enabled"}
      />

      <Dialog
        open={setupOpen}
        onClose={confirm.isPending ? undefined : () => setSetupOpen(false)}
        maxWidth="xs"
        fullWidth
      >
        <form
          onSubmit={(e: SubmitEvent) => {
            e.preventDefault();
            confirm.mutate();
          }}
        >
          <DialogTitle>Set up authenticator</DialogTitle>
          <DialogContent sx={{ display: "grid", gap: 2 }}>
            {error && <Alert severity="error">{error}</Alert>}
            {setup.isPending && <Loading minHeight={200} />}
            {setup.data && (
              <>
                <DialogContentText>
                  Scan the QR code or enter the secret manually, then type the current code.
                </DialogContentText>
                <Box
                  sx={{
                    display: "grid",
                    placeItems: "center",
                    p: 2,
                    bgcolor: "#fff",
                    borderRadius: 2,
                  }}
                >
                  <QRCodeSVG value={setup.data.otpauth_url} size={180} />
                </Box>
                <CopyField label="Secret" value={setup.data.secret} />
                <TextField
                  autoFocus
                  label="6-digit code"
                  required
                  inputMode="numeric"
                  autoComplete="one-time-code"
                  value={code}
                  onChange={(e) => setCode(e.target.value)}
                  disabled={confirm.isPending}
                />
              </>
            )}
          </DialogContent>
          <DialogActions sx={{ px: 3, pb: 2 }}>
            <Button
              onClick={() => setSetupOpen(false)}
              color="inherit"
              disabled={confirm.isPending}
            >
              Cancel
            </Button>
            <Button
              type="submit"
              variant="contained"
              disabled={!setup.data || confirm.isPending || code.trim() === ""}
            >
              Enable
            </Button>
          </DialogActions>
        </form>
      </Dialog>

      <Dialog
        open={disableOpen}
        onClose={disable.isPending ? undefined : () => setDisableOpen(false)}
        maxWidth="xs"
        fullWidth
      >
        <form
          onSubmit={(e: SubmitEvent) => {
            e.preventDefault();
            disable.mutate();
          }}
        >
          <DialogTitle>Disable authenticator app</DialogTitle>
          <DialogContent sx={{ display: "grid", gap: 2 }}>
            {error && <Alert severity="error">{error}</Alert>}
            <DialogContentText>
              Enter a current code (or a backup code) to confirm.
            </DialogContentText>
            <TextField
              autoFocus
              label="Code"
              required
              autoComplete="one-time-code"
              value={code}
              onChange={(e) => setCode(e.target.value)}
              disabled={disable.isPending}
            />
          </DialogContent>
          <DialogActions sx={{ px: 3, pb: 2 }}>
            <Button
              onClick={() => setDisableOpen(false)}
              color="inherit"
              disabled={disable.isPending}
            >
              Cancel
            </Button>
            <Button
              type="submit"
              variant="contained"
              color="error"
              disabled={disable.isPending || code.trim() === ""}
            >
              Disable
            </Button>
          </DialogActions>
        </form>
      </Dialog>

      <CodesDialog codes={codes} onClose={() => setCodes(null)} />
    </Section>
  );
}

function BackupCodesSection({ remaining }: { remaining: number }) {
  const qc = useQueryClient();
  const snack = useSnackbar();
  const [codes, setCodes] = useState<string[] | null>(null);
  const [confirmOpen, setConfirmOpen] = useState(false);
  const regen = useMutation({
    mutationFn: accountApi.backupCodes,
    onSuccess: async (r) => {
      setConfirmOpen(false);
      setCodes(r.codes);
      await qc.invalidateQueries({ queryKey: queryKeys.mfa });
    },
    onError: (e) => snack.error(errorMessage(e)),
  });
  return (
    <Section
      title="Backup codes"
      description="One-time codes for when you cannot reach your authenticator."
      actions={
        <Button variant="outlined" onClick={() => setConfirmOpen(true)}>
          Regenerate
        </Button>
      }
    >
      <Typography variant="body2">
        <b>{remaining}</b> unused {remaining === 1 ? "code" : "codes"} remaining
      </Typography>
      <ConfirmDialog
        open={confirmOpen}
        title="Regenerate backup codes?"
        confirmLabel="Regenerate"
        busy={regen.isPending}
        onCancel={() => setConfirmOpen(false)}
        onConfirm={() => regen.mutate()}
      >
        All existing backup codes stop working and a new set of ten is generated.
      </ConfirmDialog>
      <CodesDialog codes={codes} onClose={() => setCodes(null)} />
    </Section>
  );
}

function isCreationChallenge(
  v: unknown,
): v is { publicKey: PublicKeyCredentialCreationOptionsJSON } {
  return typeof v === "object" && v !== null && "publicKey" in v;
}

function WebauthnSection({ credentials }: { credentials: WebauthnCredentialInfo[] }) {
  const qc = useQueryClient();
  const snack = useSnackbar();
  const [addOpen, setAddOpen] = useState(false);
  const [name, setName] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [removing, setRemoving] = useState<WebauthnCredentialInfo | null>(null);

  const add = useMutation({
    mutationFn: async () => {
      const challenge = await accountApi.webauthnRegisterStart();
      if (!isCreationChallenge(challenge)) throw new Error("Malformed WebAuthn challenge");
      const credential = await startRegistration({ optionsJSON: challenge.publicKey });
      return accountApi.webauthnRegisterFinish(name.trim(), credential);
    },
    onSuccess: async () => {
      setAddOpen(false);
      setName("");
      await qc.invalidateQueries({ queryKey: queryKeys.mfa });
      await qc.invalidateQueries({ queryKey: queryKeys.account });
      snack.notify("Security key added");
    },
    onError: (e) => setError(errorMessage(e)),
  });
  const remove = useMutation({
    mutationFn: (id: string) => accountApi.webauthnDelete(id),
    onSuccess: async () => {
      setRemoving(null);
      await qc.invalidateQueries({ queryKey: queryKeys.mfa });
      await qc.invalidateQueries({ queryKey: queryKeys.account });
      snack.notify("Security key removed");
    },
    onError: (e) => snack.error(errorMessage(e)),
  });

  return (
    <Section
      title="Security keys & passkeys"
      description="Hardware keys (YubiKey etc.) or platform passkeys via WebAuthn."
      actions={
        <Button
          variant="outlined"
          startIcon={<KeyRoundedIcon />}
          onClick={() => {
            setError(null);
            setAddOpen(true);
          }}
        >
          Add key
        </Button>
      }
      disablePadding
    >
      {credentials.length === 0 ? (
        <EmptyState
          title="No security keys"
          description="Add a hardware key or passkey as a second factor."
        />
      ) : (
        <List disablePadding>
          {credentials.map((c) => (
            <ListItem
              key={c.id}
              divider
              secondaryAction={
                <Tooltip title="Remove">
                  <IconButton edge="end" onClick={() => setRemoving(c)} aria-label="Remove key">
                    <DeleteOutlineRoundedIcon />
                  </IconButton>
                </Tooltip>
              }
            >
              <ListItemText
                primary={c.name}
                secondary={`Added ${formatDateTime(c.created_at)} · Last used ${formatRelative(c.last_used_at)}`}
              />
            </ListItem>
          ))}
        </List>
      )}

      <Dialog
        open={addOpen}
        onClose={add.isPending ? undefined : () => setAddOpen(false)}
        maxWidth="xs"
        fullWidth
      >
        <form
          onSubmit={(e: SubmitEvent) => {
            e.preventDefault();
            add.mutate();
          }}
        >
          <DialogTitle>Add security key</DialogTitle>
          <DialogContent sx={{ display: "grid", gap: 2 }}>
            {error && <Alert severity="error">{error}</Alert>}
            <DialogContentText>
              Give the key a name, then follow your browser's prompt.
            </DialogContentText>
            <TextField
              autoFocus
              label="Name"
              required
              placeholder="YubiKey 5C"
              value={name}
              onChange={(e) => setName(e.target.value)}
              disabled={add.isPending}
            />
          </DialogContent>
          <DialogActions sx={{ px: 3, pb: 2 }}>
            <Button onClick={() => setAddOpen(false)} color="inherit" disabled={add.isPending}>
              Cancel
            </Button>
            <Button
              type="submit"
              variant="contained"
              disabled={add.isPending || name.trim() === ""}
            >
              Continue
            </Button>
          </DialogActions>
        </form>
      </Dialog>

      <ConfirmDialog
        open={removing !== null}
        title="Remove security key?"
        confirmLabel="Remove"
        danger
        busy={remove.isPending}
        onCancel={() => setRemoving(null)}
        onConfirm={() => {
          if (removing) remove.mutate(removing.id);
        }}
      >
        “{removing?.name}” will no longer be accepted for two-factor sign-in.
      </ConfirmDialog>
    </Section>
  );
}

function SecurityEventsSection() {
  const events = useQuery({
    queryKey: queryKeys.securityEvents,
    queryFn: accountApi.securityEvents,
  });
  return (
    <Section
      title="Recent activity"
      description="Sign-ins, device changes and security settings updates."
      disablePadding
    >
      {events.isPending ? (
        <Loading minHeight={120} />
      ) : events.isError ? (
        <Box sx={{ p: 3 }}>
          <Alert severity="error">{errorMessage(events.error)}</Alert>
        </Box>
      ) : events.data.events.length === 0 ? (
        <EmptyState title="No activity yet" />
      ) : (
        <Table size="small">
          <TableHead>
            <TableRow>
              <TableCell>Event</TableCell>
              <TableCell>IP</TableCell>
              <TableCell>Client</TableCell>
              <TableCell align="right">When</TableCell>
            </TableRow>
          </TableHead>
          <TableBody>
            {events.data.events.map((ev) => (
              <TableRow key={ev.id} hover>
                <TableCell>{titleCase(ev.kind)}</TableCell>
                <TableCell sx={{ fontFamily: monoFontFamily, fontSize: "0.8rem" }}>
                  {ev.ip ?? "—"}
                </TableCell>
                <TableCell sx={{ maxWidth: 320 }}>
                  <Typography variant="body2" noWrap title={ev.user_agent}>
                    {ev.user_agent ?? "—"}
                  </Typography>
                </TableCell>
                <TableCell align="right">
                  <Tooltip title={formatDateTime(ev.created_at)}>
                    <Stack component="span">{formatRelative(ev.created_at)}</Stack>
                  </Tooltip>
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      )}
    </Section>
  );
}
