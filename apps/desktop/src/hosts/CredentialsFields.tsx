import { useState } from "react";
import {
  Button,
  IconButton,
  InputAdornment,
  MenuItem,
  Switch,
  TextField,
  Typography,
} from "@mui/material";
import VisibilityRoundedIcon from "@mui/icons-material/VisibilityRounded";
import VisibilityOffRoundedIcon from "@mui/icons-material/VisibilityOffRounded";
import { Field, SettingRow } from "@/components/ui";
import { useIdentities, useSshKeys } from "@/ipc/hooks";
import type { Inherited, Uuid } from "@/ipc/types";

/** The credential slice shared by the host and group editors. */
export interface CredentialValues {
  identityId: Uuid | null;
  username: string;
  /** `null` = keep the stored password (see `hasPassword`). */
  password: string | null;
  hasPassword: boolean;
  sshKeyId: Uuid | null;
  agentForwarding: boolean;
}

export function CredentialsFields({
  vaultId,
  value,
  onChange,
  ssh,
  inherited,
  inlineLabel,
}: {
  vaultId: Uuid;
  value: CredentialValues;
  onChange: (patch: Partial<CredentialValues>) => void;
  ssh: boolean;
  /** Effective values coming from the group chain, shown as placeholders. */
  inherited?: Inherited | null;
  inlineLabel: string;
}) {
  const identities = useIdentities(vaultId);
  const sshKeys = useSshKeys(vaultId);
  const [showPassword, setShowPassword] = useState(false);

  const usingIdentity = value.identityId !== null;
  const identityKnown =
    value.identityId === null || (identities.data ?? []).some((i) => i.id === value.identityId);
  const from = inherited && inherited.groupPath.length > 0 ? inherited.groupPath.join(" / ") : null;
  const inheritedKey = inherited?.sshKeyLabel ?? null;
  const inheritedIdentity = inherited?.identityLabel ?? null;

  return (
    <>
      <Field
        label="Use"
        hint={
          !usingIdentity && inheritedIdentity && from
            ? `Empty fields fall back to “${inheritedIdentity}” from ${from}.`
            : undefined
        }
      >
        <TextField
          select
          value={value.identityId ?? "inline"}
          onChange={(e) => {
            const v = e.target.value;
            onChange({ identityId: v === "inline" ? null : v });
          }}
        >
          <MenuItem value="inline">{inlineLabel}</MenuItem>
          {!identityKnown && value.identityId && (
            <MenuItem value={value.identityId}>
              <em>{identities.data ? "Unknown identity" : "Loading…"}</em>
            </MenuItem>
          )}
          {(identities.data ?? []).map((i) => (
            <MenuItem key={i.id} value={i.id}>
              {i.label}
              <Typography component="span" variant="caption" color="text.secondary" sx={{ ml: 1 }}>
                {i.username}
              </Typography>
            </MenuItem>
          ))}
        </TextField>
      </Field>
      {!usingIdentity && (
        <>
          <Field
            label="Username"
            hint={
              !value.username && inherited?.username && from ? `Inherited from ${from}` : undefined
            }
          >
            <TextField
              value={value.username}
              onChange={(e) => onChange({ username: e.target.value })}
              autoComplete="off"
              placeholder={inherited?.username ?? "root"}
            />
          </Field>
          <Field
            label="Password"
            hint={
              value.hasPassword && value.password === null
                ? "A password is stored. Type to replace it or clear it to remove."
                : !value.hasPassword && value.password === null && inherited?.hasPassword && from
                  ? `Inherited from ${from}`
                  : undefined
            }
          >
            <TextField
              type={showPassword ? "text" : "password"}
              value={value.password ?? ""}
              onChange={(e) => onChange({ password: e.target.value })}
              autoComplete="new-password"
              placeholder={
                value.hasPassword && value.password === null
                  ? "••••••••"
                  : inherited?.hasPassword
                    ? "•••••••• (inherited)"
                    : ""
              }
              slotProps={{
                input: {
                  endAdornment: (
                    <InputAdornment position="end">
                      {value.hasPassword && value.password === null && (
                        <Button
                          size="small"
                          color="inherit"
                          onClick={() => onChange({ password: "" })}
                        >
                          Clear
                        </Button>
                      )}
                      <IconButton
                        size="small"
                        onClick={() => setShowPassword((v) => !v)}
                        aria-label="Toggle password visibility"
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
          </Field>
          {ssh && (
            <Field label="SSH key">
              <TextField
                select
                value={value.sshKeyId ?? ""}
                onChange={(e) =>
                  onChange({ sshKeyId: e.target.value === "" ? null : e.target.value })
                }
              >
                <MenuItem value="">
                  <em>
                    {inheritedKey && from
                      ? `Inherited — ${inheritedKey} (${from})`
                      : "None — password or agent"}
                  </em>
                </MenuItem>
                {(sshKeys.data ?? []).map((k) => (
                  <MenuItem key={k.id} value={k.id}>
                    {k.label}
                    <Typography
                      component="span"
                      variant="caption"
                      color="text.secondary"
                      sx={{ ml: 1 }}
                    >
                      {k.keyType}
                    </Typography>
                  </MenuItem>
                ))}
              </TextField>
            </Field>
          )}
        </>
      )}
      {ssh && (
        <SettingRow
          label="Agent forwarding"
          hint={
            !value.agentForwarding && inherited?.agentForwarding && from
              ? `Enabled by ${from}; turning it on here changes nothing.`
              : "Expose the local SSH agent on the remote side."
          }
          last
          control={
            <Switch
              checked={value.agentForwarding}
              onChange={(e) => onChange({ agentForwarding: e.target.checked })}
            />
          }
        />
      )}
    </>
  );
}
