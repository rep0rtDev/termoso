import { useState } from "react";
import { Box, Button, Collapse, Stack, Typography, keyframes } from "@mui/material";
import CheckRoundedIcon from "@mui/icons-material/CheckRounded";
import CloseRoundedIcon from "@mui/icons-material/CloseRounded";
import PriorityHighRoundedIcon from "@mui/icons-material/PriorityHighRounded";
import type { ConnectPhase, HostCard, SessionInfo } from "@/ipc/types";
import { useHosts } from "@/ipc/hooks";
import { useSnackbar } from "@/components/Snackbar";
import { IconTile } from "@/components/ui";
import { HostGlyph, hostIcon } from "@/hosts/HostAvatar";
import { hostTarget } from "@/hosts/HostGrid";
import { requestEditHost } from "@/app/navigation";
import { monoFontFamily } from "@/theme/theme";
import { closePane, copyText, reconnectPane, type Pane } from "./store";

interface Stage {
  key: ConnectPhase["kind"];
  label: string;
}

const STAGES: Stage[] = [
  { key: "resolving", label: "Resolve" },
  { key: "connecting", label: "Connect" },
  { key: "handshake", label: "Key exchange" },
  { key: "host_key", label: "Host key" },
  { key: "auth", label: "Authenticate" },
  { key: "authenticated", label: "Shell" },
];

const pulse = keyframes`
  0%, 100% { opacity: 1; transform: scale(1); }
  50% { opacity: 0.55; transform: scale(0.8); }
`;

function statusLine(pane: Pane): string {
  const p = pane.progress;
  if (pane.status === "error") return pane.message ?? "Connection failed";
  if (pane.status === "closed") return pane.message ?? "Cancelled";
  if (!p) {
    if (pane.protocol === "local" || pane.target.kind === "local") return "Starting shell…";
    return pane.protocol === "serial" ? "Opening device…" : "Connecting…";
  }
  const where = p.hop ? ` through ${p.hop}` : "";
  switch (p.phase.kind) {
    case "resolving":
      return `Resolving address${where}…`;
    case "connecting":
      return `Connecting via ${p.phase.via}${where}…`;
    case "handshake":
      return `Negotiating encryption${where}…`;
    case "host_key":
      return "Waiting for you to verify the host key";
    case "auth":
      return `Authenticating with ${p.phase.method}${where}…`;
    case "authenticated":
      return "Authenticated — opening shell…";
  }
}

/** Protocol the pane is (or will be) speaking: reported by the session, else from the target. */
function paneProtocol(pane: Pane, host: HostCard | undefined): SessionInfo["protocol"] {
  if (pane.protocol) return pane.protocol;
  switch (pane.target.kind) {
    case "local":
      return "local";
    case "serial":
      return "serial";
    case "quick":
      return pane.target.protocol ?? "ssh";
    case "host":
      return pane.target.protocol ?? host?.protocol ?? "ssh";
  }
}

function protocolLabel(pane: Pane, host: HostCard | undefined): string {
  const proto = paneProtocol(pane, host).toUpperCase();
  if (pane.via.length) return `${proto} · via ${pane.via.join(" → ")}`;
  return proto;
}

const time = (ms: number) =>
  new Date(ms).toLocaleTimeString(undefined, {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });

/**
 * Full-pane view of a connection attempt: who we are connecting to, how far
 * the SSH handshake got, and the log of what happened — plus what to do next
 * when it fails.
 */
export function ConnectionView({ pane }: { pane: Pane }) {
  const snackbar = useSnackbar();
  const hosts = useHosts(null);
  const host = pane.hostId ? hosts.data?.find((h) => h.id === pane.hostId) : undefined;
  const failed = pane.status === "error" || pane.status === "closed";
  const [logPref, setLogPref] = useState<boolean | null>(null);
  const logOpen = logPref ?? failed;

  const current = pane.progress
    ? STAGES.findIndex((s) => s.key === pane.progress?.phase.kind)
    : pane.status === "connecting"
      ? -1
      : 0;
  const proto = paneProtocol(pane, host);
  const stepper = proto === "ssh";
  const brand = hostIcon(host)?.color;
  const title = host?.label ?? pane.title;
  const address = pane.subtitle || (host ? hostTarget(host, proto) : "");

  const copyLogs = () =>
    void copyText(pane.log.map((l) => `${time(l.at)}  ${l.text}`).join("\n")).then(() =>
      snackbar.notify("Logs copied"),
    );

  const stageLabel = current >= 0 && current < STAGES.length ? STAGES[current]?.label : undefined;

  return (
    <Box
      sx={{
        position: "absolute",
        inset: 0,
        zIndex: 6,
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        overflowY: "auto",
        overflowX: "hidden",
        bgcolor: "surface.base",
        color: "text.primary",
        p: 3,
      }}
    >
      <Stack spacing={2.5} sx={{ width: 440, maxWidth: "100%", minWidth: 0, my: "auto" }}>
        <Stack direction="row" spacing={1.5} sx={{ alignItems: "center", minWidth: 0 }}>
          <IconTile
            size={48}
            tone={failed ? "danger" : "neutral"}
            color={failed ? undefined : brand}
          >
            {failed ? (
              <PriorityHighRoundedIcon />
            ) : (
              <HostGlyph osName={host?.osName} icon={host?.icon} protocol={proto} />
            )}
          </IconTile>
          <Box sx={{ minWidth: 0, flex: 1 }}>
            <Typography
              variant="subtitle1"
              noWrap
              title={title}
              sx={{ fontWeight: 600, lineHeight: 1.3 }}
            >
              {title}
            </Typography>
            <Typography
              variant="body2"
              color="text.secondary"
              noWrap
              title={address}
              sx={{ display: "block" }}
            >
              {protocolLabel(pane, host)}
              {address && address !== title ? ` ${address}` : ""}
            </Typography>
          </Box>
          <Button
            variant="tonal"
            onClick={() => setLogPref(!logOpen)}
            disabled={!pane.log.length}
            sx={{ flexShrink: 0 }}
          >
            {logOpen ? "Hide logs" : "Show logs"}
          </Button>
        </Stack>

        {stepper && (
          <Box sx={{ px: 0.5, minWidth: 0 }}>
            <Box sx={{ position: "relative", height: 20, display: "flex", alignItems: "center" }}>
              <Box
                sx={{
                  position: "absolute",
                  left: 8,
                  right: 8,
                  height: 2,
                  borderRadius: 1,
                  bgcolor: "border.strong",
                }}
              />
              <Box
                sx={{
                  position: "absolute",
                  left: 8,
                  height: 2,
                  borderRadius: 1,
                  width: `calc((100% - 16px) * ${Math.max(0, current) / (STAGES.length - 1)})`,
                  bgcolor: failed ? "error.main" : "primary.main",
                  transition: "width 240ms ease",
                }}
              />
              <Box
                sx={{
                  position: "relative",
                  width: "100%",
                  display: "flex",
                  justifyContent: "space-between",
                }}
              >
                {STAGES.map((s, i) => {
                  const done = i < current || pane.status === "connected";
                  const active = i === current && !failed;
                  const bad = i === current && failed;
                  return (
                    <Box
                      key={s.key}
                      sx={{
                        width: 16,
                        height: 16,
                        borderRadius: "50%",
                        display: "grid",
                        placeItems: "center",
                        bgcolor: bad
                          ? "error.main"
                          : done || active
                            ? "primary.main"
                            : "border.strong",
                        color: "#fff",
                        boxShadow: "0 0 0 3px var(--mui-palette-surface-base)",
                        animation: active ? `${pulse} 1.2s ease-in-out infinite` : "none",
                      }}
                    >
                      {done && <CheckRoundedIcon sx={{ fontSize: 11 }} />}
                      {bad && <CloseRoundedIcon sx={{ fontSize: 11 }} />}
                    </Box>
                  );
                })}
              </Box>
            </Box>
            {stageLabel && (
              <Typography
                variant="caption"
                noWrap
                sx={{
                  display: "block",
                  mt: 0.75,
                  textAlign: "center",
                  fontWeight: 600,
                  color: failed ? "error.main" : "text.secondary",
                }}
              >
                {pane.status === "connected" ? "Connected" : stageLabel}
                <Box component="span" sx={{ color: "text.disabled", fontWeight: 400 }}>
                  {` · step ${current + 1} of ${STAGES.length}`}
                </Box>
              </Typography>
            )}
          </Box>
        )}

        <Typography
          variant="body2"
          color={failed ? "error.main" : "text.primary"}
          sx={{ minWidth: 0, overflowWrap: "anywhere", whiteSpace: "pre-wrap" }}
        >
          {statusLine(pane)}
        </Typography>

        <Stack spacing={1.5} sx={{ minWidth: 0 }}>
          <Stack direction="row" spacing={1} useFlexGap sx={{ flexWrap: "wrap" }}>
            {failed ? (
              <>
                <Button variant="tonal" onClick={() => void closePane(pane.id)}>
                  Close
                </Button>
                <Button variant="contained" onClick={() => void reconnectPane(pane.id)}>
                  Start over
                </Button>
                {pane.hostId && (
                  <Button
                    variant="text"
                    color="inherit"
                    onClick={() => {
                      const id = pane.hostId;
                      if (id) requestEditHost(id);
                    }}
                  >
                    Edit host
                  </Button>
                )}
                <Button
                  variant="text"
                  color="inherit"
                  onClick={copyLogs}
                  disabled={!pane.log.length}
                >
                  Copy logs
                </Button>
              </>
            ) : (
              <Button variant="tonal" onClick={() => void closePane(pane.id)}>
                Cancel
              </Button>
            )}
          </Stack>

          <Collapse in={logOpen && pane.log.length > 0}>
            <Box
              sx={{
                borderRadius: 2,
                bgcolor: "surface.lowest",
                p: 1.5,
                maxHeight: 180,
                minWidth: 0,
                overflowY: "auto",
                overflowX: "hidden",
                fontFamily: monoFontFamily,
                fontSize: 12,
                lineHeight: 1.6,
              }}
            >
              {pane.log.map((l, i) => (
                <Box
                  key={i}
                  sx={{
                    display: "flex",
                    gap: 1.5,
                    color: l.level === "error" ? "error.main" : "text.secondary",
                  }}
                >
                  <Box component="span" sx={{ color: "text.disabled", flexShrink: 0 }}>
                    {time(l.at)}
                  </Box>
                  <Box
                    component="span"
                    sx={{ minWidth: 0, overflowWrap: "anywhere", whiteSpace: "pre-wrap" }}
                  >
                    {l.text}
                  </Box>
                </Box>
              ))}
            </Box>
          </Collapse>
        </Stack>
      </Stack>
    </Box>
  );
}
