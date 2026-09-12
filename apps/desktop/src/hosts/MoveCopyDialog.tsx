import { useState } from "react";
import {
  Button,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  List,
  ListItemButton,
  ListItemIcon,
  ListItemText,
  Typography,
} from "@mui/material";
import FolderRoundedIcon from "@mui/icons-material/FolderRounded";
import HomeRoundedIcon from "@mui/icons-material/HomeRounded";
import { useSnackbar } from "@/components/Snackbar";
import { useCopyHostsToVault, useGroups, useMoveHosts, useVaults } from "@/ipc/hooks";
import { errorMessage, type HostCard, type Uuid } from "@/ipc/types";
import { vaultHint, vaultIcon } from "@/app/vault";
import { groupPathLabel } from "./GroupPanel";

export type MoveCopyRequest =
  { kind: "group"; hosts: HostCard[] } | { kind: "vault"; hosts: HostCard[]; move: boolean };

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
  const [target, setTarget] = useState<string | null>(null);

  const ids = request.hosts.map((h) => h.id);
  const count = ids.length;
  const what = count === 1 ? `“${request.hosts[0]?.label ?? ""}”` : `${count} hosts`;
  const busy = moveHosts.isPending || copyToVault.isPending;

  const commonGroup = request.hosts.every((h) => h.groupId === request.hosts[0]?.groupId)
    ? (request.hosts[0]?.groupId ?? "")
    : null;

  const run = () => {
    if (target === null) return;
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
      copyToVault.mutate(
        { ids, vaultId: target, move: request.move },
        {
          onSuccess: () => done(`${request.move ? "Moved" : "Copied"} ${what} to ${name}`),
          onError: fail,
        },
      );
    }
  };

  const title =
    request.kind === "group"
      ? `Move ${what} to…`
      : request.move
        ? `Move ${what} to vault…`
        : `Copy ${what} to vault…`;

  const otherVaults = (vaults.data ?? []).filter((v) => v.id !== vaultId);

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
          {busy ? "Working…" : request.kind === "group" || request.move ? "Move" : "Copy"}
        </Button>
      </DialogActions>
    </>
  );
}
