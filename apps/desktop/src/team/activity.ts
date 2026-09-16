import type { AuditEvent, Uuid } from "@/ipc/types";

/** Coarse groups for the action filter; each maps to an action prefix. */
export const ACTIVITY_GROUPS = [
  { value: "", label: "All activity" },
  { value: "team.", label: "Team" },
  { value: "member.", label: "Members" },
  { value: "invite.", label: "Invitations" },
  { value: "vault.", label: "Vaults & access" },
  { value: "entity.", label: "Shared data" },
  { value: "multiplayer.", label: "Multiplayer" },
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
  host: ["host", "hosts"],
  group: ["group", "groups"],
  ssh_key: ["SSH key", "SSH keys"],
  ssh_certificate: ["certificate", "certificates"],
  identity: ["identity", "identities"],
  known_host: ["known host", "known hosts"],
  snippet: ["snippet", "snippets"],
  snippet_package: ["snippet package", "snippet packages"],
  host_snippet: ["snippet target", "snippet targets"],
  pf_rule: ["port forwarding rule", "port forwarding rules"],
  proxy: ["proxy", "proxies"],
  host_chain: ["host chain", "host chains"],
  tag: ["tag", "tags"],
  tag_host: ["tag link", "tag links"],
  ssh_config: ["SSH config", "SSH configs"],
  telnet_config: ["Telnet config", "Telnet configs"],
  serial_config: ["serial config", "serial configs"],
  port_knocking: ["port knocking", "port knockings"],
  workspace: ["workspace", "workspaces"],
  workspace_template: ["workspace template", "workspace templates"],
  log_bookmark: ["log bookmark", "log bookmarks"],
  cloud_import: ["cloud import", "cloud imports"],
};

const ROLE_LABEL: Record<string, string> = {
  viewer: "can view",
  editor: "can edit",
  manager: "can manage",
  owner: "Owner",
  admin: "Admin",
  member: "Member",
};

const str = (v: unknown): string | null => (typeof v === "string" && v.length > 0 ? v : null);
const num = (v: unknown): number | null => (typeof v === "number" ? v : null);
const bool = (v: unknown): boolean | null => (typeof v === "boolean" ? v : null);
const list = (v: unknown): unknown[] => (Array.isArray(v) ? v : []);

const countOf = (kind: string | null, count: number) => {
  const [one, many] = KIND_LABEL[kind ?? ""] ?? [kind ?? "item", `${kind ?? "item"}s`];
  const article = /^(?:[aeiou]|SSH|SFTP)/i.test(one) ? "an" : "a";
  return count === 1 ? `${article} ${one}` : `${count} ${many}`;
};

/** Turn one audit row into a readable sentence. Never echoes secrets: the
 *  server only stores metadata, and this only reads known keys from it. */
export function describeEvent(ev: AuditEvent, ctx: ActivityContext): ActivityLine {
  const d = ev.details;
  const actor = ev.actor_name ?? ev.actor_email ?? "Someone";
  const target =
    ev.target_email ??
    (ev.target_user ? (ctx.people.get(ev.target_user) ?? "a member") : null) ??
    str(d.email) ??
    "a member";
  const self = ev.target_user !== undefined && ev.target_user === ev.actor_id;
  const vault = ev.vault_id ? (ctx.vaultNames.get(ev.vault_id) ?? str(d.name) ?? "a vault") : null;
  const role = str(d.role);
  const prev = str(d.previous_role);
  const roleText = (r: string | null) => (r ? (ROLE_LABEL[r] ?? r) : "");

  let text: string;
  let meta: string | null = null;
  switch (ev.action) {
    case "team.created":
      text = `created the team${str(d.name) ? ` “${str(d.name)}”` : ""}`;
      break;
    case "team.renamed":
      text = `renamed the team to “${str(d.name) ?? "…"}”`;
      break;
    case "team.settings": {
      const mp = bool(d.multiplayer_enabled);
      const mfa = bool(d.require_mfa);
      text =
        mp !== null
          ? `${mp ? "enabled" : "disabled"} Multiplayer`
          : mfa !== null
            ? `${mfa ? "required" : "stopped requiring"} 2FA for the team`
            : "changed team settings";
      break;
    }
    case "member.role":
      text = `changed ${self ? "their own" : `${target}'s`} role to ${roleText(role)}`;
      if (prev) meta = `was ${roleText(prev)}`;
      break;
    case "member.removed":
      text = `removed ${target} from the team`;
      if (prev) meta = `was ${roleText(prev)}`;
      else if (str(d.account) === "converted") meta = "account converted to individual";
      break;
    case "member.account_deleted":
      text = `deleted ${target}'s account`;
      if (role) meta = `was ${roleText(role)}`;
      break;
    case "member.left":
      text = "left the team";
      if (str(d.account) === "converted") meta = "account converted to individual";
      break;
    case "invite.created":
      text = `invited ${str(d.email) ?? "someone"}${role ? ` as ${roleText(role)}` : ""}`;
      if (list(d.vault_ids).length > 0)
        meta = `with access to ${list(d.vault_ids).length} vault(s)`;
      break;
    case "invite.revoked":
      text = `revoked the invitation for ${str(d.email) ?? "someone"}`;
      break;
    case "invite.accepted":
      text = `joined the team${role ? ` as ${roleText(role)}` : ""}`;
      break;
    case "vault.created":
      text = `created the vault “${vault ?? "…"}”`;
      if (list(d.members).length > 0) meta = `shared with ${list(d.members).length} member(s)`;
      break;
    case "vault.renamed":
      text = `renamed a vault to “${str(d.name) ?? "…"}”`;
      break;
    case "vault.deleted":
      text = `deleted the vault “${str(d.name) ?? vault ?? "…"}”`;
      if (num(d.members) !== null) meta = `${num(d.members)} member(s) lost access`;
      break;
    case "vault.access_granted":
      text = `gave ${target} access (${roleText(role)})`;
      break;
    case "vault.access_changed":
      text = `changed ${target}'s access to ${roleText(role)}`;
      if (prev) meta = `was ${roleText(prev)}`;
      break;
    case "vault.access_revoked":
      text = bool(d.self) ? "left the vault" : `removed ${target}'s access`;
      if (prev) meta = `was ${roleText(prev)}`;
      break;
    case "vault.key_rotated": {
      const dropped = list(d.access_dropped).length;
      text = `rotated the vault key${num(d.key_version) ? ` (v${num(d.key_version)})` : ""}`;
      meta = `re-sealed for ${list(d.resealed_for).length} member(s)${
        dropped > 0 ? `, ${dropped} lost access` : ""
      }`;
      break;
    }
    case "entity.created":
      text = `added ${countOf(str(d.kind), num(d.count) ?? 1)}`;
      break;
    case "entity.updated":
      text = `updated ${countOf(str(d.kind), num(d.count) ?? 1)}`;
      break;
    case "entity.deleted":
      text = `removed ${countOf(str(d.kind), num(d.count) ?? 1)}`;
      break;
    case "multiplayer.started":
      text = "started a multiplayer session";
      break;
    case "multiplayer.joined":
      text = "joined a multiplayer session";
      break;
    case "multiplayer.stopped":
      text = "stopped a multiplayer session";
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
