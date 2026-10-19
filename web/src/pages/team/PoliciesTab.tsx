import { useState } from "react";
import { tr } from "@/i18n";
import { formatDateTime } from "@/components/format";
import {
  Alert,
  Box,
  Button,
  Chip,
  FormControl,
  MenuItem,
  Select,
  Stack,
  Switch,
  TextField,
  Typography,
} from "@mui/material";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { errorMessage } from "@/api/client";
import { teamsApi } from "@/api/endpoints";
import { queryKeys, useServerInfo } from "@/api/hooks";
import type { Team, TeamPolicies, UpdateTeamPolicies } from "@/api/types";
import { Loading } from "@/components/Loading";
import { Section, SettingRow } from "@/components/Section";
import { useSnackbar } from "@/components/Snackbar";
import { can } from "./perm";

export function PoliciesTab({ team }: { team: Team }) {
  const q = useQuery({
    queryKey: queryKeys.teamPolicies(team.id),
    queryFn: () => teamsApi.policies(team.id),
  });
  if (q.isPending) return <Loading />;
  if (q.isError) return <Alert severity="error">{errorMessage(q.error)}</Alert>;
  return (
    <PoliciesForm
      key={q.data.policies.updated_at}
      team={team}
      saved={q.data.policies}
      manage={can(team, "policies.manage")}
    />
  );
}

function PoliciesForm({
  team,
  saved,
  manage,
}: {
  team: Team;
  saved: TeamPolicies;
  manage: boolean;
}) {
  const qc = useQueryClient();
  const snack = useSnackbar();
  const [p, setP] = useState<TeamPolicies>(saved);
  const [domains, setDomains] = useState(saved.sso_domains.join(", "));
  const [ipList, setIpList] = useState(saved.ip_allowlist.join("\n"));
  const dirty =
    JSON.stringify({
      ...p,
      sso_domains: parseDomains(domains),
      ip_allowlist: parseList(ipList),
    }) !== JSON.stringify(saved);
  const save = useMutation({
    mutationFn: () => {
      const patch: UpdateTeamPolicies = {
        mfa: p.mfa,
        local_keys: p.local_keys,
        session_ttl_hours: p.session_ttl_hours,
        sso_only: p.sso_only,
        sso_provider: p.sso_provider,
        sso_domains: parseDomains(domains),
        device_approval: p.device_approval,
        audit_retention_days: p.audit_retention_days,
        session_recording: p.session_recording,
        log_retention_days: p.log_retention_days,
        key_max_age_days: p.key_max_age_days,
        password_max_age_days: p.password_max_age_days,
        ip_allowlist: parseList(ipList),
        device_admin_approval: p.device_admin_approval,
      };
      return teamsApi.updatePolicies(team.id, patch);
    },
    onSuccess: async () => {
      await qc.invalidateQueries({ queryKey: queryKeys.teamPolicies(team.id) });
      await qc.invalidateQueries({ queryKey: queryKeys.team(team.id) });
      snack.notify(tr("Policies saved"));
    },
    onError: (e) => snack.error(errorMessage(e)),
  });
  const ro = !manage;
  const serverInfo = useServerInfo();
  const retentionCap = serverInfo.data?.audit_retention_days_max ?? 3650;
  return (
    <Section
      title={tr("Team policies")}
      description={tr(
        "Apply to every member. When someone belongs to several teams, the strictest value across them wins. Members see which team requires what.",
      )}
      actions={
        manage && (
          <Button
            variant="contained"
            disabled={!dirty || save.isPending}
            onClick={() => save.mutate()}
          >
            {tr("Save")}
          </Button>
        )
      }
    >
      <SettingRow
        label={tr("Two-factor authentication")}
        description={tr(
          "“Any” accepts TOTP, e-mail codes or passkeys; “Passkey” requires WebAuthn. Enforced at sign-in and before team data is served.",
        )}
        control={
          <FormControl size="small">
            <Select
              value={p.mfa}
              disabled={ro}
              onChange={(e) => setP({ ...p, mfa: e.target.value })}
            >
              <MenuItem value="off">{tr("Not required")}</MenuItem>
              <MenuItem value="any">{tr("Any second factor")}</MenuItem>
              <MenuItem value="passkey">{tr("Passkey only")}</MenuItem>
            </Select>
          </FormControl>
        }
      />
      <SettingRow
        label={
          <Stack direction="row" spacing={1} sx={{ alignItems: "center" }}>
            <span>{tr("Keys only in team vaults")}</span>
            <Chip label={tr("client")} size="small" variant="outlined" sx={{ height: 18 }} />
          </Stack>
        }
        description={tr(
          "Deny device-local identities for this team's hosts; keys live in team vaults or SSH ID. Honoured by the apps — the server never sees keys.",
        )}
        control={
          <Switch
            checked={p.local_keys === "deny"}
            disabled={ro}
            onChange={(e) => setP({ ...p, local_keys: e.target.checked ? "deny" : "allow" })}
          />
        }
      />
      <SettingRow
        label={tr("Session lifetime")}
        description={tr(
          "Hours before members must sign in again (1–2160). Existing sessions are shortened when you lower it.",
        )}
        control={
          <TextField
            size="small"
            type="number"
            placeholder={tr("server default")}
            value={p.session_ttl_hours ?? ""}
            disabled={ro}
            onChange={(e) =>
              setP({
                ...p,
                session_ttl_hours: e.target.value === "" ? null : Number(e.target.value),
              })
            }
            slotProps={{ htmlInput: { min: 1, max: 2160 } }}
            sx={{ width: 150 }}
          />
        }
      />
      <SettingRow
        label={tr("Sign in only through SSO")}
        description={tr(
          "Members (except the owner) must use the configured identity provider; passwords are refused. Needs server-side SSO.",
        )}
        control={
          <Switch
            checked={p.sso_only}
            disabled={ro}
            onChange={(e) => setP({ ...p, sso_only: e.target.checked })}
          />
        }
      />
      <SettingRow
        label={tr("SSO provider")}
        description={tr(
          "Provider id as configured on the server (e.g. okta). Empty = any configured provider.",
        )}
        control={
          <TextField
            size="small"
            value={p.sso_provider ?? ""}
            disabled={ro}
            onChange={(e) => setP({ ...p, sso_provider: e.target.value.trim() || null })}
            sx={{ width: 200 }}
          />
        }
      />
      <SettingRow
        label={tr("Allowed e-mail domains")}
        description={tr("Invitations are limited to these domains (comma-separated). Empty = any.")}
        control={
          <TextField
            size="small"
            value={domains}
            disabled={ro}
            onChange={(e) => setDomains(e.target.value)}
            placeholder={tr("example.com, corp.example.com")}
            sx={{ width: 280 }}
          />
        }
      />
      <SettingRow
        label={tr("Approve new devices")}
        description={tr(
          "Every new device of a member needs confirmation by e-mail before it can sync.",
        )}
        control={
          <Switch
            checked={p.device_approval}
            disabled={ro}
            onChange={(e) => setP({ ...p, device_approval: e.target.checked })}
          />
        }
      />
      <SettingRow
        label={tr("Audit log retention")}
        description={tr(
          "Days to keep audit events (30–3650, capped by the server). Export before they expire.",
        )}
        control={
          <TextField
            size="small"
            type="number"
            value={p.audit_retention_days}
            disabled={ro}
            onChange={(e) => setP({ ...p, audit_retention_days: Number(e.target.value) })}
            slotProps={{ htmlInput: { min: 30, max: retentionCap } }}
            helperText={tr("Max. {v1} days on this server", { v1: retentionCap })}
            sx={{ width: 150 }}
          />
        }
      />
      <SettingRow
        label={tr("Session recording")}
        description={tr(
          "Required turns recording on in every team vault; members cannot switch it off and only logs.delete may remove recordings.",
        )}
        control={
          <Select
            size="small"
            value={p.session_recording}
            disabled={ro}
            onChange={(e) => setP({ ...p, session_recording: e.target.value })}
            sx={{ minWidth: 180 }}
          >
            <MenuItem value="optional">{tr("Optional")}</MenuItem>
            <MenuItem value="required">{tr("Required")}</MenuItem>
          </Select>
        }
      />
      <SettingRow
        label={tr("Recording retention")}
        description={tr(
          "Days before unpinned session recordings are deleted; 0 keeps them forever.",
        )}
        control={
          <TextField
            size="small"
            type="number"
            value={p.log_retention_days}
            disabled={ro}
            onChange={(e) => setP({ ...p, log_retention_days: Number(e.target.value) })}
            slotProps={{ htmlInput: { min: 0, max: 3650 } }}
            helperText={
              p.log_retention_days === 0
                ? tr("Forever")
                : tr("{v1} days", { v1: p.log_retention_days })
            }
            sx={{ width: 150 }}
          />
        }
      />
      <SettingRow
        label={tr("Key rotation age")}
        description={tr(
          "SSH keys, certificates and identities older than this are listed as due in Rotation; 0 disables the check.",
        )}
        control={
          <TextField
            size="small"
            type="number"
            value={p.key_max_age_days}
            disabled={ro}
            onChange={(e) => setP({ ...p, key_max_age_days: Number(e.target.value) })}
            slotProps={{ htmlInput: { min: 0, max: 3650 } }}
            helperText={
              p.key_max_age_days === 0 ? tr("Off") : tr("{v1} days", { v1: p.key_max_age_days })
            }
            sx={{ width: 150 }}
          />
        }
      />
      <SettingRow
        label={tr("Password rotation age")}
        description={tr(
          "Host passwords older than this are due for rotation; 0 disables the check.",
        )}
        control={
          <TextField
            size="small"
            type="number"
            value={p.password_max_age_days}
            disabled={ro}
            onChange={(e) => setP({ ...p, password_max_age_days: Number(e.target.value) })}
            slotProps={{ htmlInput: { min: 0, max: 3650 } }}
            helperText={
              p.password_max_age_days === 0
                ? tr("Off")
                : tr("{v1} days", { v1: p.password_max_age_days })
            }
            sx={{ width: 150 }}
          />
        }
      />
      <SettingRow
        label={tr("IP allow-list")}
        description={tr(
          "Members may sign in only from these networks (CIDR, one per line, up to 100). Empty allows everywhere. Owners and admins are included — keep your own network in the list.",
        )}
        control={
          <TextField
            size="small"
            multiline
            minRows={2}
            placeholder={"203.0.113.0/24\n2001:db8::/32"}
            value={ipList}
            disabled={ro}
            onChange={(e) => setIpList(e.target.value)}
            sx={{ width: { xs: "100%", sm: 260 }, fontFamily: "monospace" }}
          />
        }
      />
      <SettingRow
        label={tr("Admin approval for new devices")}
        description={tr(
          "A member's new device waits in Devices until an owner, admin or devices.approve holder approves it; rejection signs it out.",
        )}
        divider={false}
        control={
          <Switch
            checked={p.device_admin_approval}
            disabled={ro}
            onChange={(e) => setP({ ...p, device_admin_approval: e.target.checked })}
          />
        }
      />
      <Box sx={{ pt: 1.5 }}>
        <Typography variant="caption" color="text.secondary">
          {tr("Last changed {v1}", { v1: formatDateTime(saved.updated_at) })}
        </Typography>
      </Box>
    </Section>
  );
}

function parseList(s: string): string[] {
  return s
    .split(/[\s,]+/)
    .map((x) => x.trim())
    .filter(Boolean);
}

function parseDomains(s: string): string[] {
  return s
    .split(/[,\s]+/)
    .map((d) => d.trim().toLowerCase())
    .filter(Boolean);
}
