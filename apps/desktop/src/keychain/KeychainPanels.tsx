import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { useQuery } from "@tanstack/react-query";
import {
  Alert,
  Box,
  Button,
  Checkbox,
  Chip,
  FormControlLabel,
  IconButton,
  InputAdornment,
  ListItemText,
  Menu,
  MenuItem,
  Stack,
  Switch,
  TextField,
  Typography,
} from "@mui/material";
import KeyRoundedIcon from "@mui/icons-material/KeyRounded";
import KeyOutlinedIcon from "@mui/icons-material/KeyOutlined";
import BadgeOutlinedIcon from "@mui/icons-material/BadgeOutlined";
import BadgeRoundedIcon from "@mui/icons-material/BadgeRounded";
import WorkspacePremiumOutlinedIcon from "@mui/icons-material/WorkspacePremiumOutlined";
import UsbRoundedIcon from "@mui/icons-material/UsbRounded";
import FingerprintRoundedIcon from "@mui/icons-material/FingerprintRounded";
import PersonOutlineRoundedIcon from "@mui/icons-material/PersonOutlineRounded";
import PasswordRoundedIcon from "@mui/icons-material/PasswordRounded";
import VisibilityRoundedIcon from "@mui/icons-material/VisibilityRounded";
import VisibilityOffRoundedIcon from "@mui/icons-material/VisibilityOffRounded";
import CloseRoundedIcon from "@mui/icons-material/CloseRounded";
import AddRoundedIcon from "@mui/icons-material/AddRounded";
import MoreHorizRoundedIcon from "@mui/icons-material/MoreHorizRounded";
import ContentCopyRoundedIcon from "@mui/icons-material/ContentCopyRounded";
import FolderOpenOutlinedIcon from "@mui/icons-material/FolderOpenOutlined";
import LockOutlinedIcon from "@mui/icons-material/LockOutlined";
import FileDownloadOutlinedIcon from "@mui/icons-material/FileDownloadOutlined";
import NoteAddOutlinedIcon from "@mui/icons-material/NoteAddOutlined";
import { open as openFile } from "@tauri-apps/plugin-dialog";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import * as ipc from "@/ipc/commands";
import {
  SSH_ID_DEFAULT_TYPE,
  SSH_ID_KEY_TYPES,
  errorMessage,
  sshIdTypeLabel,
  type CertificateCard,
  type Fido2Device,
  type Fido2GenerateForm,
  type Fido2LoadForm,
  type GenerateKeyForm,
  type IdentityCard,
  type IdentityForm,
  type ImportKeyForm,
  type KeyAlgorithm,
  type KeyCard,
  type KeyPreview,
  type SkAlgorithm,
  type SshIdKeyType,
  type Uuid,
} from "@/ipc/types";
import {
  ActionMenu,
  IconTile,
  Mono,
  SectionCard,
  SettingRow,
  SidePanel,
  ToolIconButton,
  type MenuAction,
} from "@/components/ui";
import { sizes } from "@/theme/theme";
import {
  certificateName,
  certificateState,
  certificateSummary,
  droppedKind,
  isHardwareKey,
  keyTypeLabel,
  labelFromPath,
} from "./model";

/* ---------------------------------------------------------------- tiles */

export function KeyTile({ card, size = sizes.tile }: { card?: KeyCard; size?: number }) {
  return (
    <IconTile size={size} tone={card?.unreadable ? "warning" : "info"}>
      {card?.securityKey ? <UsbRoundedIcon /> : <KeyRoundedIcon />}
    </IconTile>
  );
}

export function IdentityTile({ size = sizes.tile }: { size?: number }) {
  return (
    <IconTile size={size} tone="info">
      <BadgeRoundedIcon />
    </IconTile>
  );
}

const adornment = (icon: ReactNode) => (
  <InputAdornment position="start" sx={{ color: "text.secondary" }}>
    {icon}
  </InputAdornment>
);

const mono = { spellCheck: false, style: { fontFamily: "monospace", fontSize: 12 } } as const;

function useDebounced<T>(value: T, ms: number): T {
  const [v, setV] = useState(value);
  useEffect(() => {
    const t = setTimeout(() => setV(value), ms);
    return () => clearTimeout(t);
  }, [value, ms]);
  return v;
}

interface Inspection<T> {
  value: T | null;
  error: string | null;
}

/** Runs `inspect` on the (debounced) text and exposes the result only while
 *  it still belongs to the current text — no stale previews, no flicker. */
function useInspection<T>(text: string, inspect: (text: string) => Promise<T>): Inspection<T> {
  const [result, setResult] = useState<(Inspection<T> & { text: string }) | null>(null);
  useEffect(() => {
    if (!text) return;
    let alive = true;
    inspect(text)
      .then((value) => {
        if (alive) setResult({ text, value, error: null });
      })
      .catch((e: unknown) => {
        if (alive) setResult({ text, value: null, error: errorMessage(e) });
      });
    return () => {
      alive = false;
    };
  }, [text, inspect]);
  return text && result?.text === text ? result : { value: null, error: null };
}

/** Native file drop anywhere over the window while the panel is open. */
function useFileDrop(onPaths: (paths: string[]) => void) {
  const cb = useRef(onPaths);
  useEffect(() => {
    cb.current = onPaths;
  });
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let alive = true;
    void getCurrentWebview()
      .onDragDropEvent((e) => {
        if (e.payload.type === "drop" && e.payload.paths.length > 0) cb.current(e.payload.paths);
      })
      .then((off) => {
        if (alive) unlisten = off;
        else off();
      })
      .catch(() => undefined);
    return () => {
      alive = false;
      unlisten?.();
    };
  }, []);
}

function PanelMenuButton({ items }: { items: MenuAction[] }) {
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);
  if (items.length === 0) return null;
  return (
    <>
      <IconButton aria-label="More" onClick={(e) => setAnchor(e.currentTarget)}>
        <MoreHorizRoundedIcon fontSize="small" />
      </IconButton>
      <ActionMenu anchor={anchor} onClose={() => setAnchor(null)} items={items} />
    </>
  );
}

/** Certificate metadata under the certificate field. */
function CertificateFacts({
  cert,
  unreadable,
}: {
  cert: CertificateCard | null;
  unreadable: boolean;
}) {
  const state = certificateState(cert, unreadable);
  if (!state) return null;
  if (state === "unreadable" || !cert) {
    return (
      <Typography variant="caption" color="warning.main">
        The attached certificate cannot be parsed.
      </Typography>
    );
  }
  return (
    <Stack spacing={0.5}>
      <Stack direction="row" spacing={0.5} sx={{ flexWrap: "wrap", rowGap: 0.5 }}>
        <Chip
          size="small"
          color={state === "valid" ? "success" : "warning"}
          variant="outlined"
          label={state === "valid" ? "Valid" : state === "expired" ? "Expired" : "Not yet valid"}
        />
        <Chip size="small" variant="outlined" label={cert.kind === "host" ? "Host" : "User"} />
        {cert.keyId && <Chip size="small" variant="outlined" label={`ID ${cert.keyId}`} />}
        {cert.serial > 0 && <Chip size="small" variant="outlined" label={`#${cert.serial}`} />}
      </Stack>
      <Typography variant="caption" color="text.secondary">
        {certificateSummary(cert)}
      </Typography>
      <Typography variant="caption" color="text.secondary" noWrap>
        CA <Mono>{cert.caFingerprint}</Mono>
      </Typography>
    </Stack>
  );
}

/** Certificate textarea with file picker, live parse + key match. */
function CertificateField({
  value,
  onChange,
  onPickFile,
  autoFocus,
  keyFingerprint,
  disabled,
  storedFacts,
}: {
  value: string;
  onChange: (v: string) => void;
  onPickFile: () => void;
  autoFocus?: boolean;
  /** Fingerprint of the key the certificate must be issued for (when known). */
  keyFingerprint: string | null;
  disabled?: boolean;
  /** Facts for the stored certificate when the text has not been touched. */
  storedFacts?: ReactNode;
}) {
  const debounced = useDebounced(value.trim(), 250);
  const { value: preview, error } = useInspection(debounced, ipc.certificateInspect);
  const mismatch =
    preview !== null && keyFingerprint !== null && preview.fingerprint !== keyFingerprint;
  return (
    <Stack spacing={0.75}>
      <TextField
        value={value}
        onChange={(e) => onChange(e.target.value)}
        autoFocus={autoFocus}
        disabled={disabled}
        multiline
        minRows={3}
        maxRows={6}
        placeholder="Certificate"
        error={error !== null || mismatch}
        helperText={
          error ?? (mismatch ? `Issued for a different key (${preview.fingerprint})` : undefined)
        }
        slotProps={{
          htmlInput: { ...mono, "aria-label": "Certificate" },
          input: {
            endAdornment: (
              <InputAdornment position="end" sx={{ alignSelf: "flex-start", mt: 1 }}>
                <ToolIconButton title="Certificate file (*-cert.pub)…" onClick={onPickFile}>
                  <FolderOpenOutlinedIcon fontSize="small" />
                </ToolIconButton>
              </InputAdornment>
            ),
          },
        }}
      />
      {preview && !mismatch ? <CertificateFacts cert={preview} unreadable={false} /> : storedFacts}
    </Stack>
  );
}

/* ------------------------------------------------------------- new key */

export interface ImportFileArgs {
  vaultId: Uuid;
  label: string;
  path: string;
  passphrase: string | null;
  rememberPassphrase: boolean;
  certificate: string | null;
  certificatePath: string | null;
}

/** Termius "New Key": paste or drop a private key (OpenSSH / PEM / PuTTY .ppk),
 *  optionally a certificate, and save. File contents are read in Rust. */
export function NewKeyPanel({
  vaultId,
  vaultName,
  focusCertificate,
  busy,
  error,
  onImport,
  onImportFile,
  onClose,
}: {
  vaultId: Uuid;
  vaultName: string;
  focusCertificate?: boolean;
  busy: boolean;
  error: string | null;
  onImport: (form: ImportKeyForm) => void;
  onImportFile: (args: ImportFileArgs) => void;
  onClose: () => void;
}) {
  const [label, setLabel] = useState("");
  const [text, setText] = useState("");
  const [path, setPath] = useState<string | null>(null);
  const [filePreview, setFilePreview] = useState<Inspection<KeyPreview>>({
    value: null,
    error: null,
  });
  const [passphrase, setPassphrase] = useState("");
  const [remember, setRemember] = useState(true);
  const [cert, setCert] = useState("");
  const [certPath, setCertPath] = useState<string | null>(null);
  const [certFile, setCertFile] = useState<CertificateCard | null>(null);
  const [certFileError, setCertFileError] = useState<string | null>(null);
  const [dropNote, setDropNote] = useState<string | null>(null);

  const debouncedText = useDebounced(path === null ? text.trim() : "", 250);
  const textPreview = useInspection(debouncedText, ipc.keyInspect);
  const preview = path !== null ? filePreview.value : textPreview.value;
  const previewError = path !== null ? filePreview.error : textPreview.error;

  const takePrivateFile = (p: string) => {
    setPath(p);
    setText("");
    setDropNote(null);
    setFilePreview({ value: null, error: null });
    if (label.trim().length === 0) setLabel(labelFromPath(p));
    ipc
      .keyInspectFile(p)
      .then((pv) => setFilePreview({ value: pv, error: null }))
      .catch((e: unknown) => setFilePreview({ value: null, error: errorMessage(e) }));
  };
  const takeCertFile = (p: string) => {
    setCertPath(p);
    setCert("");
    ipc
      .certificateInspectFile(p)
      .then((c) => {
        setCertFile(c);
        setCertFileError(null);
      })
      .catch((e: unknown) => {
        setCertFile(null);
        setCertFileError(errorMessage(e));
      });
  };
  const takePaths = (paths: string[]) => {
    for (const p of paths) {
      switch (droppedKind(p)) {
        case "certificate":
          takeCertFile(p);
          break;
        case "public":
          setDropNote("That is a public key (.pub); drop the private key file instead.");
          break;
        case "private":
          takePrivateFile(p);
          break;
      }
    }
  };
  useFileDrop(takePaths);

  const pickPrivate = async () => {
    const picked = await openFile({ multiple: false, directory: false, title: "Private key file" });
    if (typeof picked === "string") takePaths([picked]);
  };
  const pickCert = async () => {
    const picked = await openFile({
      multiple: false,
      directory: false,
      title: "Certificate (*-cert.pub)",
    });
    if (typeof picked === "string") takeCertFile(picked);
  };

  const encrypted = preview?.encrypted ?? false;
  const certMismatch =
    certFile !== null && preview !== null && certFile.fingerprint !== preview.fingerprint;
  const havePrivate = path !== null || text.trim().length > 0;
  const valid =
    label.trim().length > 0 &&
    havePrivate &&
    previewError === null &&
    !(encrypted && passphrase.length === 0) &&
    certFileError === null &&
    !certMismatch;

  const submit = () => {
    const pass = passphrase.length > 0 ? passphrase : null;
    const certText = cert.trim().length > 0 ? cert : null;
    if (path !== null) {
      onImportFile({
        vaultId,
        label: label.trim(),
        path,
        passphrase: pass,
        rememberPassphrase: pass !== null && remember,
        certificate: certPath ? null : certText,
        certificatePath: certPath,
      });
    } else {
      onImport({
        vaultId,
        label: label.trim(),
        privateKey: text,
        passphrase: pass,
        rememberPassphrase: pass !== null && remember,
        certificate: certText,
      });
    }
  };

  const privateHint = path
    ? preview
      ? `${preview.putty ? "PuTTY .ppk" : "Private key file"} · ${keyTypeLabel({
          keyType: preview.keyType,
          bits: preview.bits,
          unreadable: false,
        })}${preview.encrypted ? " · passphrase-protected" : ""}`
      : "Read on save; the file contents never leave the app core."
    : preview
      ? `${preview.putty ? "PuTTY .ppk, converted to OpenSSH on save" : "OpenSSH / PEM"} · ${keyTypeLabel(
          {
            keyType: preview.keyType,
            bits: preview.bits,
            unreadable: false,
          },
        )}${preview.encrypted ? " · passphrase-protected" : ""}`
      : "OpenSSH, PEM / PKCS#8 or PuTTY .ppk (v2 / v3)";

  return (
    <SidePanel
      title="New Key"
      subtitle={vaultName}
      onClose={onClose}
      footer={
        <>
          <Button variant="text" color="inherit" onClick={onClose} disabled={busy}>
            Cancel
          </Button>
          <Button variant="contained" onClick={submit} disabled={!valid || busy}>
            {busy ? "Saving…" : "Save"}
          </Button>
        </>
      }
    >
      <SectionCard>
        <TextField
          autoFocus={!focusCertificate}
          value={label}
          onChange={(e) => setLabel(e.target.value)}
          placeholder="Label"
          slotProps={{ htmlInput: { "aria-label": "Label" } }}
        />
        {path === null ? (
          <TextField
            value={text}
            onChange={(e) => setText(e.target.value)}
            multiline
            minRows={4}
            maxRows={10}
            required
            placeholder="Private key *"
            error={previewError !== null}
            helperText={previewError ?? privateHint}
            slotProps={{ htmlInput: { ...mono, "aria-label": "Private key" } }}
          />
        ) : (
          <TextField
            value={path}
            disabled
            error={previewError !== null}
            helperText={previewError ?? privateHint}
            slotProps={{
              htmlInput: { "aria-label": "Private key file" },
              input: {
                startAdornment: adornment(<KeyOutlinedIcon fontSize="small" />),
                endAdornment: (
                  <InputAdornment position="end">
                    <IconButton
                      size="small"
                      aria-label="Remove file"
                      onClick={() => {
                        setPath(null);
                        setFilePreview({ value: null, error: null });
                      }}
                    >
                      <CloseRoundedIcon fontSize="small" />
                    </IconButton>
                  </InputAdornment>
                ),
              },
            }}
          />
        )}
        {(encrypted || passphrase.length > 0) && (
          <Stack spacing={0.5}>
            <TextField
              type="password"
              value={passphrase}
              onChange={(e) => setPassphrase(e.target.value)}
              placeholder="Passphrase *"
              autoComplete="off"
              autoFocus={encrypted}
              slotProps={{
                htmlInput: { "aria-label": "Passphrase" },
                input: { startAdornment: adornment(<LockOutlinedIcon fontSize="small" />) },
              }}
              helperText="The key is passphrase-protected; it is needed once to import."
            />
            <FormControlLabel
              control={
                <Checkbox checked={remember} onChange={(e) => setRemember(e.target.checked)} />
              }
              label={<Typography variant="body2">Remember passphrase in the vault</Typography>}
            />
          </Stack>
        )}
        <TextField
          value={preview?.publicKey ?? ""}
          multiline
          minRows={3}
          maxRows={5}
          placeholder="Public key"
          helperText={
            preview ? (
              <>
                Derived from the private key · <Mono>{preview.fingerprint}</Mono>
              </>
            ) : undefined
          }
          slotProps={{
            htmlInput: { ...mono, readOnly: true, "aria-label": "Public key" },
            input: {
              endAdornment: preview ? (
                <InputAdornment position="end" sx={{ alignSelf: "flex-start", mt: 1 }}>
                  <ToolIconButton
                    title="Copy public key"
                    onClick={() => void navigator.clipboard.writeText(preview.publicKey)}
                  >
                    <ContentCopyRoundedIcon fontSize="small" />
                  </ToolIconButton>
                </InputAdornment>
              ) : undefined,
            },
          }}
        />
        {certPath === null ? (
          <CertificateField
            value={cert}
            onChange={setCert}
            onPickFile={() => void pickCert()}
            autoFocus={focusCertificate}
            keyFingerprint={preview?.fingerprint ?? null}
          />
        ) : (
          <Stack spacing={0.75}>
            <TextField
              value={certPath}
              disabled
              error={certFileError !== null || certMismatch}
              helperText={
                certFileError ??
                (certMismatch ? `Issued for a different key (${certFile.fingerprint})` : undefined)
              }
              slotProps={{
                htmlInput: { "aria-label": "Certificate file" },
                input: {
                  startAdornment: adornment(<WorkspacePremiumOutlinedIcon fontSize="small" />),
                  endAdornment: (
                    <InputAdornment position="end">
                      <IconButton
                        size="small"
                        aria-label="Remove certificate file"
                        onClick={() => {
                          setCertPath(null);
                          setCertFile(null);
                          setCertFileError(null);
                        }}
                      >
                        <CloseRoundedIcon fontSize="small" />
                      </IconButton>
                    </InputAdornment>
                  ),
                },
              }}
            />
            {certFile && !certMismatch && <CertificateFacts cert={certFile} unreadable={false} />}
          </Stack>
        )}
        {error && (
          <Alert severity="error" variant="outlined">
            {error}
          </Alert>
        )}
      </SectionCard>

      <SectionCard>
        <Box
          sx={{
            border: 1,
            borderStyle: "dashed",
            borderColor: "border.strong",
            borderRadius: 2,
            py: 3,
            px: 2,
            textAlign: "center",
            color: "text.secondary",
          }}
        >
          <NoteAddOutlinedIcon sx={{ fontSize: 32, mb: 1, opacity: 0.7 }} />
          <Typography variant="body2">Drag and drop a private key file to import</Typography>
          <Typography variant="caption" color="text.disabled">
            .ppk, id_ed25519, *.pem — a *-cert.pub next to it attaches as certificate
          </Typography>
          {dropNote && (
            <Typography variant="caption" color="warning.main" sx={{ display: "block", mt: 1 }}>
              {dropNote}
            </Typography>
          )}
        </Box>
        <Button variant="contained" fullWidth onClick={() => void pickPrivate()} disabled={busy}>
          Import from key file
        </Button>
      </SectionCard>
    </SidePanel>
  );
}

/* ------------------------------------------------------------ edit key */

/** Certificate textarea for a stored key; remounted (via `key`) whenever the
 *  stored text changes so the draft always starts from what is in the vault. */
function StoredCertificateEditor({
  card,
  stored,
  loading,
  busy,
  onPickFile,
  onSetCertificate,
}: {
  card: KeyCard;
  stored: string;
  loading: boolean;
  busy: boolean;
  onPickFile: () => void;
  onSetCertificate: (text: string | null) => void;
}) {
  const [cert, setCert] = useState(stored);
  const hasCert = card.certificate !== null || card.certificateUnreadable;
  const dirty = cert.trim() !== stored.trim();
  return (
    <>
      <CertificateField
        value={cert}
        onChange={setCert}
        onPickFile={onPickFile}
        keyFingerprint={card.unreadable ? null : card.fingerprint}
        disabled={busy || loading}
        storedFacts={
          !dirty ? (
            <CertificateFacts cert={card.certificate} unreadable={card.certificateUnreadable} />
          ) : undefined
        }
      />
      {(dirty || hasCert) && (
        <Stack direction="row" spacing={1} sx={{ justifyContent: "flex-end" }}>
          {hasCert && (
            <Button
              size="small"
              color="inherit"
              disabled={busy}
              onClick={() => onSetCertificate(null)}
            >
              Remove certificate
            </Button>
          )}
          {dirty && cert.trim().length > 0 && (
            <Button
              size="small"
              variant="tonal"
              disabled={busy}
              onClick={() => onSetCertificate(cert)}
            >
              {hasCert ? "Replace certificate" : "Attach certificate"}
            </Button>
          )}
        </Stack>
      )}
    </>
  );
}

/** Termius "Edit Key": label, key material (public half only — the private
 *  key stays in the vault; use Export), certificate, Key export. */
export function EditKeyPanel({
  card,
  vaultName,
  busy,
  readOnly = false,
  error,
  menu,
  onRename,
  onSetCertificate,
  onSetCertificateFile,
  onExportToHost,
  onExportPrivate,
  onChangePassphrase,
  onClose,
}: {
  card: KeyCard;
  vaultName: string;
  busy: boolean;
  /** Viewer role: label, passphrase and certificate cannot change; export still works. */
  readOnly?: boolean;
  error: string | null;
  menu: MenuAction[];
  onRename: (label: string) => void;
  onSetCertificate: (text: string | null) => void;
  onSetCertificateFile: (path: string) => void;
  onExportToHost: () => void;
  onExportPrivate: () => void;
  onChangePassphrase: () => void;
  onClose: () => void;
}) {
  const [label, setLabel] = useState(card.label);
  const commitLabel = () => {
    const l = label.trim();
    if (l.length > 0 && l !== card.label) onRename(l);
    else setLabel(card.label);
  };

  const hasCert = card.certificate !== null || card.certificateUnreadable;
  const stored = useQuery({
    queryKey: [
      "keyCertificate",
      card.id,
      card.certificate?.fingerprint ?? null,
      card.certificate?.serial ?? null,
      card.certificateUnreadable,
    ],
    queryFn: () => (hasCert ? ipc.keyCertificate(card.id) : Promise.resolve(null)),
  });
  const storedText = stored.data ?? "";

  const pickCert = async () => {
    const picked = await openFile({
      multiple: false,
      directory: false,
      title: "Certificate (*-cert.pub)",
    });
    if (typeof picked === "string") onSetCertificateFile(picked);
  };
  useFileDrop((paths) => {
    const c = paths.find((p) => droppedKind(p) === "certificate");
    if (c) onSetCertificateFile(c);
  });

  return (
    <SidePanel
      title="Edit Key"
      subtitle={vaultName}
      onClose={onClose}
      actions={<PanelMenuButton items={menu} />}
    >
      <SectionCard>
        <TextField
          value={label}
          onChange={(e) => setLabel(e.target.value)}
          onBlur={commitLabel}
          onKeyDown={(e) => {
            if (e.key === "Enter") (e.target as HTMLInputElement).blur();
          }}
          required
          placeholder="Label *"
          slotProps={{ htmlInput: { "aria-label": "Label", readOnly } }}
        />
        <Box
          sx={{
            border: 1,
            borderColor: "border.light",
            borderRadius: 1.5,
            px: 1.5,
            py: 1,
          }}
        >
          <Box sx={{ display: "flex", alignItems: "center", gap: 1 }}>
            <LockOutlinedIcon fontSize="small" sx={{ color: "text.secondary" }} />
            <Typography variant="body2" noWrap sx={{ flex: 1, minWidth: 0 }}>
              Private key
            </Typography>
            <ToolIconButton
              title="Change passphrase"
              onClick={onChangePassphrase}
              disabled={card.unreadable || readOnly}
            >
              <PasswordRoundedIcon fontSize="small" />
            </ToolIconButton>
            <ToolIconButton
              title="Export private key…"
              onClick={onExportPrivate}
              disabled={card.unreadable}
            >
              <FileDownloadOutlinedIcon fontSize="small" />
            </ToolIconButton>
          </Box>
          <Typography
            variant="caption"
            color="text.secondary"
            sx={{ display: "block", pl: 3.5 }}
            noWrap
          >
            {card.unreadable
              ? "Stored, but could not be parsed"
              : card.securityKey
                ? `${keyTypeLabel(card)} · private key stays on the security key${
                    card.securityKey.flags?.resident ? " · resident" : ""
                  }${card.securityKey.flags?.userPresence ? " · touch" : ""}${
                    card.securityKey.flags?.userVerification ? " · PIN" : ""
                  }${card.encrypted && !card.hasPassphrase ? " · asks for passphrase" : ""}`
                : `${keyTypeLabel(card)} · stored encrypted in the vault${
                    card.encrypted
                      ? card.hasPassphrase
                        ? " · passphrase remembered"
                        : " · asks for passphrase"
                      : ""
                  }`}
          </Typography>
        </Box>
        <TextField
          value={card.publicKey}
          multiline
          minRows={3}
          maxRows={6}
          placeholder="Public key"
          helperText={card.fingerprint ? <Mono>{card.fingerprint}</Mono> : undefined}
          slotProps={{
            htmlInput: { ...mono, readOnly: true, "aria-label": "Public key" },
            input: {
              endAdornment: card.publicKey ? (
                <InputAdornment position="end" sx={{ alignSelf: "flex-start", mt: 1 }}>
                  <ToolIconButton
                    title="Copy public key"
                    onClick={() => void navigator.clipboard.writeText(card.publicKey)}
                  >
                    <ContentCopyRoundedIcon fontSize="small" />
                  </ToolIconButton>
                </InputAdornment>
              ) : undefined,
            },
          }}
        />
        <StoredCertificateEditor
          key={storedText}
          card={card}
          stored={storedText}
          loading={hasCert && stored.isPending}
          busy={busy || readOnly}
          onPickFile={() => void pickCert()}
          onSetCertificate={onSetCertificate}
        />
        {error && (
          <Alert severity="error" variant="outlined">
            {error}
          </Alert>
        )}
      </SectionCard>

      <SectionCard title="Key export">
        <Button
          variant="contained"
          fullWidth
          onClick={onExportToHost}
          disabled={busy || card.unreadable}
        >
          Export to host
        </Button>
        <Typography variant="caption" color="text.secondary">
          Adds the public key to <Mono>~/.ssh/authorized_keys</Mono> on a saved host.
        </Typography>
      </SectionCard>
    </SidePanel>
  );
}

/* ---------------------------------------------------------- generate */

type Family = "ed25519" | "rsa" | "ecdsa";
const RSA_BITS = [2048, 3072, 4096] as const;
const ECDSA_BITS = [256, 384, 521] as const;

function algorithmOf(family: Family, bits: number): KeyAlgorithm {
  if (family === "ed25519") return "ed25519";
  if (family === "rsa") return { rsa: { bits } };
  return bits === 521 ? "ecdsa_p521" : bits === 384 ? "ecdsa_p384" : "ecdsa_p256";
}

export function GenerateKeyPanel({
  vaultId,
  vaultName,
  busy,
  error,
  onGenerate,
  onClose,
}: {
  vaultId: Uuid;
  vaultName: string;
  busy: boolean;
  error: string | null;
  onGenerate: (form: GenerateKeyForm) => void;
  onClose: () => void;
}) {
  const [label, setLabel] = useState("");
  const [family, setFamily] = useState<Family>("ed25519");
  const [rsaBits, setRsaBits] = useState<number>(4096);
  const [ecdsaBits, setEcdsaBits] = useState<number>(256);
  const [comment, setComment] = useState("");
  const [passphrase, setPassphrase] = useState("");
  const [confirm, setConfirm] = useState("");
  const [show, setShow] = useState(false);
  const [remember, setRemember] = useState(true);
  const mismatch = passphrase.length > 0 && passphrase !== confirm;
  const valid = label.trim().length > 0 && !mismatch;

  return (
    <SidePanel
      title="Generate Key"
      subtitle={vaultName}
      onClose={onClose}
      footer={
        <>
          <Button variant="text" color="inherit" onClick={onClose} disabled={busy}>
            Cancel
          </Button>
          <Button
            variant="contained"
            disabled={!valid || busy}
            onClick={() =>
              onGenerate({
                vaultId,
                label: label.trim(),
                algorithm: algorithmOf(family, family === "rsa" ? rsaBits : ecdsaBits),
                comment: comment.trim(),
                passphrase: passphrase.length > 0 ? passphrase : null,
                rememberPassphrase: passphrase.length > 0 && remember,
              })
            }
          >
            {busy ? "Generating…" : "Generate & Save"}
          </Button>
        </>
      }
    >
      <SectionCard>
        <TextField
          autoFocus
          value={label}
          onChange={(e) => setLabel(e.target.value)}
          required
          placeholder="Label *"
          slotProps={{ htmlInput: { "aria-label": "Label" } }}
        />
        <TextField
          type={show ? "text" : "password"}
          value={passphrase}
          onChange={(e) => setPassphrase(e.target.value)}
          placeholder="Passphrase"
          autoComplete="new-password"
          slotProps={{
            htmlInput: { "aria-label": "Passphrase" },
            input: {
              startAdornment: adornment(<LockOutlinedIcon fontSize="small" />),
              endAdornment: (
                <InputAdornment position="end">
                  <IconButton
                    size="small"
                    onClick={() => setShow((v) => !v)}
                    aria-label="Toggle passphrase visibility"
                  >
                    {show ? (
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
        {passphrase.length > 0 && (
          <>
            <TextField
              type={show ? "text" : "password"}
              value={confirm}
              onChange={(e) => setConfirm(e.target.value)}
              placeholder="Confirm passphrase"
              autoComplete="new-password"
              error={mismatch}
              helperText={mismatch ? "Passphrases differ" : undefined}
              slotProps={{
                htmlInput: { "aria-label": "Confirm passphrase" },
                input: { startAdornment: adornment(<LockOutlinedIcon fontSize="small" />) },
              }}
            />
            <FormControlLabel
              control={
                <Checkbox checked={remember} onChange={(e) => setRemember(e.target.checked)} />
              }
              label={<Typography variant="body2">Remember passphrase in the vault</Typography>}
            />
          </>
        )}
      </SectionCard>

      <SectionCard>
        <Stack direction="row" spacing={1.5}>
          <TextField
            select
            label="Key type"
            value={family}
            onChange={(e) => setFamily(e.target.value as Family)}
            sx={{ flex: 1 }}
            slotProps={{ htmlInput: { "aria-label": "Key type" } }}
          >
            <MenuItem value="ed25519">ED25519</MenuItem>
            <MenuItem value="rsa">RSA</MenuItem>
            <MenuItem value="ecdsa">ECDSA</MenuItem>
          </TextField>
          {family === "rsa" && (
            <TextField
              select
              label="Key size"
              value={rsaBits}
              onChange={(e) => setRsaBits(Number(e.target.value))}
              sx={{ width: 120 }}
            >
              {RSA_BITS.map((b) => (
                <MenuItem key={b} value={b}>
                  {b}
                </MenuItem>
              ))}
            </TextField>
          )}
          {family === "ecdsa" && (
            <TextField
              select
              label="Curve"
              value={ecdsaBits}
              onChange={(e) => setEcdsaBits(Number(e.target.value))}
              sx={{ width: 120 }}
            >
              {ECDSA_BITS.map((b) => (
                <MenuItem key={b} value={b}>
                  P-{b}
                </MenuItem>
              ))}
            </TextField>
          )}
        </Stack>
        <TextField
          value={comment}
          onChange={(e) => setComment(e.target.value)}
          placeholder="Comment"
          helperText="Appended to the public key, e.g. you@laptop"
          slotProps={{ htmlInput: { "aria-label": "Comment" } }}
        />
        {family === "ed25519" && (
          <Typography variant="caption" color="text.secondary">
            ED25519 is small, fast and the recommended default. Use RSA only for servers that do not
            accept it.
          </Typography>
        )}
        {error && (
          <Alert severity="error" variant="outlined">
            {error}
          </Alert>
        )}
      </SectionCard>
    </SidePanel>
  );
}

/* ---------------------------------------------------------- identity */

type AuthRow = "sshid" | "key" | "certificate" | "fido2";

/** Termius "New / Edit Identity": label, username, password and the auth
 *  method rows added through "+ Key, Certificate, FIDO2". */
export function IdentityPanel({
  vaultId,
  vaultName,
  initial,
  keys,
  busy,
  readOnly = false,
  error,
  menu,
  onSave,
  onNewKey,
  onNewFido2,
  onClose,
}: {
  vaultId: Uuid;
  vaultName: string;
  initial: IdentityCard | null;
  keys: KeyCard[];
  busy: boolean;
  readOnly?: boolean;
  error: string | null;
  menu: MenuAction[];
  onSave: (form: IdentityForm) => void;
  onNewKey: () => void;
  onNewFido2: () => void;
  onClose: () => void;
}) {
  const [label, setLabel] = useState(initial?.label ?? "");
  const [username, setUsername] = useState(initial?.username ?? "");
  const [password, setPassword] = useState<string | null>(null);
  const [showPassword, setShowPassword] = useState(false);
  const [keyId, setKeyId] = useState<Uuid | null>(initial?.sshKeyId ?? null);
  const [certId, setCertId] = useState<Uuid | null>(initial?.sshCertificateId ?? null);
  const [sshId, setSshId] = useState(initial?.sshId ?? false);
  const [sshIdType, setSshIdType] = useState<SshIdKeyType | null>(initial?.sshIdKeyType ?? null);
  const [rows, setRows] = useState<AuthRow[]>(() => {
    const r: AuthRow[] = [];
    if (initial?.sshId) r.push("sshid");
    if (initial?.sshKeyId) {
      const k = keys.find((x) => x.id === initial.sshKeyId);
      r.push(k && isHardwareKey(k) ? "fido2" : "key");
    }
    if (initial?.sshCertificateId) r.push("certificate");
    return r;
  });
  const [addAnchor, setAddAnchor] = useState<HTMLElement | null>(null);

  const certified = useMemo(() => keys.filter((k) => k.certificate !== null), [keys]);
  const softwareKeys = useMemo(() => keys.filter((k) => !isHardwareKey(k)), [keys]);
  const hardwareKeys = useMemo(() => keys.filter(isHardwareKey), [keys]);
  const keyById = useMemo(() => new Map(keys.map((k) => [k.id, k])), [keys]);
  const selectedKey = keyId ? (keyById.get(keyId) ?? null) : null;
  const certKey = certId ? (certified.find((k) => k.certificate?.id === certId) ?? null) : null;
  const keyCertId = selectedKey?.certificate?.id ?? null;

  const addRow = (r: AuthRow) => {
    setRows((rs) => {
      // Key and FIDO2 both fill `sshKeyId`; adding one drops the other.
      const other: AuthRow | null = r === "key" ? "fido2" : r === "fido2" ? "key" : null;
      const base = other ? rs.filter((x) => x !== other) : rs;
      return base.includes(r) ? base : [...base, r];
    });
    if (r === "key" || r === "fido2") {
      setKeyId(null);
      setCertId(null);
      setRows((rs) => rs.filter((x) => x !== "certificate"));
    }
    if (r === "sshid") setSshId(true);
    setAddAnchor(null);
  };
  const removeRow = (r: AuthRow) => {
    setRows((rs) => rs.filter((x) => x !== r));
    if (r === "key" || r === "fido2") {
      setKeyId(null);
      setCertId(null);
      setRows((rs) => rs.filter((x) => x !== "certificate"));
    }
    if (r === "certificate") setCertId(null);
    if (r === "sshid") {
      setSshId(false);
      setSshIdType(null);
    }
  };
  const chooseKey = (id: Uuid | null) => {
    setKeyId(id);
    const k = id ? keyById.get(id) : undefined;
    if (certId && k?.certificate?.id !== certId) setCertId(null);
  };
  const chooseCert = (id: Uuid | null) => {
    setCertId(id);
    const k = id ? certified.find((c) => c.certificate?.id === id) : undefined;
    if (k) {
      setKeyId(k.id);
      setRows((rs) => (rs.includes("key") ? rs : [...rs, "key"]));
    }
  };

  const missing = readOnly
    ? []
    : (["sshid", "key", "certificate", "fido2"] as AuthRow[]).filter(
        (r) =>
          !rows.includes(r) &&
          // one of Key / FIDO2 at a time
          !(r === "key" && rows.includes("fido2")) &&
          !(r === "fido2" && rows.includes("key")),
      );
  const valid = label.trim().length > 0 && (sshId || username.trim().length > 0);
  const lock = { readOnly, disabled: readOnly };

  const rowTitle: Record<AuthRow, string> = {
    sshid: "SSH ID",
    key: "Key",
    certificate: "Certificate",
    fido2: "FIDO2",
  };
  const rowIcon: Record<AuthRow, ReactNode> = {
    sshid: <FingerprintRoundedIcon fontSize="small" />,
    key: <KeyOutlinedIcon fontSize="small" />,
    certificate: <WorkspacePremiumOutlinedIcon fontSize="small" />,
    fido2: <UsbRoundedIcon fontSize="small" />,
  };

  return (
    <SidePanel
      title={readOnly ? "Identity" : initial ? "Edit Identity" : "New Identity"}
      subtitle={vaultName}
      onClose={onClose}
      actions={<PanelMenuButton items={menu} />}
      footer={
        <>
          <Button variant="text" color="inherit" onClick={onClose} disabled={busy}>
            {readOnly ? "Close" : "Cancel"}
          </Button>
          <Button
            variant="contained"
            disabled={!valid || busy || readOnly}
            onClick={() =>
              onSave({
                id: initial?.id ?? null,
                vaultId,
                label: label.trim(),
                username: username.trim(),
                password,
                sshKeyId: keyId,
                sshCertificateId: certId,
                sshId,
                sshIdKeyType: sshId ? sshIdType : null,
              })
            }
          >
            {busy && !readOnly ? "Saving…" : "Save"}
          </Button>
        </>
      }
    >
      <SectionCard>
        <Stack direction="row" spacing={1} sx={{ alignItems: "center" }}>
          <IdentityTile />
          <TextField
            autoFocus={!readOnly}
            value={label}
            onChange={(e) => setLabel(e.target.value)}
            placeholder="Label"
            sx={{ flex: 1 }}
            slotProps={{ htmlInput: { "aria-label": "Label", readOnly } }}
          />
        </Stack>
        <TextField
          value={username}
          onChange={(e) => setUsername(e.target.value)}
          autoComplete="off"
          required={!sshId}
          placeholder={sshId ? "Username (defaults to your SSH ID handle)" : "Username *"}
          slotProps={{
            htmlInput: { "aria-label": "Username", readOnly },
            input: { startAdornment: adornment(<PersonOutlineRoundedIcon fontSize="small" />) },
          }}
        />
        <TextField
          type={showPassword ? "text" : "password"}
          value={password ?? ""}
          onChange={(e) => setPassword(e.target.value)}
          autoComplete="new-password"
          placeholder={initial?.hasPassword && password === null ? "••••••••••••" : "Password"}
          helperText={
            initial?.hasPassword && password === null
              ? readOnly
                ? "A password is stored."
                : "A password is stored. Type to replace it or clear it to remove."
              : undefined
          }
          slotProps={{
            htmlInput: { "aria-label": "Password", readOnly },
            input: {
              startAdornment: adornment(<PasswordRoundedIcon fontSize="small" />),
              endAdornment: (
                <InputAdornment position="end">
                  {initial?.hasPassword && password === null && !readOnly && (
                    <Button size="small" color="inherit" onClick={() => setPassword("")}>
                      Clear
                    </Button>
                  )}
                  <IconButton
                    size="small"
                    onClick={() => setShowPassword((v) => !v)}
                    aria-label="Toggle password visibility"
                    disabled={readOnly}
                  >
                    {showPassword ? (
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

        {rows.includes("sshid") && (
          <TextField
            select
            {...lock}
            value={sshIdType ?? ""}
            onChange={(e) =>
              setSshIdType(e.target.value === "" ? null : (e.target.value as SshIdKeyType))
            }
            helperText="Signs in with this account's passkeys (Settings → SSH ID). Pick which one to offer first."
            slotProps={{
              htmlInput: { "aria-label": "SSH ID key type" },
              input: {
                startAdornment: adornment(rowIcon.sshid),
                endAdornment: (
                  <InputAdornment position="end" sx={{ mr: 2 }}>
                    <IconButton
                      size="small"
                      aria-label="Remove SSH ID"
                      onClick={() => removeRow("sshid")}
                      disabled={readOnly}
                    >
                      <CloseRoundedIcon fontSize="small" />
                    </IconButton>
                  </InputAdornment>
                ),
              },
            }}
          >
            <MenuItem value="">
              {sshIdTypeLabel(SSH_ID_DEFAULT_TYPE)}
              <Typography component="span" variant="caption" color="text.secondary" sx={{ ml: 1 }}>
                Default
              </Typography>
            </MenuItem>
            {SSH_ID_KEY_TYPES.filter((t) => t.value !== SSH_ID_DEFAULT_TYPE).map((t) => (
              <MenuItem key={t.value} value={t.value}>
                {t.label}
                <Typography
                  component="span"
                  variant="caption"
                  color="text.secondary"
                  sx={{ ml: 1 }}
                >
                  {t.hint}
                </Typography>
              </MenuItem>
            ))}
          </TextField>
        )}

        {rows.includes("key") && (
          <TextField
            select
            {...lock}
            value={keyId ?? ""}
            onChange={(e) => {
              const v = e.target.value;
              if (v === "__new") {
                onNewKey();
                return;
              }
              chooseKey(v === "" ? null : v);
            }}
            helperText={
              selectedKey
                ? keyCertId && !certId
                  ? "This key carries a certificate; it is used automatically."
                  : keyTypeLabel(selectedKey)
                : undefined
            }
            slotProps={{
              htmlInput: { "aria-label": "Key" },
              input: {
                startAdornment: adornment(rowIcon.key),
                endAdornment: (
                  <InputAdornment position="end" sx={{ mr: 2 }}>
                    <IconButton
                      size="small"
                      aria-label="Remove key"
                      onClick={() => removeRow("key")}
                      disabled={readOnly}
                    >
                      <CloseRoundedIcon fontSize="small" />
                    </IconButton>
                  </InputAdornment>
                ),
              },
            }}
          >
            <MenuItem value="">
              <em>Choose a key</em>
            </MenuItem>
            {softwareKeys.map((k) => (
              <MenuItem key={k.id} value={k.id}>
                {k.label}
                <Typography
                  component="span"
                  variant="caption"
                  color="text.secondary"
                  sx={{ ml: 1 }}
                >
                  {keyTypeLabel(k).replace(/^Type /, "")}
                  {k.certificate ? " · cert" : ""}
                </Typography>
              </MenuItem>
            ))}
            <MenuItem value="__new" sx={{ color: "primary.main" }}>
              <AddRoundedIcon fontSize="small" sx={{ mr: 1.25 }} />
              New key…
            </MenuItem>
          </TextField>
        )}

        {rows.includes("certificate") && (
          <TextField
            select
            {...lock}
            value={certId ?? ""}
            onChange={(e) => chooseCert(e.target.value === "" ? null : e.target.value)}
            helperText={
              certKey?.certificate
                ? certificateSummary(certKey.certificate)
                : certified.length === 0
                  ? "No key has a certificate yet — attach one in Edit Key."
                  : "Selecting a certificate also selects its key."
            }
            slotProps={{
              htmlInput: { "aria-label": "Certificate" },
              input: {
                startAdornment: adornment(rowIcon.certificate),
                endAdornment: (
                  <InputAdornment position="end" sx={{ mr: 2 }}>
                    <IconButton
                      size="small"
                      aria-label="Remove certificate"
                      onClick={() => removeRow("certificate")}
                      disabled={readOnly}
                    >
                      <CloseRoundedIcon fontSize="small" />
                    </IconButton>
                  </InputAdornment>
                ),
              },
            }}
          >
            <MenuItem value="">
              <em>Choose a certificate</em>
            </MenuItem>
            {certified.map((k) => (
              <MenuItem key={k.id} value={k.certificate?.id ?? ""}>
                {k.label}
                <Typography
                  component="span"
                  variant="caption"
                  color="text.secondary"
                  sx={{ ml: 1 }}
                >
                  {k.certificate ? certificateName(k.certificate) : null}
                </Typography>
              </MenuItem>
            ))}
          </TextField>
        )}

        {rows.includes("fido2") && (
          <TextField
            select
            {...lock}
            value={keyId ?? ""}
            onChange={(e) => {
              const v = e.target.value;
              if (v === "__new") {
                onNewFido2();
                return;
              }
              chooseKey(v === "" ? null : v);
            }}
            helperText={
              selectedKey
                ? "The token must be plugged in to connect; you will be asked to touch it."
                : hardwareKeys.length === 0
                  ? "No FIDO2 key in this vault yet — generate one on your security key."
                  : undefined
            }
            slotProps={{
              htmlInput: { "aria-label": "FIDO2 key" },
              input: {
                startAdornment: adornment(rowIcon.fido2),
                endAdornment: (
                  <InputAdornment position="end" sx={{ mr: 2 }}>
                    <IconButton
                      size="small"
                      aria-label="Remove FIDO2"
                      onClick={() => removeRow("fido2")}
                      disabled={readOnly}
                    >
                      <CloseRoundedIcon fontSize="small" />
                    </IconButton>
                  </InputAdornment>
                ),
              },
            }}
          >
            <MenuItem value="">
              <em>Choose a FIDO2 key</em>
            </MenuItem>
            {hardwareKeys.map((k) => (
              <MenuItem key={k.id} value={k.id}>
                {k.label}
                <Typography
                  component="span"
                  variant="caption"
                  color="text.secondary"
                  sx={{ ml: 1 }}
                >
                  {keyTypeLabel(k).replace(/^Type /, "")}
                </Typography>
              </MenuItem>
            ))}
            <MenuItem value="__new" sx={{ color: "primary.main" }}>
              <AddRoundedIcon fontSize="small" sx={{ mr: 1.25 }} />
              Generate FIDO2 key…
            </MenuItem>
          </TextField>
        )}

        {missing.length > 0 && (
          <>
            <Button
              variant="text"
              color="inherit"
              size="small"
              startIcon={<AddRoundedIcon />}
              onClick={(e) => setAddAnchor(e.currentTarget)}
              sx={{ alignSelf: "flex-start", color: "text.secondary", ml: -0.5 }}
            >
              {missing.map((r) => rowTitle[r]).join(", ")}
            </Button>
            <Menu anchorEl={addAnchor} open={Boolean(addAnchor)} onClose={() => setAddAnchor(null)}>
              {missing.map((r) => (
                <MenuItem key={r} onClick={() => addRow(r)}>
                  <Box sx={{ display: "flex", alignItems: "center", gap: 1.25 }}>
                    {rowIcon[r]}
                    {rowTitle[r]}
                  </Box>
                </MenuItem>
              ))}
            </Menu>
          </>
        )}
        {error && (
          <Alert severity="error" variant="outlined">
            {error}
          </Alert>
        )}
      </SectionCard>
    </SidePanel>
  );
}

/* -------------------------------------------------------------- fido2 */

const SK_ALGORITHMS: { value: SkAlgorithm; label: string; hint: string }[] = [
  { value: "ed25519", label: "ED25519-SK", hint: "sk-ssh-ed25519@openssh.com" },
  { value: "ecdsa_p256", label: "ECDSA-SK", hint: "sk-ecdsa-sha2-nistp256@openssh.com" },
];

function deviceName(d: Fido2Device): string {
  return d.product.trim() || `USB ${d.vendorId.toString(16)}:${d.productId.toString(16)}`;
}

/** Generate a key on a FIDO2 security key. Polls for tokens every 2 s while
 *  open; the private key never leaves the token — the vault stores the
 *  public half plus the credential handle (like `~/.ssh/id_ed25519_sk`). */
export function Fido2Panel({
  vaultId,
  vaultName,
  busy,
  error,
  onGenerate,
  onLoadResident,
  onClose,
}: {
  vaultId: Uuid;
  vaultName: string;
  busy: boolean;
  error: string | null;
  onGenerate: (form: Fido2GenerateForm) => void;
  onLoadResident: (form: Fido2LoadForm) => void;
  onClose: () => void;
}) {
  const devices = useQuery({
    queryKey: ["fido2", "devices"],
    queryFn: ipc.fido2Devices,
    refetchInterval: busy ? false : 2000,
    refetchIntervalInBackground: false,
  });
  const list = devices.data ?? [];
  const [devicePath, setDevicePath] = useState<string | null>(null);
  const device = list.find((d) => d.path === devicePath) ?? list[0] ?? null;

  const [label, setLabel] = useState("");
  const [algorithmPref, setAlgorithm] = useState<SkAlgorithm>("ed25519");
  const [resident, setResident] = useState(false);
  const [userPresence, setUserPresence] = useState(true);
  const [userVerification, setUserVerification] = useState(false);
  const [pin, setPin] = useState("");
  const [showPin, setShowPin] = useState(false);
  const [passphrase, setPassphrase] = useState("");
  const [confirm, setConfirm] = useState("");
  const [show, setShow] = useState(false);
  const [remember, setRemember] = useState(true);
  const [mode, setMode] = useState<"generate" | "load">("generate");

  const supported = device?.algorithms ?? [];
  const algorithm: SkAlgorithm =
    supported.length === 0 || supported.includes(algorithmPref)
      ? algorithmPref
      : (supported[0] ?? algorithmPref);
  const canResident = device?.residentKeys ?? false;
  const pinNeeded = device?.pinSet === true || resident || userVerification || mode === "load";
  const mismatch = passphrase.length > 0 && passphrase !== confirm;
  const valid =
    device !== null &&
    !mismatch &&
    (!pinNeeded || pin.length >= 4) &&
    (mode === "load" || label.trim().length > 0);

  const pinField = (
    <TextField
      type={showPin ? "text" : "password"}
      value={pin}
      onChange={(e) => setPin(e.target.value)}
      placeholder="PIN"
      autoComplete="off"
      inputMode="numeric"
      helperText={
        device?.pinSet === false
          ? "This token has no PIN yet; set one with your vendor tool or ssh-keygen -O verify-required."
          : undefined
      }
      slotProps={{
        htmlInput: { "aria-label": "PIN" },
        input: {
          startAdornment: adornment(<PasswordRoundedIcon fontSize="small" />),
          endAdornment: (
            <InputAdornment position="end">
              <IconButton
                size="small"
                onClick={() => setShowPin((v) => !v)}
                aria-label="Toggle PIN visibility"
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
  );

  const passphraseFields = (
    <>
      <TextField
        type={show ? "text" : "password"}
        value={passphrase}
        onChange={(e) => setPassphrase(e.target.value)}
        placeholder="Passphrase"
        autoComplete="new-password"
        helperText="Protects the stored key handle, like OpenSSH does. Optional."
        slotProps={{
          htmlInput: { "aria-label": "Passphrase" },
          input: {
            startAdornment: adornment(<LockOutlinedIcon fontSize="small" />),
            endAdornment: (
              <InputAdornment position="end">
                <IconButton
                  size="small"
                  onClick={() => setShow((v) => !v)}
                  aria-label="Toggle passphrase visibility"
                >
                  {show ? (
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
      {passphrase.length > 0 && (
        <>
          <TextField
            type={show ? "text" : "password"}
            value={confirm}
            onChange={(e) => setConfirm(e.target.value)}
            placeholder="Confirm passphrase"
            autoComplete="new-password"
            error={mismatch}
            helperText={mismatch ? "Passphrases differ" : undefined}
            slotProps={{
              htmlInput: { "aria-label": "Confirm passphrase" },
              input: { startAdornment: adornment(<LockOutlinedIcon fontSize="small" />) },
            }}
          />
          <FormControlLabel
            control={
              <Checkbox checked={remember} onChange={(e) => setRemember(e.target.checked)} />
            }
            label={<Typography variant="body2">Save passphrase in the vault</Typography>}
          />
        </>
      )}
    </>
  );

  const submit = () => {
    if (!device) return;
    const pass = passphrase.length > 0 ? passphrase : null;
    if (mode === "load") {
      onLoadResident({
        vaultId,
        device: device.path,
        pin,
        passphrase: pass,
        rememberPassphrase: pass !== null && remember,
      });
      return;
    }
    onGenerate({
      vaultId,
      label: label.trim(),
      device: device.path,
      algorithm,
      resident,
      userPresence,
      userVerification,
      pin: pin.length > 0 ? pin : null,
      user: null,
      comment: "",
      passphrase: pass,
      rememberPassphrase: pass !== null && remember,
    });
  };

  return (
    <SidePanel
      title={mode === "load" ? "Load FIDO2 Keys" : "Generate FIDO2 Key"}
      subtitle={vaultName}
      onClose={onClose}
      footer={
        device ? (
          <>
            <Button
              variant="text"
              color="inherit"
              onClick={() => (mode === "load" ? setMode("generate") : onClose())}
              disabled={busy}
            >
              {mode === "load" ? "Back" : "Cancel"}
            </Button>
            <Button variant="contained" disabled={!valid || busy} onClick={submit}>
              {busy ? "Touch your security key…" : mode === "load" ? "Load" : "Generate"}
            </Button>
          </>
        ) : undefined
      }
    >
      {!device ? (
        <Box sx={{ flex: 1, display: "flex", alignItems: "center", justifyContent: "center" }}>
          <Box sx={{ textAlign: "center", color: "text.secondary", py: 6 }}>
            <IconTile size={56} tone="neutral" sx={{ mx: "auto" }}>
              <UsbRoundedIcon />
            </IconTile>
            <Typography variant="subtitle1" color="text.primary" sx={{ mt: 2 }}>
              Insert FIDO2 device
            </Typography>
            <Typography variant="body2" sx={{ mt: 0.5 }}>
              {devices.isError
                ? errorMessage(devices.error)
                : "Connect your FIDO2 device to show here."}
            </Typography>
          </Box>
        </Box>
      ) : (
        <>
          <SectionCard>
            <TextField
              select
              label="Security key"
              value={device.path}
              onChange={(e) => setDevicePath(e.target.value)}
              slotProps={{ htmlInput: { "aria-label": "Security key" } }}
            >
              {list.map((d) => (
                <MenuItem key={d.path} value={d.path}>
                  {deviceName(d)}
                </MenuItem>
              ))}
            </TextField>
            <Stack direction="row" spacing={0.75} useFlexGap sx={{ flexWrap: "wrap" }}>
              {device.versions.map((v) => (
                <Chip key={v} size="small" variant="outlined" label={v} />
              ))}
              {device.pinSet === true && <Chip size="small" variant="outlined" label="PIN set" />}
              {device.residentKeys && (
                <Chip size="small" variant="outlined" label="Resident keys" />
              )}
            </Stack>
            {mode === "generate" && canResident && (
              <Button
                variant="text"
                size="small"
                sx={{ alignSelf: "flex-start" }}
                onClick={() => setMode("load")}
              >
                Load resident keys from this device…
              </Button>
            )}
          </SectionCard>

          {mode === "generate" ? (
            <>
              <SectionCard>
                <TextField
                  autoFocus
                  value={label}
                  onChange={(e) => setLabel(e.target.value)}
                  required
                  placeholder="Label *"
                  slotProps={{ htmlInput: { "aria-label": "Label" } }}
                />
                <TextField
                  select
                  label="Key type"
                  value={algorithm}
                  onChange={(e) => setAlgorithm(e.target.value as SkAlgorithm)}
                  slotProps={{ htmlInput: { "aria-label": "Key type" } }}
                >
                  {SK_ALGORITHMS.map((a) => (
                    <MenuItem
                      key={a.value}
                      value={a.value}
                      disabled={supported.length > 0 && !supported.includes(a.value)}
                    >
                      <ListItemText primary={a.label} secondary={a.hint} />
                    </MenuItem>
                  ))}
                </TextField>
                <SettingRow
                  label="Require User Presence"
                  hint="Touch the key for every connection"
                  control={
                    <Switch
                      checked={userPresence}
                      onChange={(e) => setUserPresence(e.target.checked)}
                      slotProps={{ input: { "aria-label": "Require User Presence" } }}
                    />
                  }
                />
                <SettingRow
                  label="Require PIN Code"
                  hint="Ask for the PIN for every connection"
                  control={
                    <Switch
                      checked={userVerification}
                      onChange={(e) => setUserVerification(e.target.checked)}
                      slotProps={{ input: { "aria-label": "Require PIN Code" } }}
                    />
                  }
                />
                <SettingRow
                  label="Resident key"
                  hint={
                    canResident
                      ? "Store the credential on the token so it can be loaded on another machine"
                      : "This token cannot store resident credentials"
                  }
                  last
                  control={
                    <Switch
                      checked={resident && canResident}
                      disabled={!canResident}
                      onChange={(e) => setResident(e.target.checked)}
                      slotProps={{ input: { "aria-label": "Resident key" } }}
                    />
                  }
                />
                {pinNeeded && pinField}
              </SectionCard>
              <SectionCard>{passphraseFields}</SectionCard>
            </>
          ) : (
            <SectionCard>
              <Typography variant="body2" color="text.secondary">
                Resident SSH credentials on this token are imported into <b>{vaultName}</b>. The PIN
                is required to list them.
              </Typography>
              {pinField}
              {passphraseFields}
            </SectionCard>
          )}
          {error && (
            <Alert severity="error" variant="outlined">
              {error}
            </Alert>
          )}
        </>
      )}
    </SidePanel>
  );
}

export { BadgeOutlinedIcon as IdentityIcon };
