import { useState } from "react";
import { Alert, Button, Chip, MenuItem, Select, Stack, Switch, TextField } from "@mui/material";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { tr } from "@/i18n";
import * as ipc from "@/ipc/commands";

import { errorMessage, type Team } from "@/ipc/types";
import { Loading, SettingRow } from "@/components/ui";
import { ConfirmDialog } from "@/components/ConfirmDialog";

import { useSnackbar } from "@/components/Snackbar";
import { Section } from "./Section";
import { can } from "./roles";
import type { TeamIdp, TeamIdpKind, UpdateTeamIdp } from "@/ipc/types";

interface Form {
  kind: TeamIdpKind;
  name: string;
  issuer: string;
  client_id: string;
  client_secret: string;
  scopes: string;
  saml_metadata: string;
  saml_idp_entity_id: string;
  domains: string;
  enabled: boolean;
  jit_join: boolean;
}

function fromIdp(idp: TeamIdp | null): Form {
  return {
    kind: idp?.kind ?? "oidc",
    name: idp?.name ?? "",
    issuer: idp?.issuer ?? "",
    client_id: idp?.client_id ?? "",
    client_secret: "",
    scopes: idp?.scopes ?? "",
    saml_metadata: idp?.saml_metadata ?? "",
    saml_idp_entity_id: idp?.saml_idp_entity_id ?? "",
    domains: idp?.domains.join(", ") ?? "",
    enabled: idp?.enabled ?? true,
    jit_join: idp?.jit_join ?? true,
  };
}

function toPatch(f: Form): UpdateTeamIdp {
  const p: UpdateTeamIdp = {
    kind: f.kind,
    name: f.name.trim(),
    domains: f.domains
      .split(/[\s,]+/)
      .map((d) => d.trim().toLowerCase())
      .filter(Boolean),
    enabled: f.enabled,
    jit_join: f.jit_join,
  };
  if (f.kind === "oidc") {
    p.issuer = f.issuer.trim();
    p.client_id = f.client_id.trim();
    if (f.client_secret) p.client_secret = f.client_secret;
    if (f.scopes.trim()) p.scopes = f.scopes.trim();
  } else {
    p.saml_metadata = f.saml_metadata.trim();
    if (f.saml_idp_entity_id.trim()) p.saml_idp_entity_id = f.saml_idp_entity_id.trim();
  }
  return p;
}

export function SsoTab({ team }: { team: Team }) {
  const q = useQuery({ queryKey: ["team", team.id, "idp"], queryFn: () => ipc.teamIdp(team.id) });
  if (q.isPending) return <Loading />;
  if (q.isError) return <Alert severity="error">{errorMessage(q.error)}</Alert>;
  if (!q.data.available) {
    return (
      <Alert severity="info">
        {tr(
          "Per-team identity providers are disabled on this server (TERMOSO_TEAM_IDP=false). Server-wide SSO providers still work.",
        )}
      </Alert>
    );
  }
  return <IdpForm key={q.data.idp?.updated_at ?? "new"} team={team} idp={q.data.idp} />;
}

function IdpForm({ team, idp }: { team: Team; idp: TeamIdp | null }) {
  const qc = useQueryClient();
  const snack = useSnackbar();
  const [form, setForm] = useState<Form>(() => fromIdp(idp));
  const [removing, setRemoving] = useState(false);
  const manage = can(team, "policies.manage");
  const refresh = () => qc.invalidateQueries({ queryKey: ["team", team.id, "idp"] });
  const save = useMutation({
    mutationFn: (f: Form) => ipc.teamUpdateIdp(team.id, toPatch(f)),
    onSuccess: async () => {
      await refresh();
      snack.notify(tr("Identity provider saved"));
    },
    onError: (e) => snack.error(errorMessage(e)),
  });
  const test = useMutation({
    mutationFn: (f: Form) => ipc.teamTestIdp(team.id, toPatch(f)),
    onSuccess: () => snack.notify(tr("Provider reachable: discovery and metadata look valid")),
    onError: (e) => snack.error(errorMessage(e)),
  });
  const remove = useMutation({
    mutationFn: () => ipc.teamDeleteIdp(team.id),
    onSuccess: async () => {
      await refresh();
      setRemoving(false);
      snack.notify(tr("Identity provider removed"));
    },
    onError: (e) => snack.error(errorMessage(e)),
  });
  const set = <K extends keyof Form>(k: K, v: Form[K]) => setForm({ ...form, [k]: v });
  const ro = !manage;
  const valid =
    form.name.trim() !== "" &&
    (form.kind === "oidc"
      ? form.issuer.trim() !== "" &&
        form.client_id.trim() !== "" &&
        (form.client_secret !== "" || (idp?.has_client_secret ?? false))
      : form.saml_metadata.trim() !== "");
  const w = { width: 320 };
  return (
    <Stack spacing={2}>
      <Section
        title={tr("Team identity provider")}
        description={tr(
          "Members whose e-mail domain is listed below sign in through your company's IdP: the login form detects the domain and offers “Continue with <provider>”. New employees get an account and join this team on first sign-in (JIT) with the default role, or a role mapped from their IdP groups. To make SSO mandatory, set the policy “Sign in only through SSO” with provider “team”.",
        )}
        actions={
          manage && (
            <Stack direction="row" spacing={1}>
              <Button disabled={!valid || test.isPending} onClick={() => test.mutate(form)}>
                {tr("Test")}
              </Button>
              <Button
                variant="contained"
                disabled={!valid || save.isPending}
                onClick={() => save.mutate(form)}
              >
                {tr("Save")}
              </Button>
            </Stack>
          )
        }
      >
        {idp && (
          <Stack direction="row" spacing={1} sx={{ mb: 1.5 }}>
            <Chip
              size="small"
              color={idp.enabled ? "success" : "default"}
              variant="outlined"
              label={idp.enabled ? tr("Enabled") : tr("Disabled")}
            />
            <Chip size="small" variant="outlined" label={idp.kind.toUpperCase()} />
          </Stack>
        )}
        <SettingRow
          label={tr("Protocol")}
          control={
            <Select
              size="small"
              value={form.kind}
              disabled={ro}
              onChange={(e) => set("kind", e.target.value)}
              sx={{ minWidth: 160 }}
            >
              <MenuItem value="oidc">OpenID Connect</MenuItem>
              <MenuItem value="saml">SAML 2.0</MenuItem>
            </Select>
          }
        />
        <SettingRow
          label={tr("Display name")}
          hint={tr("Shown on the login button, e.g. “Okta (Acme)”.")}
          control={
            <TextField
              size="small"
              value={form.name}
              disabled={ro}
              onChange={(e) => set("name", e.target.value)}
              sx={w}
            />
          }
        />
        {form.kind === "oidc" ? (
          <>
            <SettingRow
              label={tr("Issuer URL")}
              hint={tr("Discovery is read from <issuer>/.well-known/openid-configuration.")}
              control={
                <TextField
                  size="small"
                  value={form.issuer}
                  disabled={ro}
                  placeholder="https://login.example.com"
                  onChange={(e) => set("issuer", e.target.value)}
                  sx={w}
                />
              }
            />
            <SettingRow
              label={tr("Client ID")}
              control={
                <TextField
                  size="small"
                  value={form.client_id}
                  disabled={ro}
                  onChange={(e) => set("client_id", e.target.value)}
                  sx={w}
                />
              }
            />
            <SettingRow
              label={tr("Client secret")}
              hint={
                idp?.has_client_secret
                  ? tr(
                      "Stored encrypted under the server master key. Leave empty to keep the current one.",
                    )
                  : tr("Stored encrypted under the server master key.")
              }
              control={
                <TextField
                  size="small"
                  type="password"
                  autoComplete="new-password"
                  value={form.client_secret}
                  disabled={ro}
                  placeholder={idp?.has_client_secret ? "••••••••" : ""}
                  onChange={(e) => set("client_secret", e.target.value)}
                  sx={w}
                />
              }
            />
            <SettingRow
              label={tr("Scopes")}
              hint={tr("Optional, space-separated. Default: openid email profile groups.")}
              control={
                <TextField
                  size="small"
                  value={form.scopes}
                  disabled={ro}
                  onChange={(e) => set("scopes", e.target.value)}
                  sx={w}
                />
              }
            />
          </>
        ) : (
          <>
            <SettingRow
              label={tr("IdP metadata XML")}
              hint={tr("Paste the SAML metadata document exported from your IdP.")}
              control={
                <TextField
                  size="small"
                  multiline
                  minRows={3}
                  maxRows={8}
                  value={form.saml_metadata}
                  disabled={ro}
                  onChange={(e) => set("saml_metadata", e.target.value)}
                  sx={{ width: 360, fontFamily: "monospace" }}
                />
              }
            />
            <SettingRow
              label={tr("IdP entity ID")}
              hint={tr("Optional; taken from the metadata when empty.")}
              control={
                <TextField
                  size="small"
                  value={form.saml_idp_entity_id}
                  disabled={ro}
                  onChange={(e) => set("saml_idp_entity_id", e.target.value)}
                  sx={w}
                />
              }
            />
          </>
        )}
        <SettingRow
          label={tr("E-mail domains")}
          hint={tr(
            "Comma-separated. A domain can belong to one team only; sign-in with these domains is routed to this provider.",
          )}
          control={
            <TextField
              size="small"
              value={form.domains}
              disabled={ro}
              placeholder="acme.com, acme.io"
              onChange={(e) => set("domains", e.target.value)}
              sx={w}
            />
          }
        />
        <SettingRow
          label={tr("Join this team on first sign-in")}
          hint={tr(
            "New accounts created through the IdP become members with the default role (or a mapped one).",
          )}
          control={
            <Switch
              checked={form.jit_join}
              disabled={ro}
              onChange={(e) => set("jit_join", e.target.checked)}
            />
          }
        />
        <SettingRow
          label={tr("Enabled")}
          last
          control={
            <Switch
              checked={form.enabled}
              disabled={ro}
              onChange={(e) => set("enabled", e.target.checked)}
            />
          }
        />
      </Section>
      {idp && (
        <Section
          title={tr("Register in your IdP")}
          description={tr(
            "Add this redirect / ACS URL to the application in your identity provider.",
          )}
          actions={
            manage && (
              <Button color="error" onClick={() => setRemoving(true)}>
                {tr("Remove provider")}
              </Button>
            )
          }
        >
          <TextField
            size="small"
            fullWidth
            value={idp.callback_url}
            slotProps={{ input: { readOnly: true } }}
            onFocus={(e) => e.target.select()}
          />
        </Section>
      )}
      <ConfirmDialog
        open={removing}
        title={tr("Remove identity provider?")}
        confirmLabel={tr("Remove")}
        danger
        busy={remove.isPending}
        onCancel={() => setRemoving(false)}
        onConfirm={() => remove.mutate()}
      >
        {tr(
          "Members will no longer be able to sign in through it. Accounts and team memberships stay; if the policy requires SSO through the team provider, members will be locked out until you change it.",
        )}
      </ConfirmDialog>
    </Stack>
  );
}
