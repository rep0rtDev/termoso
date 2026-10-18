import { useState } from "react";
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
import { tr } from "@/i18n";
import * as ipc from "@/ipc/commands";
import { keys } from "@/ipc/hooks";
import { errorMessage, type Team, type TeamPolicies, type UpdateTeamPolicies } from "@/ipc/types";
import { Loading } from "@/components/ui";
import { Section } from "./Section";
import { SettingRow } from "@/components/ui";
import { useSnackbar } from "@/components/Snackbar";
import { can } from "./roles";

export function PoliciesTab({ team }: { team: Team }) {
  const q = useQuery({
    queryKey: ["team", team.id, "policies"],
    queryFn: () => ipc.teamPolicies(team.id),
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
  const dirty =
    JSON.stringify({ ...p, sso_domains: parseDomains(domains) }) !== JSON.stringify(saved);
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
      };
      return ipc.teamUpdatePolicies(team.id, patch);
    },
    onSuccess: async () => {
      await qc.invalidateQueries({ queryKey: ["team", team.id, "policies"] });
      await qc.invalidateQueries({ queryKey: keys.account });
      snack.notify(tr("Policies saved"));
    },
    onError: (e) => snack.error(errorMessage(e)),
  });
  const ro = !manage;
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
        hint={tr(
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
        hint={tr(
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
        hint={tr(
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
        hint={tr(
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
        hint={tr(
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
        hint={tr("Invitations are limited to these domains (comma-separated). Empty = any.")}
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
        hint={tr("Every new device of a member needs confirmation by e-mail before it can sync.")}
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
        hint={tr(
          "Days to keep audit events (30–3650, capped by the server). Export before they expire.",
        )}
        last
        control={
          <TextField
            size="small"
            type="number"
            value={p.audit_retention_days}
            disabled={ro}
            onChange={(e) => setP({ ...p, audit_retention_days: Number(e.target.value) })}
            slotProps={{ htmlInput: { min: 30, max: 3650 } }}
            sx={{ width: 150 }}
          />
        }
      />
      <Box sx={{ pt: 1.5 }}>
        <Typography variant="caption" color="text.secondary">
          Last changed {new Date(saved.updated_at).toLocaleString()}
        </Typography>
      </Box>
    </Section>
  );
}

function parseDomains(s: string): string[] {
  return s
    .split(/[,\s]+/)
    .map((d) => d.trim().toLowerCase())
    .filter(Boolean);
}
