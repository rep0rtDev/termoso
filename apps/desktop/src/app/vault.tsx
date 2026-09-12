// Active vault: the whole home tab (hosts, keychain, forwarding, snippets,
// New Tab) shows one vault at a time, picked from the Vaults dropdown in the
// top bar. Falls back to the default vault when the remembered one is gone or
// locked (sign-out, team left).

import {
  ListItemIcon,
  ListItemText,
  Menu,
  MenuItem,
  Typography,
  type PopoverOrigin,
} from "@mui/material";
import CheckRoundedIcon from "@mui/icons-material/CheckRounded";
import ComputerRoundedIcon from "@mui/icons-material/ComputerRounded";
import GroupsRoundedIcon from "@mui/icons-material/GroupsRounded";
import LockRoundedIcon from "@mui/icons-material/LockRounded";
import PersonRoundedIcon from "@mui/icons-material/PersonRounded";
import type { LocalVault, Uuid } from "@/ipc/types";
import { useDefaultVault, useVaults } from "@/ipc/hooks";
import { createStore, useStore } from "@/lib/store";

const VAULT_KEY = "termoso.activeVault";

interface VaultState {
  selectedId: Uuid | null;
}

export const vaultStore = createStore<VaultState>({
  selectedId: localStorage.getItem(VAULT_KEY),
});

export function selectVault(id: Uuid) {
  localStorage.setItem(VAULT_KEY, id);
  vaultStore.set((s) => (s.selectedId === id ? s : { ...s, selectedId: id }));
}

export interface ActiveVault {
  /** The vault every home section reads from and writes to. */
  data: LocalVault | null;
  /** All vaults on this device, for the dropdown. */
  vaults: LocalVault[];
  isPending: boolean;
  error: Error | null;
}

/** Same shape as `useDefaultVault()` so pages swap hooks without churn. */
export function useActiveVault(): ActiveVault {
  const vaults = useVaults();
  const fallback = useDefaultVault();
  const selectedId = useStore(vaultStore, (s) => s.selectedId);
  const list = vaults.data ?? [];
  const chosen = list.find((v) => v.id === selectedId && v.unlocked) ?? fallback.data ?? null;
  return {
    data: chosen,
    vaults: list,
    isPending: vaults.isPending || fallback.isPending,
    error: vaults.error ?? fallback.error,
  };
}

export const vaultIcon = (v: LocalVault) =>
  !v.unlocked ? (
    <LockRoundedIcon fontSize="small" />
  ) : v.kind === "team" ? (
    <GroupsRoundedIcon fontSize="small" />
  ) : v.kind === "personal" ? (
    <PersonRoundedIcon fontSize="small" />
  ) : (
    <ComputerRoundedIcon fontSize="small" />
  );

export const vaultHint = (v: LocalVault) =>
  !v.unlocked
    ? "Locked"
    : v.role === "viewer"
      ? "Read-only"
      : v.kind === "team"
        ? "Team vault"
        : v.kind === "personal"
          ? "Synced personal vault"
          : "This device only";

const ORIGIN_TOP: PopoverOrigin = { vertical: "top", horizontal: "left" };
const ORIGIN_BOTTOM: PopoverOrigin = { vertical: "bottom", horizontal: "left" };

/** Vault picker under the Vaults tab; one vault is active at a time, like Termius. */
export function VaultMenu({
  anchor,
  onClose,
}: {
  anchor: HTMLElement | null;
  onClose: () => void;
}) {
  const active = useActiveVault();
  return (
    <Menu
      open={anchor !== null}
      anchorEl={anchor}
      onClose={onClose}
      anchorOrigin={ORIGIN_BOTTOM}
      transformOrigin={ORIGIN_TOP}
      slotProps={{ paper: { sx: { minWidth: 240 } } }}
    >
      {active.vaults.map((v) => {
        const selected = v.id === active.data?.id;
        return (
          <MenuItem
            key={v.id}
            selected={selected}
            disabled={!v.unlocked}
            onClick={() => {
              selectVault(v.id);
              onClose();
            }}
            sx={{ py: 0.75 }}
          >
            <ListItemIcon>{vaultIcon(v)}</ListItemIcon>
            <ListItemText
              primary={v.name}
              secondary={vaultHint(v)}
              slotProps={{ secondary: { sx: { fontSize: 11 } } }}
            />
            <CheckRoundedIcon
              fontSize="small"
              sx={{ color: "primary.main", ml: 1.5, visibility: selected ? "visible" : "hidden" }}
            />
          </MenuItem>
        );
      })}
      {active.vaults.length === 0 && (
        <Typography variant="body2" color="text.secondary" sx={{ px: 2, py: 1 }}>
          No vaults yet
        </Typography>
      )}
    </Menu>
  );
}
