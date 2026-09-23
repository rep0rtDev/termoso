import type { AuditEvent, Uuid } from "@/ipc/types";
import { tr, trn, msg } from "@/i18n";

/** Coarse groups for the action filter; each maps to an action prefix. */
export const ACTIVITY_GROUPS = [
  { value: "", label: msg("All activity") },
  { value: "team.", label: msg("Team") },
  { value: "member.", label: msg("Members") },
  { value: "invite.", label: msg("Invitations") },
  { value: "vault.", label: msg("Vaults & access") },
  { value: "entity.", label: msg("Shared data") },
  { value: "multiplayer.", label: msg("Multiplayer") },
] as const;

export interface ActivityContext {
  /** Vault names by id, for "in Staging". */
  vaultNames: ReadonlyMap<Uuid, string>;
  /** Member display names/emails by id, for targets the server couldn't join. */
  people: ReadonlyMap<Uuid, string>;
}

export interface ActivityLine {
  /** Who did it: display name, email or "Someone" when the account is gone. */
  actor: string;
  /** Verb phrase following the actor, e.g. "renamed the team to Ops". */
  text: string;
  /** Vault the event belongs to, when any. */
  vault: string | null;
  /** Extra metadata worth showing on a second line. */
  meta: string | null;
}

const KIND_LABEL: Record<string, [string, string]> = {
  host: [msg("a host"), msg("{count} hosts")],
  group: [msg("a group"), msg("{count} groups")],
  ssh_key: [msg("an SSH key"), msg("{count} SSH keys")],
  ssh_certificate: [msg("a certificate"), msg("{count} certificates")],
  identity: [msg("an identity"), msg("{count} identities")],
  known_host: [msg("a known host"), msg("{count} known hosts")],
  snippet: [msg("a snippet"), msg("{count} snippets")],
  snippet_package: [msg("a snippet package"), msg("{count} snippet packages")],
  host_snippet: [msg("a snippet target"), msg("{count} snippet targets")],
  pf_rule: [msg("a port forwarding rule"), msg("{count} port forwarding rules")],
  proxy: [msg("a proxy"), msg("{count} proxies")],
  host_chain: [msg("a host chain"), msg("{count} host chains")],
  tag: [msg("a tag"), msg("{count} tags")],
  tag_host: [msg("a tag link"), msg("{count} tag links")],
  ssh_config: [msg("an SSH config"), msg("{count} SSH configs")],
  telnet_config: [msg("a Telnet config"), msg("{count} Telnet configs")],
  webdav_config: [msg("a WebDAV config"), msg("{count} WebDAV configs")],
  serial_config: [msg("a serial config"), msg("{count} serial configs")],
  port_knocking: [msg("a port knocking"), msg("{count} port knockings")],
  workspace: [msg("a workspace"), msg("{count} workspaces")],
  workspace_template: [msg("a workspace template"), msg("{count} workspace templates")],
  log_bookmark: [msg("a log bookmark"), msg("{count} log bookmarks")],
  cloud_import: [msg("a cloud import"), msg("{count} cloud imports")],
};

const ROLE_LABEL: Record<string, string> = {
  viewer: msg("can view"),
  editor: msg("can edit"),
  manager: msg("can manage"),
  owner: msg("Owner"),
  admin: msg("Admin"),
  member: msg("Member"),
};

const str = (v: unknown): string | null => (typeof v === "string" && v.length > 0 ? v : null);
const num = (v: unknown): number | null => (typeof v === "number" ? v : null);
const bool = (v: unknown): boolean | null => (typeof v === "boolean" ? v : null);
const list = (v: unknown): unknown[] => (Array.isArray(v) ? v : []);

const countOf = (kind: string | null, count: number) => {
  const known = KIND_LABEL[kind ?? ""];
  if (known) return trn(count, known[0], known[1]);
  const k = kind ?? "item";
  return count === 1 ? tr("a {kind}", { kind: k }) : `${count} ${k}s`;
};

const wasRole = (r: string) => tr("was {role}", { role: tr(ROLE_LABEL[r] ?? r) });
const members = (n: number) => trn(n, "{count} member", "{count} members");
const vaults = (n: number) => trn(n, "{count} vault", "{count} vaults");

/** Turn one audit row into a readable sentence. Never echoes secrets: the
 *  server only stores metadata, and this only reads known keys from it. */
export function describeEvent(ev: AuditEvent, ctx: ActivityContext): ActivityLine {
  const d = ev.details;
  const actor = ev.actor_name ?? ev.actor_email ?? tr("Someone");
  const target =
    ev.target_email ??
    (ev.target_user ? (ctx.people.get(ev.target_user) ?? tr("a member")) : null) ??
    str(d.email) ??
    tr("a member");
  const self = ev.target_user !== undefined && ev.target_user === ev.actor_id;
  const vault = ev.vault_id
    ? (ctx.vaultNames.get(ev.vault_id) ?? str(d.name) ?? tr("a vault"))
    : null;
  const role = str(d.role);
  const prev = str(d.previous_role);
  const roleText = (r: string | null) => (r ? tr(ROLE_LABEL[r] ?? r) : "");
  const email = str(d.email) ?? tr("someone");

  let text: string;
  let meta: string | null = null;
  switch (ev.action) {
    case "team.created":
      text = str(d.name)
        ? tr("created the team “{name}”", { name: str(d.name) ?? "" })
        : tr("created the team");
      break;
    case "team.renamed":
      text = tr("renamed the team to “{name}”", { name: str(d.name) ?? "…" });
      break;
    case "team.settings": {
      const mp = bool(d.multiplayer_enabled);
      const mfa = bool(d.require_mfa);
      text =
        mp !== null
          ? mp
            ? tr("enabled Multiplayer")
            : tr("disabled Multiplayer")
          : mfa !== null
            ? mfa
              ? tr("required 2FA for the team")
              : tr("stopped requiring 2FA for the team")
            : tr("changed team settings");
      break;
    }
    case "member.role":
      text = self
        ? tr("changed their own role to {role}", { role: roleText(role) })
        : tr("changed {target}'s role to {role}", { target, role: roleText(role) });
      if (prev) meta = wasRole(prev);
      break;
    case "member.removed":
      text = tr("removed {target} from the team", { target });
      if (prev) meta = wasRole(prev);
      else if (str(d.account) === "converted") meta = tr("account converted to individual");
      break;
    case "member.account_deleted":
      text = tr("deleted {target}'s account", { target });
      if (role) meta = wasRole(role);
      break;
    case "member.left":
      text = tr("left the team");
      if (str(d.account) === "converted") meta = tr("account converted to individual");
      break;
    case "invite.created":
      text = role
        ? tr("invited {email} as {role}", { email, role: roleText(role) })
        : tr("invited {email}", { email });
      if (list(d.vault_ids).length > 0)
        meta = tr("with access to {vaults}", { vaults: vaults(list(d.vault_ids).length) });
      break;
    case "invite.revoked":
      text = tr("revoked the invitation for {email}", { email });
      break;
    case "invite.accepted":
      text = role
        ? tr("joined the team as {role}", { role: roleText(role) })
        : tr("joined the team");
      break;
    case "vault.created":
      text = tr("created the vault “{name}”", { name: vault ?? "…" });
      if (list(d.members).length > 0)
        meta = tr("shared with {members}", { members: members(list(d.members).length) });
      break;
    case "vault.renamed":
      text = tr("renamed a vault to “{name}”", { name: str(d.name) ?? "…" });
      break;
    case "vault.deleted":
      text = tr("deleted the vault “{name}”", { name: str(d.name) ?? vault ?? "…" });
      if (num(d.members) !== null)
        meta = tr("{members} lost access", { members: members(num(d.members) ?? 0) });
      break;
    case "vault.access_granted":
      text = tr("gave {target} access ({role})", { target, role: roleText(role) });
      break;
    case "vault.access_changed":
      text = tr("changed {target}'s access to {role}", { target, role: roleText(role) });
      if (prev) meta = wasRole(prev);
      break;
    case "vault.access_revoked":
      text = bool(d.self) ? tr("left the vault") : tr("removed {target}'s access", { target });
      if (prev) meta = wasRole(prev);
      break;
    case "vault.key_rotated": {
      const dropped = list(d.access_dropped).length;
      const version = num(d.key_version);
      text = version
        ? tr("rotated the vault key (v{version})", { version })
        : tr("rotated the vault key");
      meta = tr("re-sealed for {members}", { members: members(list(d.resealed_for).length) });
      if (dropped > 0) meta += `, ${tr("{count} lost access", { count: dropped })}`;
      break;
    }
    case "entity.created":
      text = tr("added {what}", { what: countOf(str(d.kind), num(d.count) ?? 1) });
      break;
    case "entity.updated":
      text = tr("updated {what}", { what: countOf(str(d.kind), num(d.count) ?? 1) });
      break;
    case "entity.deleted":
      text = tr("removed {what}", { what: countOf(str(d.kind), num(d.count) ?? 1) });
      break;
    case "multiplayer.started":
      text = tr("started a multiplayer session");
      break;
    case "multiplayer.joined":
      text = tr("joined a multiplayer session");
      break;
    case "multiplayer.stopped":
      text = tr("stopped a multiplayer session");
      break;
    default:
      text = ev.action.replace(".", ": ").replace(/_/g, " ");
  }
  return { actor, text, vault, meta };
}

/** Same day → time only; otherwise a short date + time in the local zone. */
export function formatWhen(iso: string, now = new Date()): string {
  const t = new Date(iso);
  if (Number.isNaN(t.getTime())) return "";
  const sameDay = t.toDateString() === now.toDateString();
  const time = t.toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" });
  if (sameDay) return time;
  const sameYear = t.getFullYear() === now.getFullYear();
  const date = t.toLocaleDateString(undefined, {
    day: "numeric",
    month: "short",
    ...(sameYear ? {} : { year: "numeric" }),
  });
  return `${date}, ${time}`;
}
