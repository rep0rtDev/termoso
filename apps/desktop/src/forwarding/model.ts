import type { PfKind, PfRuleCard, PfRuleForm, Uuid } from "@/ipc/types";
import { tr } from "@/i18n";

export const KIND_NAME: Record<PfKind, string> = {
  local: "Local",
  remote: "Remote",
  dynamic: "Dynamic",
};

export const KIND_LETTER: Record<PfKind, string> = { local: "L", remote: "R", dynamic: "D" };

export const KIND_ORDER: PfKind[] = ["local", "remote", "dynamic"];

export const DEFAULT_BIND = "127.0.0.1";

export function emptyRuleForm(vaultId: Uuid, kind: PfKind, hostId: Uuid | null): PfRuleForm {
  return {
    id: null,
    vaultId,
    label: "",
    hostId: hostId ?? "",
    kind,
    boundAddress: DEFAULT_BIND,
    localPort: 0,
    remoteHost: "",
    remotePort: 0,
    autoStart: false,
  };
}

export function ruleToForm(r: PfRuleCard): PfRuleForm {
  return {
    id: r.id,
    vaultId: r.vaultId,
    label: r.label,
    hostId: r.hostId,
    kind: r.kind,
    boundAddress: r.boundAddress,
    localPort: r.localPort,
    remoteHost: r.remoteHost,
    remotePort: r.remotePort,
    autoStart: r.autoStart,
  };
}

type Route = Pick<
  PfRuleForm,
  "kind" | "boundAddress" | "localPort" | "remoteHost" | "remotePort"
> & { hostLabel: string };

/** One-line route as Termius prints it under a card. */
export function routeLine(r: Route): string {
  const bind = r.boundAddress || DEFAULT_BIND;
  const host = r.hostLabel || "…";
  switch (r.kind) {
    case "local":
      return tr("Local:{localPort} → {host} → {remoteHost}:{remotePort}", {
        localPort: r.localPort,
        host,
        remoteHost: r.remoteHost,
        remotePort: r.remotePort,
      });
    case "remote":
      return tr("Port {remotePort} on {host} → This device → {remoteHost}:{localPort}", {
        remotePort: r.remotePort,
        host,
        remoteHost: r.remoteHost,
        localPort: r.localPort,
      });
    case "dynamic":
      return bind === DEFAULT_BIND
        ? tr("SOCKS proxy on local port {localPort} through {host}", {
            localPort: r.localPort,
            host,
          })
        : tr("SOCKS proxy on {bind}:{localPort} through {host}", {
            bind,
            localPort: r.localPort,
            host,
          });
  }
}

export function ruleTitle(r: PfRuleCard): string {
  return r.label || routeLine(r);
}

/** What the rule form still lacks before it can be saved. */
export function formProblem(f: PfRuleForm): string | null {
  if (!f.hostId)
    return f.kind === "remote"
      ? tr("Remote host is required")
      : tr("Intermediate host is required");
  if (f.kind === "remote") {
    if (f.remotePort <= 0) return tr("Remote port number is required");
    if (!f.remoteHost.trim()) return tr("Destination address is required");
    if (f.localPort <= 0) return tr("Destination port number is required");
    return null;
  }
  if (f.localPort <= 0) return tr("Local port number is required");
  if (f.kind === "local") {
    if (!f.remoteHost.trim()) return tr("Destination address is required");
    if (f.remotePort <= 0) return tr("Destination port number is required");
  }
  return null;
}

export function parsePort(s: string): number {
  const n = Number.parseInt(s, 10);
  return Number.isFinite(n) && n >= 0 && n <= 65535 ? n : 0;
}
