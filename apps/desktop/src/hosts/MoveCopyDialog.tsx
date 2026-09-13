import { useState, type ReactNode } from "react";
import {
  Box,
  Button,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  List,
  ListItemButton,
  ListItemIcon,
  ListItemText,
  Radio,
  Typography,
} from "@mui/material";
import FolderRoundedIcon from "@mui/icons-material/FolderRounded";
import HomeRoundedIcon from "@mui/icons-material/HomeRounded";
import PersonRoundedIcon from "@mui/icons-material/PersonRounded";
import GroupsRoundedIcon from "@mui/icons-material/GroupsRounded";
import { useSnackbar } from "@/components/Snackbar";
import { useCopyHostsToVault, useGroups, useMoveHosts, useVaults } from "@/ipc/hooks";
import { errorMessage, type HostCard, type Uuid } from "@/ipc/types";
import { vaultHint, vaultIcon } from "@/app/vault";
import { groupPathLabel } from "./GroupPanel";

export type MoveCopyRequest =
  | { kind: "group"; hosts: HostCard[] }
  | {
      kind: "vault";
      hosts: HostCard[];
      move: boolean;
      /** Destination already picked (from the `Copy to ▸` submenu): skip the vault list. */
      target?: Uuid;
    };

/** "Move to…" (another group) / "Copy to…" / "Move to vault…" for one or many hosts. */
export function MoveCopyDialog({
  request,
  vaultId,
  onClose,
  onDone,
}: {
  request: MoveCopyRequest | null;
  vaultId: Uuid;
  onClose: () => void;
  onDone: () => void;
}) {
  return (
    <Dialog open={request !== null} onClose={onClose} maxWidth="xs" fullWidth>
      {request && <Body request={request} vaultId={vaultId} onClose={onClose} onDone={onDone} />}
    </Dialog>
  );
}

function Body({
  request,
  vaultId,
  onClose,
  onDone,
}: {
  request: MoveCopyRequest;
  vaultId: Uuid;
  onClose: () => void;
  onDone: () => void;
}) {
  const snackbar = useSnackbar();
  const groups = useGroups(vaultId);
  const vaults = useVaults();
  const moveHosts = useMoveHosts();
  const copyToVault = useCopyHostsToVault();
  /** Chosen destination: a group/vault id, `""` for the top level, `null` until picked. */
  const [target, setTarget] = useState<string | null>(
    request.kind === "vault" ? (request.target ?? null) : null,
  );
  /** Second step for team vaults: do the credentials travel with the hosts? */
  const [credStep, setCredStep] = useState(request.kind === "vault" && !!request.target);
  const [shared, setShared] = useState(false);

  const ids = request.hosts.map((h) => h.id);
  const count = ids.length;
  const what = count === 1 ? `“${request.hosts[0]?.label ?? ""}”` : `${count} hosts`;
  const busy = moveHosts.isPending || copyToVault.isPending;

  const commonGroup = request.hosts.every((h) => h.groupId === request.hosts[0]?.groupId)
    ? (request.hosts[0]?.groupId ?? "")
    : null;

  const targetVault = (vaults.data ?? []).find((v) => v.id === target);
  const toTeam = request.kind === "vault" && targetVault?.kind === "team";
  const move = request.kind === "vault" && request.move;

  const run = () => {
    if (target === null) return;
    if (toTeam && !credStep) {
      setCredStep(true);
      return;
    }
    const done = (msg: string) => {
      snackbar.notify(msg);
      onDone();
      onClose();
    };
    const fail = (e: unknown) => snackbar.error(errorMessage(e));
    if (request.kind === "group") {
      const groupId = target === "" ? null : target;
      const name = groupId ? groupPathLabel(groups.data ?? [], groupId) : "All hosts";
      moveHosts.mutate(
        { ids, groupId, vaultId },
        { onSuccess: () => done(`Moved ${what} to ${name}`), onError: fail },
      );
    } else if (target !== "") {
      const name = (vaults.data ?? []).find((v) => v.id === target)?.name ?? "vault";
      const withCredentials = !toTeam || shared;
      copyToVault.mutate(
        { ids, vaultId: target, move, withCredentials },
        {
          onSuccess: () =>
            done(
              `${move ? "Moved" : "Copied"} ${what} to ${name}${
                withCredentials ? "" : " without credentials"
              }`,
            ),
          onError: fail,
        },
      );
    }
  };

  const title =
    request.kind === "group"
      ? `Move ${what} to…`
      : move
        ? `Move ${what} to vault…`
        : `Copy ${what} to vault…`;

  const otherVaults = (vaults.data ?? []).filter((v) => v.id !== vaultId);

  if (credStep && targetVault) {
    const preset = request.kind === "vault" && !!request.target;
    return (
      <>
        <DialogTitle>
          {move ? "Move" : "Copy"} {what} to {targetVault.name}
        </DialogTitle>
        <DialogContent>
          <Typography variant="body2" color="text.secondary" sx={{ mb: 1.5 }}>
            Everyone with access to {targetVault.name} will see{" "}
            {count === 1 ? "this host" : "these hosts"}. How should they connect?
          </Typography>
          <Box sx={{ display: "flex", flexDirection: "column", gap: 1 }}>
            <CredentialsChoice
              selected={shared}
              onSelect={() => setShared(true)}
              icon={<GroupsRoundedIcon fontSize="small" />}
              title="Members share one set of credentials"
              text="Your username, password and keys are copied into the team vault, re-encrypted for its members."
            />
            <CredentialsChoice
              selected={!shared}
              onSelect={() => setShared(false)}
              icon={<PersonRoundedIcon fontSize="small" />}
              title="Members use their own credentials"
              text="Hosts arrive without a username, password or key; each member connects with credentials from their personal vault."
            />
          </Box>
        </DialogContent>
        <DialogActions sx={{ px: 3, pb: 2 }}>
          <Button
            color="inherit"
            onClick={() => (preset ? onClose() : setCredStep(false))}
            disabled={busy}
          >
            {preset ? "Cancel" : "Back"}
          </Button>
          <Button variant="contained" onClick={run} disabled={busy}>
            {busy ? "Working…" : move ? "Move" : "Copy"}
          </Button>
        </DialogActions>
      </>
    );
  }

  return (
    <>
      <DialogTitle>{title}</DialogTitle>
      <DialogContent sx={{ px: 1.5 }}>
        {request.kind === "group" ? (
          <List dense disablePadding>
            <ListItemButton
              selected={target === ""}
              disabled={commonGroup === ""}
              onClick={() => setTarget("")}
              sx={{ borderRadius: 1.5 }}
            >
              <ListItemIcon sx={{ minWidth: 32 }}>
                <HomeRoundedIcon fontSize="small" />
              </ListItemIcon>
              <ListItemText primary="All hosts" secondary="Top level" />
            </ListItemButton>
            {(groups.data ?? [])
              .map((g) => ({ g, path: groupPathLabel(groups.data ?? [], g.id) }))
              .sort((a, b) => a.path.localeCompare(b.path))
              .map(({ g, path }) => (
                <ListItemButton
                  key={g.id}
                  selected={target === g.id}
                  disabled={commonGroup === g.id}
                  onClick={() => setTarget(g.id)}
                  sx={{ borderRadius: 1.5 }}
                >
                  <ListItemIcon sx={{ minWidth: 32 }}>
                    <FolderRoundedIcon fontSize="small" />
                  </ListItemIcon>
                  <ListItemText
                    primary={g.label}
                    secondary={path.includes(" / ") ? path : undefined}
                  />
                </ListItemButton>
              ))}
          </List>
        ) : otherVaults.length === 0 ? (
          <Typography variant="body2" color="text.secondary" sx={{ px: 1.5, py: 1 }}>
            No other vaults on this device. Sign in and enable sync to get a personal vault, or join
            a team to share hosts.
          </Typography>
        ) : (
          <List dense disablePadding>
            {otherVaults.map((v) => (
              <ListItemButton
                key={v.id}
                selected={target === v.id}
                disabled={!v.unlocked || v.role === "viewer"}
                onClick={() => setTarget(v.id)}
                sx={{ borderRadius: 1.5 }}
              >
                <ListItemIcon sx={{ minWidth: 32 }}>{vaultIcon(v)}</ListItemIcon>
                <ListItemText primary={v.name} secondary={vaultHint(v)} />
              </ListItemButton>
            ))}
          </List>
        )}
        {request.kind === "vault" && otherVaults.length > 0 && (
          <Typography
            variant="caption"
            color="text.secondary"
            sx={{ display: "block", px: 1.5, pt: 1 }}
          >
            Inline credentials, keys, tags and group defaults travel with the hosts. Shared
            identities, proxies and jump hosts from this vault are copied as inline settings or
            dropped when they cannot be shared.
          </Typography>
        )}
      </DialogContent>
      <DialogActions sx={{ px: 3, pb: 2 }}>
        <Button color="inherit" onClick={onClose} disabled={busy}>
          Cancel
        </Button>
        <Button variant="contained" onClick={run} disabled={busy || target === null}>
          {busy
            ? "Working…"
            : toTeam
              ? "Next"
              : request.kind === "group" || request.move
                ? "Move"
                : "Copy"}
        </Button>
      </DialogActions>
    </>
  );
}

function CredentialsChoice({
  selected,
  onSelect,
  icon,
  title,
  text,
}: {
  selected: boolean;
  onSelect: () => void;
  icon: ReactNode;
  title: string;
  text: string;
}) {
  return (
    <Box
      role="radio"
      aria-checked={selected}
      tabIndex={0}
      onClick={onSelect}
      onKeyDown={(e) => {
        if (e.key === " " || e.key === "Enter") onSelect();
      }}
      sx={{
        display: "flex",
        alignItems: "flex-start",
        gap: 1,
        p: 1.25,
        pr: 1.5,
        borderRadius: 2,
        cursor: "pointer",
        bgcolor: selected ? "surface.strong" : "surface.high",
        outline: "1px solid",
        outlineColor: selected ? "primary.main" : "transparent",
        outlineOffset: -1,
        "&:hover": { bgcolor: selected ? "surface.strong" : "surface.highest" },
      }}
    >
      <Radio checked={selected} size="small" sx={{ p: 0.25, mt: -0.25 }} tabIndex={-1} />
      <Box sx={{ flex: 1, minWidth: 0 }}>
        <Box sx={{ display: "flex", alignItems: "center", gap: 0.75 }}>
          {icon}
          <Typography variant="body2" sx={{ fontWeight: 600 }}>
            {title}
          </Typography>
        </Box>
        <Typography variant="caption" color="text.secondary" sx={{ display: "block", mt: 0.25 }}>
          {text}
        </Typography>
      </Box>
    </Box>
  );
}
