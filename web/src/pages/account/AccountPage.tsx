import { useRef, useState, type SubmitEvent } from "react";
import {
  Alert,
  Box,
  Button,
  Checkbox,
  Chip,
  Dialog,
  DialogActions,
  DialogContent,
  DialogContentText,
  DialogTitle,
  FormControlLabel,
  Stack,
  TextField,
  Typography,
} from "@mui/material";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "react-router";
import { errorMessage } from "@/api/client";
import { accountApi } from "@/api/endpoints";
import type { UserProfile } from "@/api/types";
import { queryKeys, useAccount, useServerInfo } from "@/api/hooks";
import { changePassword } from "@/auth/flows";
import { authStore, useAuthState } from "@/auth/store";
import { requireUnlocked, UnlockCancelled, withStepUp } from "@/auth/unlock";
import { loadCrypto, rotateRecovery } from "@/crypto";
import { Loading } from "@/components/Loading";
import { PageHeader } from "@/components/PageHeader";
import { Section } from "@/components/Section";
import { useSnackbar } from "@/components/Snackbar";
import { UserAvatar } from "@/components/UserAvatar";
import { formatDate } from "@/components/format";
import { shrinkImage } from "@/pages/account/shrinkImage";
import { PasswordField, passwordProblem } from "@/pages/auth/common";

export function AccountPage() {
  const account = useAccount();
  if (account.isPending) return <Loading />;
  if (account.isError) return <Alert severity="error">{errorMessage(account.error)}</Alert>;
  return (
    <>
      <PageHeader title="Account" subtitle="Your profile, sign-in email and password." />
      <ProfileSection
        displayName={account.data.user.display_name ?? ""}
        createdAt={account.data.user.created_at}
      />
      <PictureSection user={account.data.user} />
      <EmailSection email={account.data.user.email} verified={account.data.user.email_verified} />
      <PasswordSection />
      <RecoveryKeySection />
    </>
  );
}

function ProfileSection({ displayName, createdAt }: { displayName: string; createdAt: string }) {
  const [name, setName] = useState(displayName);
  const qc = useQueryClient();
  const snack = useSnackbar();
  const save = useMutation({
    mutationFn: () => accountApi.updateProfile(name.trim() === "" ? null : name.trim()),
    onSuccess: async (user) => {
      authStore.updateUser(user);
      await qc.invalidateQueries({ queryKey: queryKeys.account });
      snack.notify("Profile saved");
    },
    onError: (e) => snack.error(errorMessage(e)),
  });
  return (
    <Section title="Profile" description={`Member since ${formatDate(createdAt)}`}>
      <Box
        component="form"
        onSubmit={(e: SubmitEvent) => {
          e.preventDefault();
          save.mutate();
        }}
        sx={{ display: "flex", gap: 1, flexWrap: "wrap", alignItems: "flex-start" }}
      >
        <TextField
          label="Display name"
          value={name}
          onChange={(e) => setName(e.target.value)}
          fullWidth={false}
          sx={{ width: { xs: "100%", sm: 360 } }}
          helperText="Shown to teammates instead of your email."
        />
        <Button
          type="submit"
          variant="contained"
          disabled={save.isPending || name.trim() === displayName.trim()}
          sx={{ mt: "22px", height: 36 }}
        >
          Save
        </Button>
      </Box>
    </Section>
  );
}

function PictureSection({ user }: { user: UserProfile }) {
  const qc = useQueryClient();
  const snack = useSnackbar();
  const input = useRef<HTMLInputElement>(null);
  const apply = async (updated: UserProfile, message: string) => {
    authStore.updateUser(updated);
    await qc.invalidateQueries({ queryKey: queryKeys.account });
    snack.notify(message);
  };
  const put = useMutation({
    mutationFn: async (file: File) => accountApi.putAvatar(await shrinkImage(file)),
    onSuccess: (u) => apply(u, "Picture updated"),
    onError: (e) => snack.error(errorMessage(e)),
  });
  const remove = useMutation({
    mutationFn: () => accountApi.deleteAvatar(),
    onSuccess: (u) => apply(u, "Picture removed"),
    onError: (e) => snack.error(errorMessage(e)),
  });
  const busy = put.isPending || remove.isPending;
  return (
    <Section
      title="Picture"
      description="Shown next to your name to teammates. Resized in your browser and stored as a small 192×192 image."
    >
      <Stack direction="row" spacing={2} useFlexGap sx={{ alignItems: "center", flexWrap: "wrap" }}>
        <UserAvatar
          userId={user.id}
          tag={user.avatar}
          email={user.email}
          displayName={user.display_name}
          size={96}
        />
        <input
          ref={input}
          type="file"
          accept="image/*"
          hidden
          onChange={(e) => {
            const file = e.target.files?.[0];
            e.target.value = "";
            if (file) put.mutate(file);
          }}
        />
        <Button variant="contained" disabled={busy} onClick={() => input.current?.click()}>
          {user.avatar ? "Change" : "Upload"}
        </Button>
        {user.avatar && (
          <Button variant="text" color="inherit" disabled={busy} onClick={() => remove.mutate()}>
            Remove
          </Button>
        )}
      </Stack>
    </Section>
  );
}

function EmailSection({ email, verified }: { email: string; verified: boolean }) {
  const info = useServerInfo();
  const emailEnabled = info.data?.features.email ?? false;
  const qc = useQueryClient();
  const snack = useSnackbar();
  const [code, setCode] = useState("");
  const [sent, setSent] = useState(false);
  const [changeOpen, setChangeOpen] = useState(false);

  const send = useMutation({
    mutationFn: () => accountApi.emailVerifySend(),
    onSuccess: () => {
      setSent(true);
      snack.notify("Verification code sent");
    },
    onError: (e) => snack.error(errorMessage(e)),
  });
  const confirm = useMutation({
    mutationFn: () => accountApi.emailVerifyConfirm(code.trim()),
    onSuccess: async () => {
      setCode("");
      await qc.invalidateQueries({ queryKey: queryKeys.account });
      snack.notify("Email verified");
    },
    onError: (e) => snack.error(errorMessage(e)),
  });

  return (
    <Section
      id="email"
      title="Email"
      description="Used for sign-in, device approvals and team invitations."
      actions={
        <Button variant="outlined" onClick={() => setChangeOpen(true)}>
          Change email
        </Button>
      }
    >
      <Stack spacing={2}>
        <Box sx={{ display: "flex", alignItems: "center", gap: 1.5, flexWrap: "wrap" }}>
          <Typography sx={{ fontWeight: 500 }}>{email}</Typography>
          {verified ? (
            <Chip size="small" color="success" label="Verified" />
          ) : (
            <Chip size="small" color="warning" label="Not verified" />
          )}
        </Box>
        {!verified && emailEnabled && (
          <Box
            component="form"
            onSubmit={(e: SubmitEvent) => {
              e.preventDefault();
              confirm.mutate();
            }}
            sx={{ display: "flex", gap: 1, flexWrap: "wrap", alignItems: "center" }}
          >
            <Button
              variant={sent ? "text" : "outlined"}
              onClick={() => send.mutate()}
              disabled={send.isPending}
              sx={{ height: 36 }}
            >
              {sent ? "Resend code" : "Send verification code"}
            </Button>
            <TextField
              placeholder="Code from email"
              value={code}
              onChange={(e) => setCode(e.target.value)}
              inputMode="numeric"
              autoComplete="one-time-code"
              fullWidth={false}
              sx={{ width: 180 }}
            />
            <Button
              type="submit"
              variant="contained"
              disabled={confirm.isPending || code.trim() === ""}
              sx={{ height: 36 }}
            >
              Confirm
            </Button>
          </Box>
        )}
        {!verified && !emailEnabled && (
          <Alert severity="info">
            This server has no outgoing email configured, so addresses cannot be verified.
          </Alert>
        )}
      </Stack>
      <ChangeEmailDialog
        open={changeOpen}
        onClose={() => setChangeOpen(false)}
        emailEnabled={emailEnabled}
      />
    </Section>
  );
}

/**
 * Changing the email invalidates the OPAQUE record (it is bound to the email),
 * so the password is re-registered under the new address right afterwards.
 */
function ChangeEmailDialog({
  open,
  onClose,
  emailEnabled,
}: {
  open: boolean;
  onClose: () => void;
  emailEnabled: boolean;
}) {
  const qc = useQueryClient();
  const snack = useSnackbar();
  const [step, setStep] = useState<"email" | "code" | "password">("email");
  const [newEmail, setNewEmail] = useState("");
  const [code, setCode] = useState("");
  const [password, setPassword] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const reset = () => {
    setStep("email");
    setNewEmail("");
    setCode("");
    setPassword("");
    setError(null);
  };

  const run = async (fn: () => Promise<void>) => {
    setBusy(true);
    setError(null);
    try {
      await fn();
    } catch (e) {
      if (!(e instanceof UnlockCancelled)) setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  const afterChange = async () => {
    await qc.invalidateQueries({ queryKey: queryKeys.account });
    const fresh = await accountApi.get();
    authStore.updateUser(fresh.user);
    setStep("password");
  };

  const submit = (e: SubmitEvent) => {
    e.preventDefault();
    if (step === "email") {
      void run(async () => {
        await withStepUp(() => accountApi.emailChange(newEmail.trim()));
        if (emailEnabled) setStep("code");
        else await afterChange();
      });
    } else if (step === "code") {
      void run(async () => {
        await withStepUp(() => accountApi.emailChangeConfirm(code.trim()));
        await afterChange();
      });
    } else {
      void run(async () => {
        await requireUnlocked();
        await withStepUp(() => changePassword(password, false));
        snack.notify("Email changed");
        reset();
        onClose();
      });
    }
  };

  return (
    <Dialog
      open={open}
      onClose={busy || step === "password" ? undefined : onClose}
      maxWidth="xs"
      fullWidth
    >
      <form onSubmit={submit}>
        <DialogTitle>Change email</DialogTitle>
        <DialogContent sx={{ display: "grid", gap: 2 }}>
          {error && <Alert severity="error">{error}</Alert>}
          {step === "email" && (
            <>
              <DialogContentText>
                {emailEnabled
                  ? "We will send a confirmation code to the new address."
                  : "The address is changed immediately; you will then re-enter your password."}
              </DialogContentText>
              <TextField
                autoFocus
                label="New email"
                type="email"
                required
                value={newEmail}
                onChange={(e) => setNewEmail(e.target.value)}
                disabled={busy}
              />
            </>
          )}
          {step === "code" && (
            <>
              <DialogContentText>Enter the code sent to {newEmail}.</DialogContentText>
              <TextField
                autoFocus
                label="Confirmation code"
                required
                inputMode="numeric"
                autoComplete="one-time-code"
                value={code}
                onChange={(e) => setCode(e.target.value)}
                disabled={busy}
              />
            </>
          )}
          {step === "password" && (
            <>
              <Alert severity="warning">
                Your email is now {newEmail}. Re-enter your password to finish — until you do,
                signing in with a password is not possible.
              </Alert>
              <PasswordField
                autoFocus
                label="Current password"
                autoComplete="current-password"
                required
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                disabled={busy}
              />
            </>
          )}
        </DialogContent>
        <DialogActions sx={{ px: 3, pb: 2 }}>
          {step !== "password" && (
            <Button onClick={onClose} color="inherit" disabled={busy}>
              Cancel
            </Button>
          )}
          <Button type="submit" variant="contained" disabled={busy}>
            {step === "email"
              ? emailEnabled
                ? "Send code"
                : "Change"
              : step === "code"
                ? "Confirm"
                : "Finish"}
          </Button>
        </DialogActions>
      </form>
    </Dialog>
  );
}

function PasswordSection() {
  const snack = useSnackbar();
  const [open, setOpen] = useState(false);
  const [password, setPassword] = useState("");
  const [confirm, setConfirm] = useState("");
  const [revoke, setRevoke] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const problem = password.length > 0 ? passwordProblem(password) : null;
  const mismatch = confirm.length > 0 && confirm !== password;

  const close = () => {
    setOpen(false);
    setPassword("");
    setConfirm("");
    setError(null);
  };

  const submit = async (e: SubmitEvent) => {
    e.preventDefault();
    if (problem || mismatch) return;
    setBusy(true);
    setError(null);
    try {
      await requireUnlocked();
      await withStepUp(() => changePassword(password, revoke));
      snack.notify("Password changed");
      close();
    } catch (err) {
      if (!(err instanceof UnlockCancelled)) setError(errorMessage(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Section
      title="Password"
      description="Your password is never sent to the server; it unlocks your encryption keys locally (OPAQUE + Argon2id)."
      actions={
        <Button variant="outlined" onClick={() => setOpen(true)}>
          Change password
        </Button>
      }
    >
      <Dialog open={open} onClose={busy ? undefined : close} maxWidth="xs" fullWidth>
        <form onSubmit={submit}>
          <DialogTitle>Change password</DialogTitle>
          <DialogContent sx={{ display: "grid", gap: 2 }}>
            {error && <Alert severity="error">{error}</Alert>}
            <PasswordField
              autoFocus
              label="New password"
              autoComplete="new-password"
              required
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              error={problem !== null}
              helperText={problem ?? undefined}
              disabled={busy}
            />
            <PasswordField
              label="Confirm new password"
              autoComplete="new-password"
              required
              value={confirm}
              onChange={(e) => setConfirm(e.target.value)}
              error={mismatch}
              helperText={mismatch ? "Passwords do not match" : undefined}
              disabled={busy}
            />
            <FormControlLabel
              control={<Checkbox checked={revoke} onChange={(e) => setRevoke(e.target.checked)} />}
              label="Sign out all other devices"
            />
          </DialogContent>
          <DialogActions sx={{ px: 3, pb: 2 }}>
            <Button onClick={close} color="inherit" disabled={busy}>
              Cancel
            </Button>
            <Button type="submit" variant="contained" disabled={busy}>
              Change
            </Button>
          </DialogActions>
        </form>
      </Dialog>
    </Section>
  );
}

function RecoveryKeySection() {
  const navigate = useNavigate();
  const snack = useSnackbar();
  const { session } = useAuthState();
  const [confirmOpen, setConfirmOpen] = useState(false);
  const rotate = useMutation({
    mutationFn: async () => {
      const privateKey = await requireUnlocked();
      await loadCrypto();
      const r = rotateRecovery(privateKey);
      await withStepUp(() =>
        accountApi.rotateRecovery({
          recovery_wrapped_private_key: r.recoveryWrappedPrivateKey,
          recovery_verifier: r.recoveryVerifier,
        }),
      );
      return r.recoveryPhrase;
    },
    onSuccess: (phrase) => {
      setConfirmOpen(false);
      void navigate("/signup/recovery-key", {
        state: {
          phrase,
          email: session?.user.email,
          title: "Your new recovery key",
          next: "/account",
        },
      });
    },
    onError: (e) => {
      if (!(e instanceof UnlockCancelled)) snack.error(errorMessage(e));
    },
  });
  return (
    <Section
      title="Recovery key"
      description="A 24-word key that can reset your password if you forget it. It is shown only once and cannot be reset: replacing it requires your password. If both are lost, the encrypted data is gone — Termoso holds no copy of your keys."
      actions={
        <Button variant="outlined" onClick={() => setConfirmOpen(true)}>
          Generate new key
        </Button>
      }
    >
      <Dialog
        open={confirmOpen}
        onClose={rotate.isPending ? undefined : () => setConfirmOpen(false)}
        maxWidth="xs"
        fullWidth
      >
        <DialogTitle>Generate a new recovery key?</DialogTitle>
        <DialogContent>
          <DialogContentText>
            You will be asked for your password first. The current recovery key stops working
            immediately; you will see the new key once — store it safely.
          </DialogContentText>
        </DialogContent>
        <DialogActions sx={{ px: 3, pb: 2 }}>
          <Button onClick={() => setConfirmOpen(false)} color="inherit" disabled={rotate.isPending}>
            Cancel
          </Button>
          <Button variant="contained" onClick={() => rotate.mutate()} disabled={rotate.isPending}>
            Generate
          </Button>
        </DialogActions>
      </Dialog>
    </Section>
  );
}
