import { Chip } from "@mui/material";
import type { TeamRole, VaultRole } from "@/api/types";

const colors: Record<TeamRole | VaultRole, "default" | "primary" | "secondary" | "warning"> = {
  owner: "warning",
  admin: "primary",
  member: "default",
  manager: "primary",
  editor: "secondary",
  viewer: "default",
};

export function RoleChip({ role }: { role: TeamRole | VaultRole }) {
  return (
    <Chip
      size="small"
      variant={colors[role] === "default" ? "outlined" : "filled"}
      color={colors[role]}
      label={role.charAt(0).toUpperCase() + role.slice(1)}
      sx={{ textTransform: "none" }}
    />
  );
}
