import type { Dispatch, SetStateAction } from "react";
import {
  Autocomplete,
  Box,
  IconButton,
  InputAdornment,
  MenuItem,
  Select,
  TextField,
} from "@mui/material";
import VisibilityOffOutlinedIcon from "@mui/icons-material/VisibilityOffOutlined";
import VisibilityOutlinedIcon from "@mui/icons-material/VisibilityOutlined";
import { Field } from "@/components/ui";
import type { CloudProvider } from "@/ipc/types";
import { AWS_REGIONS, type Draft } from "./cloud";
import { tr } from "@/i18n";

interface Props {
  provider: CloudProvider;
  draft: Draft;
  setDraft: Dispatch<SetStateAction<Draft>>;
  reveal: boolean;
  onReveal: () => void;
  disabled?: boolean;
  /**
   * Shown in the empty secret field when a secret is already stored, so the
   * user knows leaving it blank keeps the old one.
   */
  storedSecretHint?: string;
}

/** Provider credential inputs shared by one-time import and cloud sync. */
export function CloudCredentialFields({
  provider,
  draft,
  setDraft,
  reveal,
  onReveal,
  disabled,
  storedSecretHint,
}: Props) {
  return (
    <>
      {provider === "aws" && (
        <>
          <Field label={tr("Region")}>
            <Autocomplete
              freeSolo
              size="small"
              options={AWS_REGIONS}
              value={draft.aws.region}
              disabled={disabled}
              onInputChange={(_, v) => setDraft((d) => ({ ...d, aws: { ...d.aws, region: v } }))}
              renderInput={(params) => (
                <TextField {...params} placeholder="us-east-1" autoComplete="off" />
              )}
            />
          </Field>
          <Field label={tr("Access Key ID")}>
            <TextField
              fullWidth
              size="small"
              value={draft.aws.accessKeyId}
              disabled={disabled}
              onChange={(e) =>
                setDraft((d) => ({ ...d, aws: { ...d.aws, accessKeyId: e.target.value } }))
              }
              placeholder="AKIA…"
              autoComplete="off"
              slotProps={{ input: { spellCheck: false } }}
            />
          </Field>
          <Field label={tr("Secret Access Key")}>
            <SecretField
              value={draft.aws.secretAccessKey}
              reveal={reveal}
              onReveal={onReveal}
              disabled={disabled}
              placeholder={storedSecretHint}
              onChange={(v) => setDraft((d) => ({ ...d, aws: { ...d.aws, secretAccessKey: v } }))}
            />
          </Field>
          <Box sx={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 1.5 }}>
            <Field label={tr("Service")}>
              <Select
                fullWidth
                size="small"
                value={draft.aws.service}
                disabled={disabled}
                onChange={(e) =>
                  setDraft((d) => ({ ...d, aws: { ...d.aws, service: e.target.value } }))
                }
              >
                <MenuItem value="ec2">EC2</MenuItem>
                <MenuItem value="lightsail">{tr("Lightsail")}</MenuItem>
              </Select>
            </Field>
            <Field label={tr("IP address type")}>
              <Select
                fullWidth
                size="small"
                value={draft.aws.addressType}
                disabled={disabled}
                onChange={(e) =>
                  setDraft((d) => ({ ...d, aws: { ...d.aws, addressType: e.target.value } }))
                }
              >
                <MenuItem value="public">{tr("Public")}</MenuItem>
                <MenuItem value="private">{tr("Private")}</MenuItem>
              </Select>
            </Field>
          </Box>
        </>
      )}
      {provider === "digital_ocean" && (
        <Field
          label={tr("Token")}
          hint={tr(
            "A personal access token with read scope is enough (API → Tokens in the DigitalOcean control panel).",
          )}
        >
          <SecretField
            value={draft.digitalOcean.token}
            reveal={reveal}
            onReveal={onReveal}
            disabled={disabled}
            onChange={(v) => setDraft((d) => ({ ...d, digitalOcean: { token: v } }))}
            placeholder={storedSecretHint ?? "dop_v1_…"}
          />
        </Field>
      )}
      {provider === "azure" && (
        <>
          <Field label={tr("Tenant ID")}>
            <TextField
              fullWidth
              size="small"
              value={draft.azure.tenantId}
              disabled={disabled}
              onChange={(e) =>
                setDraft((d) => ({ ...d, azure: { ...d.azure, tenantId: e.target.value } }))
              }
              autoComplete="off"
              slotProps={{ input: { spellCheck: false } }}
            />
          </Field>
          <Field label={tr("Client ID")}>
            <TextField
              fullWidth
              size="small"
              value={draft.azure.clientId}
              disabled={disabled}
              onChange={(e) =>
                setDraft((d) => ({ ...d, azure: { ...d.azure, clientId: e.target.value } }))
              }
              autoComplete="off"
              slotProps={{ input: { spellCheck: false } }}
            />
          </Field>
          <Field
            label={tr("Client Secret")}
            hint={tr(
              "An app registration with the Reader role on the subscriptions you want to list.",
            )}
          >
            <SecretField
              value={draft.azure.clientSecret}
              reveal={reveal}
              onReveal={onReveal}
              disabled={disabled}
              placeholder={storedSecretHint}
              onChange={(v) => setDraft((d) => ({ ...d, azure: { ...d.azure, clientSecret: v } }))}
            />
          </Field>
        </>
      )}
    </>
  );
}

export function SecretField({
  value,
  reveal,
  onReveal,
  onChange,
  placeholder,
  disabled,
}: {
  value: string;
  reveal: boolean;
  onReveal: () => void;
  onChange: (v: string) => void;
  placeholder?: string;
  disabled?: boolean;
}) {
  return (
    <TextField
      fullWidth
      size="small"
      type={reveal ? "text" : "password"}
      value={value}
      disabled={disabled}
      onChange={(e) => onChange(e.target.value)}
      placeholder={placeholder}
      autoComplete="off"
      slotProps={{
        input: {
          spellCheck: false,
          endAdornment: (
            <InputAdornment position="end">
              <IconButton
                size="small"
                edge="end"
                onClick={onReveal}
                aria-label={reveal ? tr("Hide") : tr("Show")}
              >
                {reveal ? (
                  <VisibilityOffOutlinedIcon fontSize="small" />
                ) : (
                  <VisibilityOutlinedIcon fontSize="small" />
                )}
              </IconButton>
            </InputAdornment>
          ),
        },
      }}
    />
  );
}
