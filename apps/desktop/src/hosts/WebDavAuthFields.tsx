import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import {
  Box,
  Button,
  Divider,
  IconButton,
  InputAdornment,
  Stack,
  TextField,
  ToggleButton,
  ToggleButtonGroup,
  Typography,
} from "@mui/material";
import KeyRoundedIcon from "@mui/icons-material/KeyRounded";
import VisibilityRoundedIcon from "@mui/icons-material/VisibilityRounded";
import VisibilityOffRoundedIcon from "@mui/icons-material/VisibilityOffRounded";
import VerifiedUserOutlinedIcon from "@mui/icons-material/VerifiedUserOutlined";
import FileOpenOutlinedIcon from "@mui/icons-material/FileOpenOutlined";
import { open as openFile } from "@tauri-apps/plugin-dialog";
import { Field } from "@/components/ui";
import { useSnackbar } from "@/components/Snackbar";
import { webdavClientIdentityInspect, webdavPemFile } from "@/ipc/commands";
import { errorMessage, type Uuid, type WebDavAuth, type WebDavForm } from "@/ipc/types";
import { monoFontFamily } from "@/theme/theme";
import { CredentialsFields } from "./CredentialsFields";
import { tr, msg } from "@/i18n";

const AUTH_MODES: { value: WebDavAuth; label: string }[] = [
  { value: "password", label: msg("Password") },
  { value: "token", label: msg("Token") },
];

const pemSx = { fontFamily: monoFontFamily, fontSize: 12 } as const;

/**
 * Authentication block of the WebDAV section: password (Basic / Digest) or
 * a bearer token, plus an optional client certificate for mutual TLS. Saved
 * secrets are never echoed back; `null` in the form keeps them, `""` clears.
 */
export function WebDavAuthFields({
  vaultId,
  value,
  onChange,
}: {
  vaultId: Uuid;
  value: WebDavForm;
  onChange: (patch: Partial<WebDavForm>) => void;
}) {
  const snack = useSnackbar();
  const [showToken, setShowToken] = useState(false);
  const [replacingCert, setReplacingCert] = useState(false);

  const storedCert =
    value.clientCertificateFingerprint !== null &&
    value.clientCertificate === null &&
    !replacingCert;
  const editingCert = !storedCert;
  const cert = value.clientCertificate ?? "";
  const key = value.clientKey ?? "";

  const pemCheck = useQuery({
    queryKey: ["webdavClientIdentity", cert, key],
    queryFn: () => webdavClientIdentityInspect(cert, key),
    enabled: cert.trim().length > 0 && key.trim().length > 0,
    retry: false,
    staleTime: Infinity,
  });
  const pemError = pemCheck.isError ? errorMessage(pemCheck.error) : undefined;

  const pickPem = async () => {
    const picked = await openFile({
      multiple: false,
      directory: false,
      title: tr("Client certificate / key (PEM)"),
      filters: [{ name: "PEM", extensions: ["pem", "crt", "cer", "key"] }],
    });
    if (typeof picked !== "string") return;
    try {
      const parts = await webdavPemFile(picked);
      const p: Partial<WebDavForm> = {};
      if (parts.certificate) p.clientCertificate = parts.certificate;
      if (parts.privateKey) p.clientKey = parts.privateKey;
      onChange(p);
      setReplacingCert(true);
    } catch (e) {
      snack.error(errorMessage(e));
    }
  };

  const tokenStored = value.hasBearerToken && value.bearerToken === null;

  return (
    <>
      <Field label={tr("Authentication")}>
        <ToggleButtonGroup
          exclusive
          size="small"
          value={value.auth}
          onChange={(_, v: WebDavAuth | null) => v && onChange({ auth: v })}
          aria-label={tr("Authentication")}
        >
          {AUTH_MODES.map((o) => (
            <ToggleButton key={o.value} value={o.value} sx={{ px: 1.5 }}>
              {tr(o.label)}
            </ToggleButton>
          ))}
        </ToggleButtonGroup>
      </Field>

      {value.auth === "password" ? (
        <CredentialsFields
          vaultId={vaultId}
          ssh={false}
          inlineLabel={tr("Set on this host")}
          value={{
            identityId: value.identityId,
            username: value.username,
            password: value.password,
            hasPassword: value.hasPassword,
            sshKeyId: null,
            sshCertificateId: null,
            sshId: false,
            sshIdKeyType: null,
            agentForwarding: false,
            forwardX11: false,
          }}
          onChange={({ identityId, username, password }) => {
            const p: Partial<WebDavForm> = {};
            if (identityId !== undefined) p.identityId = identityId;
            if (username !== undefined) p.username = username;
            if (password !== undefined) p.password = password;
            onChange(p);
          }}
        />
      ) : (
        <Field
          label={tr("Bearer token")}
          hint={tr(
            "Sent as Authorization: Bearer on every request; stored encrypted in the vault.",
          )}
        >
          <TextField
            type={showToken ? "text" : "password"}
            value={value.bearerToken ?? ""}
            onChange={(e) => onChange({ bearerToken: e.target.value })}
            autoComplete="off"
            placeholder={tokenStored ? "••••••••••••" : tr("Token")}
            helperText={
              tokenStored
                ? tr("A token is stored. Type to replace it or clear it to remove.")
                : undefined
            }
            slotProps={{
              htmlInput: { "aria-label": tr("Bearer token"), sx: pemSx },
              input: {
                startAdornment: (
                  <InputAdornment position="start">
                    <KeyRoundedIcon fontSize="small" />
                  </InputAdornment>
                ),
                endAdornment: (
                  <InputAdornment position="end">
                    {tokenStored && (
                      <Button
                        size="small"
                        color="inherit"
                        onClick={() => onChange({ bearerToken: "" })}
                      >
                        {tr("Clear")}
                      </Button>
                    )}
                    <IconButton
                      size="small"
                      onClick={() => setShowToken((v) => !v)}
                      aria-label={tr("Toggle token visibility")}
                    >
                      {showToken ? (
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
        </Field>
      )}

      <Divider />

      <Field
        label={tr("Client certificate")}
        hint={tr(
          "Optional mutual TLS: a PEM certificate (chain, leaf first) and its unencrypted private key. Both are stored encrypted in the vault.",
        )}
      >
        {storedCert ? (
          <Stack direction="row" spacing={1} sx={{ alignItems: "center", minWidth: 0 }}>
            <VerifiedUserOutlinedIcon fontSize="small" color="success" />
            <Typography
              variant="body2"
              sx={{ fontFamily: monoFontFamily, flex: 1, minWidth: 0, wordBreak: "break-all" }}
            >
              {value.clientCertificateFingerprint}
            </Typography>
            <Button size="small" onClick={() => setReplacingCert(true)}>
              {tr("Replace")}
            </Button>
            <Button
              size="small"
              color="error"
              onClick={() => onChange({ clientCertificate: "", clientKey: "" })}
            >
              {tr("Remove")}
            </Button>
          </Stack>
        ) : (
          <Stack spacing={1}>
            <Box>
              <Button
                size="small"
                variant="outlined"
                startIcon={<FileOpenOutlinedIcon />}
                onClick={() => void pickPem()}
              >
                {tr("Open PEM file…")}
              </Button>
            </Box>
            <TextField
              multiline
              minRows={3}
              maxRows={6}
              value={cert}
              onChange={(e) => onChange({ clientCertificate: e.target.value })}
              placeholder={"-----BEGIN CERTIFICATE-----\n…\n-----END CERTIFICATE-----"}
              helperText={
                value.clientCertificate === "" && value.clientCertificateFingerprint !== null
                  ? tr("The stored certificate will be removed on save.")
                  : undefined
              }
              slotProps={{
                htmlInput: {
                  "aria-label": tr("Client certificate"),
                  spellCheck: false,
                  sx: pemSx,
                },
              }}
            />
            <TextField
              multiline
              minRows={3}
              maxRows={6}
              value={key}
              onChange={(e) => onChange({ clientKey: e.target.value })}
              placeholder={
                value.clientCertificateFingerprint !== null && value.clientCertificate !== ""
                  ? tr("Leave empty to keep the stored private key")
                  : "-----BEGIN PRIVATE KEY-----\n…\n-----END PRIVATE KEY-----"
              }
              error={pemError !== undefined}
              helperText={
                pemError ??
                (pemCheck.data ? tr("Certificate {data}", { data: pemCheck.data }) : undefined)
              }
              slotProps={{
                htmlInput: {
                  "aria-label": tr("Client private key"),
                  spellCheck: false,
                  sx: pemSx,
                },
              }}
            />
            {(editingCert && value.clientCertificateFingerprint !== null) ||
            cert.length > 0 ||
            key.length > 0 ? (
              <Box>
                <Button
                  size="small"
                  color="inherit"
                  onClick={() => {
                    onChange({ clientCertificate: null, clientKey: null });
                    setReplacingCert(false);
                  }}
                >
                  {value.clientCertificateFingerprint !== null
                    ? tr("Keep stored certificate")
                    : tr("Clear")}
                </Button>
              </Box>
            ) : null}
          </Stack>
        )}
      </Field>
    </>
  );
}
