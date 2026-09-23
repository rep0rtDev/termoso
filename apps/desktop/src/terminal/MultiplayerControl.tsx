import { useState, type MouseEvent } from "react";
import { copyToClipboard } from "@/lib/clipboard";
import {
  Box,
  Button,
  Chip,
  Divider,
  IconButton,
  Popover,
  Stack,
  Tooltip,
  Typography,
  alpha,
} from "@mui/material";
import LinkRoundedIcon from "@mui/icons-material/LinkRounded";
import KeyboardAltRoundedIcon from "@mui/icons-material/KeyboardAltRounded";
import KeyboardAltOutlinedIcon from "@mui/icons-material/KeyboardAltOutlined";
import ScreenShareRoundedIcon from "@mui/icons-material/ScreenShareRounded";
import { useAccount, useTeams } from "@/ipc/hooks";
import { errorMessage, type LiveParticipant, type ShareInfo, type Uuid } from "@/ipc/types";
import { useSnackbar } from "@/components/Snackbar";
import { PersonAvatar, initialsOf } from "@/team/PersonAvatar";
import { goToSettings } from "@/app/navigation";
import {
  isMultiplayerDisabled,
  isNotSignedIn,
  setControl,
  startShare,
  stopShare,
  useShare,
} from "./multiplayer";
import { closePane, useTerminal } from "./store";
import { tr } from "@/i18n";

/**
 * Multiplayer control on a terminal tab, like Termius: a small screen icon
 * that turns green while the tab is shared and opens the Multiplayer popover
 * (Copy link / participants / Stop multiplayer).
 */
export function MultiplayerTabButton({ paneId }: { paneId: Uuid }) {
  const share = useShare(paneId);
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);
  const live = share !== null;
  return (
    <>
      <Tooltip title={live ? tr("Multiplayer") : tr("Share this terminal")}>
        <IconButton
          className="tab-multiplayer"
          aria-label={tr("Multiplayer")}
          onClick={(e: MouseEvent<HTMLElement>) => {
            e.stopPropagation();
            setAnchor(e.currentTarget);
          }}
          onDoubleClick={(e) => e.stopPropagation()}
          sx={{
            width: 22,
            height: 22,
            borderRadius: 1,
            color: live ? "primary.contrastText" : "text.secondary",
            bgcolor: live ? "primary.main" : "transparent",
            "&:hover": {
              bgcolor: live ? "primary.dark" : "action.hover",
              color: live ? "primary.contrastText" : "text.primary",
            },
          }}
        >
          <ScreenShareRoundedIcon sx={{ fontSize: 14 }} />
        </IconButton>
      </Tooltip>
      <MultiplayerPopover paneId={paneId} anchor={anchor} onClose={() => setAnchor(null)} />
    </>
  );
}

const WIDTH = 392;

export function MultiplayerPopover({
  paneId,
  anchor,
  onClose,
}: {
  paneId: Uuid;
  anchor: HTMLElement | null;
  onClose: () => void;
}) {
  const share = useShare(paneId);
  const pane = useTerminal((s) => s.panes[paneId]);
  const account = useAccount();
  const signedIn = Boolean(account.data?.account);
  const teams = useTeams(signedIn);
  const blockedByTeam = teams.data?.find((t) => !t.multiplayer_enabled) ?? null;
  const snackbar = useSnackbar();
  const [busy, setBusy] = useState(false);
  const [blocked, setBlocked] = useState<"team" | "signin" | null>(null);

  const copy = (link: string) =>
    copyToClipboard(link)
      .then(() => snackbar.notify(tr("Link copied")))
      .catch(() => snackbar.error(tr("Clipboard is not available")));

  const copyLink = async () => {
    if (share?.link) {
      await copy(share.link);
      return;
    }
    setBusy(true);
    try {
      const info = await startShare(paneId);
      if (info.link) await copy(info.link);
    } catch (e) {
      if (isMultiplayerDisabled(e)) setBlocked("team");
      else if (isNotSignedIn(e)) setBlocked("signin");
      else snackbar.error(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  const stop = async () => {
    setBusy(true);
    try {
      await stopShare(paneId);
      onClose();
    } catch (e) {
      snackbar.error(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  const connected = pane?.status === "connected";
  const reason: "team" | "signin" | null =
    blocked ??
    (!signedIn && account.isSuccess ? "signin" : blockedByTeam && !share ? "team" : null);

  return (
    <Popover
      open={Boolean(anchor)}
      anchorEl={anchor}
      onClose={onClose}
      anchorOrigin={{ vertical: "bottom", horizontal: "left" }}
      transformOrigin={{ vertical: -8, horizontal: 48 }}
      slotProps={{ paper: { sx: { width: WIDTH, p: 1 } } }}
    >
      <Stack direction="row" spacing={1} sx={{ alignItems: "center", px: 0.5, minHeight: 32 }}>
        <ScreenShareRoundedIcon sx={{ fontSize: 16, color: "text.secondary" }} />
        <Typography variant="body2" sx={{ flex: 1, fontWeight: 500 }}>
          {tr("Multiplayer")}
        </Typography>
        {share?.role === "viewer" ? (
          <Button
            size="small"
            color="inherit"
            disabled={busy}
            onClick={() => {
              onClose();
              void closePane(paneId);
            }}
            sx={{ bgcolor: "action.selected" }}
          >
            {tr("Leave")}
          </Button>
        ) : (
          <>
            <Button
              size="small"
              color="inherit"
              startIcon={<LinkRoundedIcon sx={{ fontSize: "16px !important" }} />}
              disabled={busy || !connected || reason !== null}
              onClick={() => void copyLink()}
              sx={{ bgcolor: "action.selected" }}
            >
              {tr("Copy link")}
            </Button>
            {share && (
              <Button
                size="small"
                disabled={busy}
                onClick={() => void stop()}
                sx={{
                  color: "error.main",
                  bgcolor: (t) => alpha(t.palette.error.main, 0.16),
                  "&:hover": { bgcolor: (t) => alpha(t.palette.error.main, 0.26) },
                }}
              >
                {tr("Stop multiplayer")}
              </Button>
            )}
          </>
        )}
      </Stack>
      <Divider sx={{ my: 1 }} />
      {share ? (
        <Participants share={share} paneId={paneId} />
      ) : reason === "signin" ? (
        <Hint
          text={tr("Sign in to a Termoso account to share this terminal session.")}
          action={tr("Sign in")}
          onAction={() => {
            onClose();
            goToSettings("account");
          }}
        />
      ) : reason === "team" ? (
        <Hint
          text={`Multiplayer is turned off for ${blockedByTeam?.name ?? "your team"}.`}
          action={
            blockedByTeam &&
            (blockedByTeam.my_role === "owner" || blockedByTeam.my_role === "admin")
              ? tr("Team settings")
              : undefined
          }
          onAction={() => {
            onClose();
            goToSettings("team");
          }}
        />
      ) : !connected ? (
        <Hint text={tr("Connect first — a live session is needed to share this terminal.")} />
      ) : (
        <Hint
          text={tr(
            "Copy link to share this terminal session.\nPeople who join will be visible here",
          )}
        />
      )}
    </Popover>
  );
}

function Hint({
  text,
  action,
  onAction,
}: {
  text: string;
  action?: string;
  onAction?: () => void;
}) {
  return (
    <Stack spacing={1} sx={{ alignItems: "center", px: 2, py: 1.5 }}>
      <Typography
        variant="body2"
        color="text.secondary"
        sx={{ textAlign: "center", whiteSpace: "pre-line", lineHeight: 1.5 }}
      >
        {text}
      </Typography>
      {action && onAction && (
        <Button size="small" variant="outlined" color="inherit" onClick={onAction}>
          {action}
        </Button>
      )}
    </Stack>
  );
}

function Participants({ share, paneId }: { share: ShareInfo; paneId: Uuid }) {
  const snackbar = useSnackbar();
  const toggle = (p: LiveParticipant) =>
    setControl(paneId, p.userId, !p.canWrite).catch((e: unknown) =>
      snackbar.error(errorMessage(e)),
    );
  return (
    <Box sx={{ px: 0.5, pb: 0.5 }}>
      <Typography variant="body2" color="text.secondary" sx={{ px: 0.5, mb: 0.75 }}>
        {tr("Participants:")}
      </Typography>
      <Stack spacing={0.25}>
        {share.participants.map((p) => (
          <ParticipantRow
            key={p.userId}
            p={p}
            hostView={share.role === "host"}
            onToggle={share.role === "host" && !p.isHost ? () => void toggle(p) : undefined}
          />
        ))}
        {share.participants.length <= 1 && (
          <Typography variant="caption" color="text.secondary" sx={{ px: 0.5, pt: 0.5 }}>
            {tr("People who join will be visible here")}
          </Typography>
        )}
      </Stack>
    </Box>
  );
}

function displayLabel(p: LiveParticipant): string {
  const name = p.displayName?.trim() ?? "";
  return name.length > 0 ? name : p.email;
}

function ParticipantRow({
  p,
  hostView,
  onToggle,
}: {
  p: LiveParticipant;
  hostView: boolean;
  onToggle?: () => void;
}) {
  const label = displayLabel(p);
  const controlTitle = p.isHost
    ? tr("Host — has control")
    : p.canWrite
      ? hostView
        ? tr("Has remote control — click to take it back")
        : tr("Has remote control")
      : hostView
        ? tr("Watching — click to give remote control")
        : tr("Watching");
  const icon = p.canWrite ? (
    <KeyboardAltRoundedIcon sx={{ fontSize: 15 }} />
  ) : (
    <KeyboardAltOutlinedIcon sx={{ fontSize: 15 }} />
  );
  return (
    <Stack
      direction="row"
      spacing={1.25}
      sx={{ alignItems: "center", px: 0.5, py: 0.5, borderRadius: 1.5, bgcolor: "action.hover" }}
    >
      <Box sx={{ position: "relative", display: "flex" }}>
        <PersonAvatar
          label={initialsOf(p.displayName, p.email)}
          seed={p.email}
          size={28}
          userId={p.userId}
          avatar={p.avatar}
        />
        <Box
          sx={{
            position: "absolute",
            right: -2,
            top: -2,
            width: 8,
            height: 8,
            borderRadius: "50%",
            bgcolor: "success.main",
            border: "1.5px solid",
            borderColor: "background.paper",
          }}
        />
      </Box>
      <Box sx={{ flex: 1, minWidth: 0 }}>
        <Typography variant="body2" noWrap>
          {label}
        </Typography>
        {label !== p.email && (
          <Typography variant="caption" color="text.secondary" noWrap sx={{ display: "block" }}>
            {p.email}
          </Typography>
        )}
      </Box>
      {p.isMe && (
        <Chip label={tr("You")} size="small" sx={{ height: 20, fontSize: 11, fontWeight: 600 }} />
      )}
      <Tooltip title={controlTitle}>
        <span>
          <IconButton
            size="small"
            disabled={!onToggle}
            onClick={onToggle}
            sx={{
              width: 24,
              height: 24,
              borderRadius: 1,
              color: p.canWrite ? "info.main" : "text.disabled",
              bgcolor: (t) => (p.canWrite ? alpha(t.palette.info.main, 0.16) : "transparent"),
              "&.Mui-disabled": { color: p.canWrite ? "info.main" : "text.disabled" },
            }}
          >
            {icon}
          </IconButton>
        </span>
      </Tooltip>
    </Stack>
  );
}
