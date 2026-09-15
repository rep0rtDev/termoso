// Active vault: the whole home tab (hosts, keychain, forwarding, snippets,
// New Tab) shows one vault at a time, picked from the Vaults dropdown in the
// top bar. Falls back to the default vault when the remembered one is gone or
// locked (sign-out, team left).

import {
  Chip,
  Divider,
  ListItemIcon,
  ListItemText,
  Menu,
  MenuItem,
  Typography,
  type PopoverOrigin,
  type SxProps,
  type Theme,
} from "@mui/material";
import AddRoundedIcon from "@mui/icons-material/AddRounded";
import CheckRoundedIcon from "@mui/icons-material/CheckRounded";
import SettingsRoundedIcon from "@mui/icons-material/SettingsRounded";
import ComputerRoundedIcon from "@mui/icons-material/ComputerRounded";
import GroupsRoundedIcon from "@mui/icons-material/GroupsRounded";
import LockRoundedIcon from "@mui/icons-material/LockRounded";
import PersonRoundedIcon from "@mui/icons-material/PersonRounded";
import type { LocalVault, Uuid } from "@/ipc/types";
import { useDefaultVault, useVaults } from "@/ipc/hooks";
import { createStore, useStore } from "@/lib/store";
import { goToSettings, goToSettingsWith } from "./navigation";

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
  /** Our role in the active vault only allows viewing; pages hide mutations. */
  readOnly: boolean;
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
    readOnly: chosen?.role === "viewer",
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

/** Small “View only” marker for pages whose active vault we can only read. */
export function ViewOnlyChip({ sx }: { sx?: SxProps<Theme> }) {
  return (
    <Chip
      size="small"
      variant="outlined"
      icon={<LockRoundedIcon />}
      label="View only"
      title="You can view this vault but not change it"
      sx={sx}
    />
  );
}

/** "Collaborate": who can open this team vault, or the Team page when there is none yet. */
export function openCollaboration(v: LocalVault | null) {
  if (v?.kind === "team") goToSettingsWith({ kind: "vault", id: v.id });
  else goToSettings("team");
}

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
      <Divider />
      <MenuItem
        onClick={() => {
          onClose();
          goToSettingsWith({ kind: "newVault" });
        }}
        sx={{ py: 0.75 }}
      >
        <ListItemIcon>
          <AddRoundedIcon fontSize="small" />
        </ListItemIcon>
        <ListItemText
          primary="New vault"
          secondary="Local, personal or shared with your team"
          slotProps={{ secondary: { sx: { fontSize: 11 } } }}
        />
      </MenuItem>
      <MenuItem
        onClick={() => {
          onClose();
          goToSettings("vaults");
        }}
        sx={{ py: 0.75 }}
      >
        <ListItemIcon>
          <SettingsRoundedIcon fontSize="small" />
        </ListItemIcon>
        <ListItemText primary="Manage vaults" />
      </MenuItem>
    </Menu>
  );
}
