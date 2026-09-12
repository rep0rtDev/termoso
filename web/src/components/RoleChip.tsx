import { Chip } from "@mui/material";
import type { TeamRole, VaultRole } from "@/api/types";

/** Owners/managers get the accent; everyone else stays neutral. */
const accent: Partial<Record<TeamRole | VaultRole, "primary" | "secondary">> = {
  owner: "primary",
  manager: "primary",
  admin: "secondary",
};

export function RoleChip({ role }: { role: TeamRole | VaultRole }) {
  const color = accent[role];
  return (
    <Chip
      size="small"
      color={color ?? "default"}
      label={role.charAt(0).toUpperCase() + role.slice(1)}
      sx={color ? undefined : { color: "text.secondary" }}
    />
  );
}
