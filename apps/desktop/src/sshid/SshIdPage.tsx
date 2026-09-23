import { useMemo, useState } from "react";
import {
  Alert,
  Box,
  Button,
  Chip,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  Divider,
  FormControlLabel,
  IconButton,
  InputAdornment,
  ListItemText,
  Menu,
  MenuItem,
  Stack,
  Switch,
  TextField,
  Tooltip,
  Typography,
} from "@mui/material";
import FingerprintRoundedIcon from "@mui/icons-material/FingerprintRounded";
import MoreHorizRoundedIcon from "@mui/icons-material/MoreHorizRounded";
import ContentCopyRoundedIcon from "@mui/icons-material/ContentCopyRounded";
import ExpandMoreRoundedIcon from "@mui/icons-material/ExpandMoreRounded";
import ComputerOutlinedIcon from "@mui/icons-material/ComputerOutlined";
import UsbRoundedIcon from "@mui/icons-material/UsbRounded";
import AddRoundedIcon from "@mui/icons-material/AddRounded";
import DeleteOutlineRoundedIcon from "@mui/icons-material/DeleteOutlineRounded";
import RefreshRoundedIcon from "@mui/icons-material/RefreshRounded";
import WarningAmberRoundedIcon from "@mui/icons-material/WarningAmberRounded";
import VisibilityRoundedIcon from "@mui/icons-material/VisibilityRounded";
import VisibilityOffRoundedIcon from "@mui/icons-material/VisibilityOffRounded";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { goToSettings } from "@/app/navigation";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { EmptyState } from "@/components/EmptyState";
import { Page, PageBody } from "@/components/PageHeader";
import { useSnackbar } from "@/components/Snackbar";
import { IconTile, Loading, Mono, SectionCard } from "@/components/ui";
import { relativeTime } from "@/hosts/HostList";
import * as ipc from "@/ipc/commands";
import { keys } from "@/ipc/hooks";
import { isReauthCancelled, withReauth } from "@/account/reauth";
import {
  SSH_ID_DEFAULT_TYPE,
  SSH_ID_KEY_TYPES,
  errorMessage,
  sshIdTypeLabel,
  type Fido2Device,
  type SkAlgorithm,
  type SshIdFido2Form,
  type SshIdKey,
  type SshIdKeyType,
  type SshIdProfile,
  type SshIdView,
} from "@/ipc/types";
import { copyToClipboard } from "@/lib/clipboard";
import { tr, trx } from "@/i18n";

/** Settings → SSH ID: the account's device-bound passkeys, published under
 *  `<sshid base>/<handle>` so `curl … >> authorized_keys` provisions a box.
 *  Private halves never show up here — the page only lists public keys. */
export function SshIdPage() {
  const view = useQuery({ queryKey: keys.sshid, queryFn: ipc.sshidView, staleTime: 5_000 });

  if (view.isPending) {
    return (
      <Page>
        <Loading />
      </Page>
    );
  }
  if (view.isError) {
    return (
      <Page>
        <EmptyState title={tr("SSH ID unavailable")} description={errorMessage(view.error)} />
      </Page>
    );
  }
  const v = view.data;
  return (
    <Page>
      <PageBody>
        <Box sx={{ maxWidth: 760, display: "flex", flexDirection: "column", gap: 1.5 }}>
          {!v.signedIn ? (
            <SignedOut />
          ) : v.profile === null ? (
            <Setup view={v} />
          ) : (
            <Profile view={v} profile={v.profile} />
          )}
        </Box>
      </PageBody>
    </Page>
  );
}

function useSshIdMutation<A>(fn: (arg: A) => Promise<SshIdView>) {
  const qc = useQueryClient();
  const snackbar = useSnackbar();
  return useMutation({
    mutationFn: (arg: A) => withReauth(() => fn(arg)),
    onSuccess: (data) => {
      qc.setQueryData(keys.sshid, data);
      void qc.invalidateQueries({ queryKey: ["sshKeys"] });
      void qc.invalidateQueries({ queryKey: ["identities"] });
      void qc.invalidateQueries({ queryKey: keys.devices });
    },
    onError: (e) => {
      if (!isReauthCancelled(e)) snackbar.error(errorMessage(e));
    },
  });
}

function Intro() {
  return (
    <Typography variant="body2" color="text.secondary">
      {tr(
        "SSH ID is a public page with your passkeys. Each signed-in device keeps its own private key and publishes only the public half; hosts fetch the list once and let every device in.",
      )}
    </Typography>
  );
}

function SignedOut() {
  return (
    <SectionCard sx={{ alignItems: "center", textAlign: "center", py: 4, gap: 1 }}>
      <IconTile size={56} tone="purple">
        <FingerprintRoundedIcon />
      </IconTile>
      <Typography variant="subtitle1">{tr("Sign in to set up SSH ID")}</Typography>
      <Typography variant="body2" color="text.secondary" sx={{ maxWidth: 420 }}>
        {tr(
          "SSH ID lives on your account server so every device you sign in on can publish its passkey. Local-only mode has no account to publish under.",
        )}
      </Typography>
      <Button variant="contained" onClick={() => goToSettings("account")} sx={{ mt: 1 }}>
        {tr("Go to Account")}
      </Button>
    </SectionCard>
  );
}

function Setup({ view }: { view: SshIdView }) {
  const [handle, setHandle] = useState("");
  const create = useSshIdMutation(ipc.sshidCreate);
  const normalized = handle.trim().replace(/^@/, "").toLowerCase();
  const valid = /^[a-z0-9][a-z0-9_-]{2,31}$/.test(normalized);
  const base = baseUrl(view);

  return (
    <>
      <SectionCard sx={{ alignItems: "center", textAlign: "center", pt: 4, gap: 1 }}>
        <IconTile size={56} tone="purple">
          <FingerprintRoundedIcon />
        </IconTile>
        <Typography variant="subtitle1">{tr("Set up your SSH ID")}</Typography>
        <Typography variant="body2" color="text.secondary" sx={{ maxWidth: 460 }}>
          {trx(
            "Pick a handle — your public keys will be served at {url}. Handles are 3–32 characters: letters, digits, {dash} and {underscore}.",
            {
              url: (
                <Mono>
                  {base}/{normalized || "<handle>"}
                </Mono>
              ),
              dash: <Mono>-</Mono>,
              underscore: <Mono>_</Mono>,
            },
          )}
        </Typography>
        <Box
          component="form"
          onSubmit={(e) => {
            e.preventDefault();
            if (valid && !create.isPending) create.mutate(normalized);
          }}
          sx={{ display: "flex", gap: 1, width: "100%", maxWidth: 460, mt: 1.5 }}
        >
          <TextField
            fullWidth
            autoFocus
            value={handle}
            onChange={(e) => setHandle(e.target.value)}
            placeholder="your_handle"
            autoComplete="off"
            error={handle.length > 0 && !valid}
            slotProps={{
              htmlInput: { "aria-label": tr("SSH ID handle"), spellCheck: false },
              input: { startAdornment: <InputAdornment position="start">@</InputAdornment> },
            }}
          />
          <Button
            type="submit"
            variant="contained"
            disabled={!valid || create.isPending}
            sx={{ flexShrink: 0 }}
          >
            {create.isPending ? tr("Creating…") : tr("Create")}
          </Button>
        </Box>
      </SectionCard>
      <SectionCard>
        <Intro />
      </SectionCard>
    </>
  );
}

function baseUrl(view: SshIdView): string {
  return view.baseUrl ?? "";
}

function hostOf(url: string): string {
  try {
    return new URL(url).host;
  } catch {
    return url;
  }
}

function Profile({ view, profile }: { view: SshIdView; profile: SshIdProfile }) {
  const snackbar = useSnackbar();
  const [type, setType] = useState<SshIdKeyType>(SSH_ID_DEFAULT_TYPE);
  const [typeMenu, setTypeMenu] = useState<HTMLElement | null>(null);
  const [menu, setMenu] = useState<HTMLElement | null>(null);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [confirmRotate, setConfirmRotate] = useState(false);
  const [fido2Open, setFido2Open] = useState(false);
  const [removeKey, setRemoveKey] = useState<SshIdKey | null>(null);

  const publish = useSshIdMutation(ipc.sshidPublish);
  const del = useSshIdMutation(ipc.sshidDelete);
  const rotate = useSshIdMutation(ipc.sshidRotate);
  const remove = useSshIdMutation(ipc.sshidRemoveKey);
  const removeDevice = useSshIdMutation(ipc.sshidRemoveDevice);

  const base = baseUrl(view);
  const typeUrl =
    type === SSH_ID_DEFAULT_TYPE ? profile.url : `${profile.url}/${sshIdTypeLabel(type)}`;
  const provision = `curl -fs ${typeUrl} >> ~/.ssh/authorized_keys`;

  const keysOfType = useMemo(
    () => profile.keys.filter((k) => k.key_type === type),
    [profile.keys, type],
  );
  const hardware = SSH_ID_KEY_TYPES.find((k) => k.value === type)?.hardware ?? false;
  const deviceCount = new Set(profile.keys.map((k) => k.device_id).filter(Boolean)).size;
  const unpublished = view.deviceKeys.filter((k) => !k.published);

  const copy = (text: string, what: string) =>
    copyToClipboard(text).then(
      () => snackbar.notify(tr("{what} copied", { what })),
      (e: unknown) => snackbar.error(errorMessage(e)),
    );

  return (
    <>
      <SectionCard sx={{ flexDirection: "row", alignItems: "center", gap: 1.5 }}>
        <IconTile tone="purple" size={36}>
          <FingerprintRoundedIcon />
        </IconTile>
        <Typography variant="subtitle2" noWrap sx={{ flex: 1 }} title={typeUrl}>
          {typeUrl}
        </Typography>
        <Tooltip title={tr("Copy URL")}>
          <IconButton
            size="small"
            onClick={() => void copy(typeUrl, "URL")}
            aria-label={tr("Copy URL")}
          >
            <ContentCopyRoundedIcon fontSize="small" />
          </IconButton>
        </Tooltip>
        <IconButton
          size="small"
          onClick={(e) => setMenu(e.currentTarget)}
          aria-label={tr("SSH ID actions")}
        >
          <MoreHorizRoundedIcon fontSize="small" />
        </IconButton>
        <Menu open={menu !== null} anchorEl={menu} onClose={() => setMenu(null)}>
          <MenuItem
            onClick={() => {
              setMenu(null);
              setConfirmRotate(true);
            }}
          >
            <RefreshRoundedIcon fontSize="small" sx={{ mr: 1 }} />
            {tr("Rotate this device's keys")}
          </MenuItem>
          <MenuItem
            onClick={() => {
              setMenu(null);
              setConfirmDelete(true);
            }}
            sx={{ color: "error.main" }}
          >
            <DeleteOutlineRoundedIcon fontSize="small" sx={{ mr: 1 }} />
            {tr("Delete SSH ID")}
          </MenuItem>
        </Menu>
      </SectionCard>

      <Typography variant="subtitle2" sx={{ mt: 0.5 }}>
        {tr("Passkeys")}
      </Typography>
      <SectionCard sx={{ gap: 1 }}>
        <Box sx={{ display: "flex", alignItems: "center", gap: 1 }}>
          <Button
            variant="text"
            color="inherit"
            size="small"
            onClick={(e) => setTypeMenu(e.currentTarget)}
            endIcon={<ExpandMoreRoundedIcon />}
            sx={{ fontWeight: 600 }}
            aria-label={tr("Passkey type")}
          >
            {sshIdTypeLabel(type)}
            {hardware && <UsbRoundedIcon sx={{ fontSize: 16, ml: 0.75, opacity: 0.7 }} />}
          </Button>
          <Menu open={typeMenu !== null} anchorEl={typeMenu} onClose={() => setTypeMenu(null)}>
            <TypeGroup label={tr("Hardware")} hint={tr("FIDO2 security keys")} />
            {SSH_ID_KEY_TYPES.filter((k) => k.hardware).map((k) => (
              <TypeItem
                key={k.value}
                value={k.value}
                selected={type === k.value}
                onSelect={(v) => {
                  setType(v);
                  setTypeMenu(null);
                }}
              />
            ))}
            <Divider />
            <TypeGroup label={tr("Software")} />
            {SSH_ID_KEY_TYPES.filter((k) => !k.hardware).map((k) => (
              <TypeItem
                key={k.value}
                value={k.value}
                selected={type === k.value}
                onSelect={(v) => {
                  setType(v);
                  setTypeMenu(null);
                }}
              />
            ))}
          </Menu>
          <Box sx={{ flex: 1 }} />
          <Tooltip title={tr("Copy provisioning command")}>
            <IconButton
              size="small"
              onClick={() => void copy(provision, "Command")}
              aria-label={tr("Copy provisioning command")}
            >
              <ContentCopyRoundedIcon fontSize="small" />
            </IconButton>
          </Tooltip>
        </Box>

        {keysOfType.length === 0 ? (
          <Box sx={{ px: 1, py: 1.5, borderRadius: 2, bgcolor: "surface.high" }}>
            {hardware ? (
              <>
                <Typography variant="body2" sx={{ fontWeight: 600 }}>
                  {tr("Add a FIDO2 key to use {type} passkeys", { type: sshIdTypeLabel(type) })}
                </Typography>
                <Typography variant="caption" color="text.secondary">
                  {tr(
                    "A hardware credential works from any signed-in device that has the token plugged in.",
                  )}
                </Typography>
              </>
            ) : (
              <Typography variant="body2" color="text.secondary">
                {tr("No {type} key is published yet.", { type: sshIdTypeLabel(type) })}
              </Typography>
            )}
          </Box>
        ) : (
          keysOfType.map((k) => (
            <KeyRow
              key={k.id}
              k={k}
              onCopy={() => void copy(k.public_key, tr("Public key"))}
              onRemove={k.current_device ? undefined : () => setRemoveKey(k)}
            />
          ))
        )}

        {hardware && (
          <Button
            variant="text"
            size="small"
            startIcon={<AddRoundedIcon />}
            onClick={() => setFido2Open(true)}
            sx={{ alignSelf: "flex-start" }}
          >
            {tr("Add FIDO2 Key")}
          </Button>
        )}
      </SectionCard>

      <SectionCard title={tr("Provision a host")}>
        <Typography variant="body2" color="text.secondary">
          {trx(
            "Run this once on the server; it appends the current {type} keys to {file}. Re-run after adding a device or rotating.",
            { type: sshIdTypeLabel(type), file: <Mono>~/.ssh/authorized_keys</Mono> },
          )}
        </Typography>
        <Box
          sx={{
            display: "flex",
            alignItems: "center",
            gap: 1,
            px: 1.25,
            py: 0.75,
            borderRadius: 2,
            bgcolor: "surface.high",
          }}
        >
          <Mono sx={{ flex: 1, minWidth: 0, overflowWrap: "anywhere" }}>{provision}</Mono>
          <Button size="small" variant="outlined" onClick={() => void copy(provision, "Command")}>
            {tr("Copy")}
          </Button>
        </Box>
      </SectionCard>

      {unpublished.length > 0 && (
        <Alert severity="warning" icon={<WarningAmberRoundedIcon fontSize="inherit" />}>
          {unpublished.length === 1
            ? tr("This device's {sshIdTypeLabel} key is not published yet", {
                sshIdTypeLabel: sshIdTypeLabel(unpublished[0]?.keyType ?? SSH_ID_DEFAULT_TYPE),
              })
            : tr("{length} of this device's keys are not published yet", {
                length: unpublished.length,
              })}{" "}
          — {tr("publishing needs a confirmation of your account.")}
          <Box sx={{ mt: 1 }}>
            <Button
              size="small"
              variant="outlined"
              color="inherit"
              disabled={publish.isPending}
              onClick={() => publish.mutate(undefined)}
            >
              {tr("Publish now")}
            </Button>
          </Box>
        </Alert>
      )}
      {deviceCount < 2 && (
        <Alert severity="warning" icon={<WarningAmberRoundedIcon fontSize="inherit" />}>
          <Typography variant="body2" sx={{ fontWeight: 600 }}>
            {tr("It is recommended to use at least two devices")}
          </Typography>
          <Typography variant="body2">
            {tr(
              "Sign in on another device or add a FIDO2 key so you keep access if this one is lost.",
            )}
          </Typography>
        </Alert>
      )}

      <ConfirmDialog
        open={confirmDelete}
        title={tr("Delete SSH ID?")}
        danger
        confirmLabel={tr("Delete")}
        busy={del.isPending}
        onCancel={() => setConfirmDelete(false)}
        onConfirm={() =>
          del.mutate(undefined, {
            onSuccess: () => setConfirmDelete(false),
          })
        }
      >
        <Typography variant="body2">
          {trx(
            "{handle} and every published key are removed from {server}. Hosts already provisioned keep the old public keys until you edit their {file}. This device's private keys are deleted.",
            {
              handle: <Mono>@{profile.handle}</Mono>,
              server: hostOf(base) || tr("the server"),
              file: <Mono>authorized_keys</Mono>,
            },
          )}
        </Typography>
      </ConfirmDialog>

      <ConfirmDialog
        open={confirmRotate}
        title={tr("Rotate this device's keys?")}
        confirmLabel={tr("Rotate")}
        busy={rotate.isPending}
        onCancel={() => setConfirmRotate(false)}
        onConfirm={() =>
          rotate.mutate(undefined, {
            onSuccess: () => setConfirmRotate(false),
          })
        }
      >
        <Typography variant="body2">
          {tr(
            "New ED25519, ECDSA and RSA passkeys are generated for this device and published in place of the old ones. Re-run the provisioning command on your hosts afterwards.",
          )}
        </Typography>
      </ConfirmDialog>

      <ConfirmDialog
        open={removeKey !== null}
        title={
          removeKey?.device_id === null ? tr("Remove FIDO2 key?") : tr("Sign out this device?")
        }
        danger
        confirmLabel={removeKey?.device_id === null ? tr("Remove") : tr("Sign out")}
        busy={remove.isPending || removeDevice.isPending}
        onCancel={() => setRemoveKey(null)}
        onConfirm={() => {
          if (!removeKey) return;
          const done = { onSuccess: () => setRemoveKey(null) };
          if (removeKey.device_id === null) remove.mutate(removeKey.id, done);
          else removeDevice.mutate(removeKey.device_id, done);
        }}
      >
        {removeKey?.device_id === null ? (
          <Typography variant="body2">
            {trx(
              "{key} is unpublished and the local credential handle is deleted. The token itself is not modified.",
              { key: <b>{removeKey.label}</b> },
            )}
          </Typography>
        ) : (
          <Typography variant="body2">
            {trx(
              "{device} is signed out of your account and all of its passkeys are unpublished. Its private keys stay on that device, so remove them from {file} on hosts you provisioned if the device was lost.",
              { device: <b>{removeKey?.label}</b>, file: <Mono>authorized_keys</Mono> },
            )}
          </Typography>
        )}
      </ConfirmDialog>

      {fido2Open && (
        <AddFido2Dialog
          algorithm={type === "ed25519_sk" ? "ed25519" : "ecdsa_p256"}
          handle={profile.handle}
          onClose={() => setFido2Open(false)}
        />
      )}
    </>
  );
}

function TypeGroup({ label, hint }: { label: string; hint?: string }) {
  return (
    <Box sx={{ px: 2, pt: 1, pb: 0.5 }}>
      <Typography variant="caption" color="text.secondary" sx={{ display: "block" }}>
        {label}
      </Typography>
      {hint && (
        <Typography variant="caption" color="text.disabled" sx={{ display: "block" }}>
          {hint}
        </Typography>
      )}
    </Box>
  );
}

function TypeItem({
  value,
  selected,
  onSelect,
}: {
  value: SshIdKeyType;
  selected: boolean;
  onSelect: (v: SshIdKeyType) => void;
}) {
  const t = SSH_ID_KEY_TYPES.find((k) => k.value === value);
  return (
    <MenuItem selected={selected} onClick={() => onSelect(value)} sx={{ minWidth: 220 }}>
      <ListItemText
        primary={
          <Box sx={{ display: "flex", alignItems: "center", gap: 0.75 }}>
            {sshIdTypeLabel(value)}
            {value === SSH_ID_DEFAULT_TYPE && <Chip size="small" label={tr("Default")} />}
          </Box>
        }
        secondary={t?.hint}
      />
    </MenuItem>
  );
}

function KeyRow({
  k,
  onCopy,
  onRemove,
}: {
  k: SshIdKey;
  onCopy: () => void;
  onRemove?: () => void;
}) {
  const hardware = k.device_id === null;
  return (
    <Box sx={{ display: "flex", flexDirection: "column", gap: 0.5 }}>
      <Box sx={{ display: "flex", alignItems: "center", gap: 1 }}>
        {hardware ? (
          <UsbRoundedIcon sx={{ fontSize: 16, color: "text.secondary" }} />
        ) : (
          <ComputerOutlinedIcon sx={{ fontSize: 16, color: "text.secondary" }} />
        )}
        <Typography variant="body2" sx={{ fontWeight: 600, flex: 1 }} noWrap>
          {k.label}
        </Typography>
        {k.current_device && <Chip size="small" label={tr("This device")} />}
        <Typography variant="caption" color="text.secondary">
          {relativeTime(k.updated_at)}
        </Typography>
        <Tooltip title={tr("Copy public key")}>
          <IconButton
            size="small"
            onClick={onCopy}
            aria-label={tr("Copy {label} public key", { label: k.label })}
          >
            <ContentCopyRoundedIcon sx={{ fontSize: 16 }} />
          </IconButton>
        </Tooltip>
        {onRemove && (
          <Tooltip title={hardware ? tr("Remove key") : tr("Sign out device")}>
            <IconButton
              size="small"
              onClick={onRemove}
              aria-label={tr("Remove {label}", { label: k.label })}
            >
              <DeleteOutlineRoundedIcon sx={{ fontSize: 16 }} />
            </IconButton>
          </Tooltip>
        )}
      </Box>
      <Mono
        sx={{
          px: 1,
          py: 0.75,
          borderRadius: 1.5,
          bgcolor: "surface.high",
          color: "text.secondary",
          fontSize: 11,
          overflowWrap: "anywhere",
        }}
      >
        {k.public_key}
      </Mono>
    </Box>
  );
}

function deviceName(d: Fido2Device): string {
  return d.product.trim() || `USB ${d.vendorId.toString(16)}:${d.productId.toString(16)}`;
}

/** Make a fresh credential on a plugged-in token and publish its public key.
 *  Polls for tokens while open; the token's private key never leaves it. */
function AddFido2Dialog({
  algorithm,
  handle,
  onClose,
}: {
  algorithm: SkAlgorithm;
  handle: string;
  onClose: () => void;
}) {
  const add = useSshIdMutation(ipc.sshidAddFido2);
  const devices = useQuery({
    queryKey: ["fido2", "devices"],
    queryFn: ipc.fido2Devices,
    refetchInterval: add.isPending ? false : 2000,
    refetchIntervalInBackground: false,
  });
  const list = devices.data ?? [];
  const [devicePath, setDevicePath] = useState<string | null>(null);
  const device = list.find((d) => d.path === devicePath) ?? list[0] ?? null;
  const [label, setLabel] = useState("");
  const [userPresence, setUserPresence] = useState(true);
  const [userVerification, setUserVerification] = useState(false);
  const [resident, setResident] = useState(false);
  const [pin, setPin] = useState("");
  const [showPin, setShowPin] = useState(false);

  const supported = device === null || device.algorithms.includes(algorithm);
  const canResident = device?.residentKeys ?? false;
  const pinNeeded = device?.pinSet === true || userVerification || (resident && canResident);
  const valid =
    device !== null && supported && label.trim().length > 0 && (!pinNeeded || pin.length >= 4);

  const submit = () => {
    if (!device || !valid) return;
    const form: SshIdFido2Form = {
      label: label.trim(),
      device: device.path,
      algorithm,
      resident: resident && canResident,
      userPresence,
      userVerification,
      pin: pin.length > 0 ? pin : null,
      user: null,
      comment: tr("SSH ID - @{handle}", { handle }),
    };
    add.mutate(form, { onSuccess: onClose });
  };

  return (
    <Dialog open onClose={add.isPending ? undefined : onClose} maxWidth="xs" fullWidth>
      <DialogTitle>{tr("Add FIDO2 Key")}</DialogTitle>
      <DialogContent sx={{ display: "flex", flexDirection: "column", gap: 1.5 }}>
        {!device ? (
          <Box sx={{ textAlign: "center", color: "text.secondary", py: 3 }}>
            <IconTile size={56} tone="neutral" sx={{ mx: "auto" }}>
              <UsbRoundedIcon />
            </IconTile>
            <Typography variant="subtitle1" color="text.primary" sx={{ mt: 2 }}>
              {tr("Insert FIDO2 device")}
            </Typography>
            <Typography variant="body2" sx={{ mt: 0.5 }}>
              {devices.isError
                ? errorMessage(devices.error)
                : tr("Connect your FIDO2 device to show here.")}
            </Typography>
          </Box>
        ) : (
          <>
            <TextField
              select
              label={tr("Security key")}
              value={device.path}
              onChange={(e) => setDevicePath(e.target.value)}
              slotProps={{ htmlInput: { "aria-label": tr("Security key") } }}
            >
              {list.map((d) => (
                <MenuItem key={d.path} value={d.path}>
                  {deviceName(d)}
                </MenuItem>
              ))}
            </TextField>
            {!supported && (
              <Alert severity="warning">
                {tr("This token does not support {algorithm}.", {
                  algorithm: algorithm === "ed25519" ? "sk-ssh-ed25519" : "sk-ecdsa-sha2-nistp256",
                })}
              </Alert>
            )}
            <TextField
              autoFocus
              value={label}
              onChange={(e) => setLabel(e.target.value)}
              placeholder={tr("Label")}
              autoComplete="off"
              slotProps={{ htmlInput: { "aria-label": tr("Label") } }}
            />
            <Stack spacing={0}>
              <FormControlLabel
                control={
                  <Switch
                    checked={userPresence}
                    onChange={(e) => setUserPresence(e.target.checked)}
                  />
                }
                label={<Typography variant="body2">{tr("Require User Presence")}</Typography>}
              />
              <FormControlLabel
                control={
                  <Switch
                    checked={userVerification}
                    onChange={(e) => setUserVerification(e.target.checked)}
                  />
                }
                label={<Typography variant="body2">{tr("Require PIN Code")}</Typography>}
              />
              <Tooltip
                title={
                  canResident
                    ? tr(
                        "Store the credential on the token so it can be loaded on another computer",
                      )
                    : tr("This token cannot store resident credentials")
                }
                placement="left"
              >
                <FormControlLabel
                  control={
                    <Switch
                      checked={resident && canResident}
                      disabled={!canResident}
                      onChange={(e) => setResident(e.target.checked)}
                    />
                  }
                  label={<Typography variant="body2">{tr("Resident key")}</Typography>}
                />
              </Tooltip>
            </Stack>
            {pinNeeded && (
              <TextField
                type={showPin ? "text" : "password"}
                value={pin}
                onChange={(e) => setPin(e.target.value)}
                placeholder={tr("PIN")}
                autoComplete="off"
                inputMode="numeric"
                slotProps={{
                  htmlInput: { "aria-label": tr("PIN") },
                  input: {
                    endAdornment: (
                      <InputAdornment position="end">
                        <IconButton
                          size="small"
                          onClick={() => setShowPin((v) => !v)}
                          aria-label={tr("Toggle PIN visibility")}
                        >
                          {showPin ? (
                            <VisibilityOffRoundedIcon fontSize="small" />
                          ) : (
                            <VisibilityRoundedIcon fontSize="small" />
                          )}
                        </IconButton>
                      </InputAdornment>
                    ),
                  },
                }}
              />
            )}
          </>
        )}
      </DialogContent>
      <DialogActions>
        <Button variant="text" color="inherit" onClick={onClose} disabled={add.isPending}>
          {tr("Cancel")}
        </Button>
        {device && (
          <Button variant="contained" disabled={!valid || add.isPending} onClick={submit}>
            {add.isPending ? tr("Touch your security key…") : tr("Add")}
          </Button>
        )}
      </DialogActions>
    </Dialog>
  );
}
