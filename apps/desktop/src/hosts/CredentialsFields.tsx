import { useState, type ReactNode } from "react";
import {
  Box,
  Button,
  IconButton,
  InputAdornment,
  Menu,
  MenuItem,
  Switch,
  TextField,
  Typography,
} from "@mui/material";
import AddRoundedIcon from "@mui/icons-material/AddRounded";
import BadgeOutlinedIcon from "@mui/icons-material/BadgeOutlined";
import CloseRoundedIcon from "@mui/icons-material/CloseRounded";
import ExpandMoreRoundedIcon from "@mui/icons-material/ExpandMoreRounded";
import KeyOutlinedIcon from "@mui/icons-material/KeyOutlined";
import PasswordRoundedIcon from "@mui/icons-material/PasswordRounded";
import PersonOutlineRoundedIcon from "@mui/icons-material/PersonOutlineRounded";
import VisibilityRoundedIcon from "@mui/icons-material/VisibilityRounded";
import VisibilityOffRoundedIcon from "@mui/icons-material/VisibilityOffRounded";
import { SettingRow } from "@/components/ui";
import { goToSection } from "@/app/navigation";
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

const adornment = (icon: ReactNode) => (
  <InputAdornment position="start" sx={{ color: "text.secondary" }}>
    {icon}
  </InputAdornment>
);

/**
 * Credentials the Termius way: a "Credentials" / "Credentials from <identity>"
 * header, then username and password, and the SSH key only once added via
 * "+ SSH key" (or when one is already set).
 */
export function CredentialsFields({
  vaultId,
  value,
  onChange,
  ssh,
  agentForwarding = ssh,
  inherited,
  inlineLabel,
}: {
  vaultId: Uuid;
  value: CredentialValues;
  onChange: (patch: Partial<CredentialValues>) => void;
  ssh: boolean;
  /** Render the Agent forwarding switch here (the host panel keeps it under Show more). */
  agentForwarding?: boolean;
  /** Effective values coming from the group chain, shown as placeholders. */
  inherited?: Inherited | null;
  inlineLabel: string;
}) {
  const identities = useIdentities(vaultId);
  const sshKeys = useSshKeys(vaultId);
  const [showPassword, setShowPassword] = useState(false);
  const [keyOpen, setKeyOpen] = useState(false);
  const [sourceAnchor, setSourceAnchor] = useState<HTMLElement | null>(null);

  const identity = (identities.data ?? []).find((i) => i.id === value.identityId) ?? null;
  const usingIdentity = value.identityId !== null;
  const from = inherited && inherited.groupPath.length > 0 ? inherited.groupPath.join(" / ") : null;
  const inheritedKey = inherited?.sshKeyLabel ?? null;
  const showKey = ssh && (keyOpen || value.sshKeyId !== null);
  const keyKnown =
    value.sshKeyId === null || (sshKeys.data ?? []).some((k) => k.id === value.sshKeyId);

  const sourceLabel = usingIdentity
    ? (identity?.label ?? (identities.data ? "Unknown identity" : "Loading…"))
    : null;
  // Nothing set here and the group provides something → the group is the source.
  const ownEmpty =
    !usingIdentity && !value.username && !value.hasPassword && !value.password && !value.sshKeyId;
  const inheritedFrom =
    ownEmpty &&
    from &&
    inherited &&
    (inherited.username || inherited.hasPassword || inherited.sshKeyId || inherited.identityId)
      ? from
      : null;

  return (
    <>
      <Box sx={{ display: "flex", alignItems: "center", gap: 0.5, minHeight: 28 }}>
        <Typography variant="subtitle2" noWrap>
          {usingIdentity || inheritedFrom ? "Credentials from" : "Credentials"}
        </Typography>
        {inheritedFrom && (
          <Typography
            variant="subtitle2"
            noWrap
            sx={{ minWidth: 0, color: "text.secondary", fontWeight: 400 }}
          >
            {inheritedFrom}
          </Typography>
        )}
        <Button
          variant="text"
          size="small"
          color="inherit"
          startIcon={usingIdentity ? <BadgeOutlinedIcon /> : undefined}
          endIcon={<ExpandMoreRoundedIcon />}
          onClick={(e) => setSourceAnchor(e.currentTarget)}
          sx={{ color: "text.secondary", ml: usingIdentity ? 0 : "auto", px: 0.75, flexShrink: 0 }}
        >
          {sourceLabel ?? "Identity"}
        </Button>
      </Box>
      <Menu
        open={sourceAnchor !== null}
        anchorEl={sourceAnchor}
        onClose={() => setSourceAnchor(null)}
        slotProps={{ paper: { sx: { minWidth: 240 } } }}
      >
        <MenuItem
          selected={!usingIdentity}
          onClick={() => {
            onChange({ identityId: null });
            setSourceAnchor(null);
          }}
        >
          {inlineLabel}
        </MenuItem>
        {(identities.data ?? []).map((i) => (
          <MenuItem
            key={i.id}
            selected={i.id === value.identityId}
            onClick={() => {
              onChange({ identityId: i.id });
              setSourceAnchor(null);
            }}
          >
            <BadgeOutlinedIcon fontSize="small" sx={{ mr: 1.25, color: "text.secondary" }} />
            {i.label}
            <Typography component="span" variant="caption" color="text.secondary" sx={{ ml: 1 }}>
              {i.username}
            </Typography>
          </MenuItem>
        ))}
        <MenuItem
          onClick={() => {
            setSourceAnchor(null);
            goToSection("keychain");
          }}
          sx={{ color: "primary.main" }}
        >
          <AddRoundedIcon fontSize="small" sx={{ mr: 1.25 }} />
          Open Keychain…
        </MenuItem>
      </Menu>

      {usingIdentity ? (
        <>
          <TextField
            value={identity?.username ?? ""}
            placeholder={identities.data ? "No username" : ""}
            disabled
            slotProps={{ input: { startAdornment: adornment(<PersonOutlineRoundedIcon />) } }}
          />
          <TextField
            value={identity?.hasPassword ? "••••••••••••" : ""}
            placeholder="No password"
            disabled
            slotProps={{ input: { startAdornment: adornment(<PasswordRoundedIcon />) } }}
          />
          {ssh && identity?.sshKeyLabel && (
            <TextField
              value={identity.sshKeyLabel}
              disabled
              slotProps={{ input: { startAdornment: adornment(<KeyOutlinedIcon />) } }}
            />
          )}
        </>
      ) : (
        <>
          <TextField
            value={value.username}
            onChange={(e) => onChange({ username: e.target.value })}
            autoComplete="off"
            placeholder={inherited?.username ? `${inherited.username} (inherited)` : "Username"}
            helperText={!value.username && inherited?.username && from ? `From ${from}` : undefined}
            slotProps={{
              input: { startAdornment: adornment(<PersonOutlineRoundedIcon />) },
              htmlInput: { "aria-label": "Username" },
            }}
          />
          <TextField
            type={showPassword ? "text" : "password"}
            value={value.password ?? ""}
            onChange={(e) => onChange({ password: e.target.value })}
            autoComplete="new-password"
            placeholder={
              value.hasPassword && value.password === null
                ? "••••••••••••"
                : inherited?.hasPassword
                  ? "•••••••• (inherited)"
                  : "Password"
            }
            helperText={
              value.hasPassword && value.password === null
                ? "A password is stored. Type to replace it or clear it to remove."
                : !value.hasPassword && value.password === null && inherited?.hasPassword && from
                  ? `From ${from}`
                  : undefined
            }
            slotProps={{
              htmlInput: { "aria-label": "Password" },
              input: {
                startAdornment: adornment(<PasswordRoundedIcon />),
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
          {showKey ? (
            <TextField
              select
              value={value.sshKeyId ?? ""}
              onChange={(e) => {
                const v = e.target.value;
                if (v === "__new") {
                  goToSection("keychain");
                  return;
                }
                onChange({ sshKeyId: v === "" ? null : v });
              }}
              helperText={
                value.sshKeyId === null && inheritedKey && from
                  ? `Without a key here “${inheritedKey}” from ${from} is used.`
                  : undefined
              }
              slotProps={{
                htmlInput: { "aria-label": "SSH key" },
                input: {
                  startAdornment: adornment(<KeyOutlinedIcon />),
                  endAdornment: (
                    <InputAdornment position="end" sx={{ mr: 2 }}>
                      <IconButton
                        size="small"
                        aria-label="Remove SSH key"
                        onClick={() => {
                          onChange({ sshKeyId: null });
                          setKeyOpen(false);
                        }}
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
              {!keyKnown && value.sshKeyId && (
                <MenuItem value={value.sshKeyId}>
                  <em>{sshKeys.data ? "Unknown key" : "Loading…"}</em>
                </MenuItem>
              )}
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
              <MenuItem value="__new" sx={{ color: "primary.main" }}>
                <AddRoundedIcon fontSize="small" sx={{ mr: 1.25 }} />
                New key in Keychain…
              </MenuItem>
            </TextField>
          ) : (
            ssh && (
              <Button
                variant="text"
                color="inherit"
                size="small"
                startIcon={<AddRoundedIcon />}
                onClick={() => setKeyOpen(true)}
                sx={{ alignSelf: "flex-start", color: "text.secondary", ml: -0.5 }}
              >
                SSH key
                {inheritedKey && from && (
                  <Typography
                    component="span"
                    variant="caption"
                    color="text.disabled"
                    sx={{ ml: 1 }}
                  >
                    using “{inheritedKey}” from {from}
                  </Typography>
                )}
              </Button>
            )
          )}
        </>
      )}
      {ssh && agentForwarding && (
        <AgentForwardingRow value={value} onChange={onChange} inherited={inherited} />
      )}
    </>
  );
}

export function AgentForwardingRow({
  value,
  onChange,
  inherited,
}: {
  value: Pick<CredentialValues, "agentForwarding">;
  onChange: (patch: Pick<CredentialValues, "agentForwarding">) => void;
  inherited?: Inherited | null;
}) {
  const from = inherited && inherited.groupPath.length > 0 ? inherited.groupPath.join(" / ") : null;
  return (
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
  );
}
