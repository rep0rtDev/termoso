import { Chip, Tooltip } from "@mui/material";
import GppMaybeRoundedIcon from "@mui/icons-material/GppMaybeRounded";
import VerifiedUserRoundedIcon from "@mui/icons-material/VerifiedUserRounded";
import type { TeamMember } from "@/api/types";

/**
 * Second-factor state of a team member as the server discloses it: admins
 * see everyone, members only themselves (nothing is drawn otherwise).
 */
export function MfaBadge({ m, required }: { m: TeamMember; required: boolean }) {
  if (m.mfa_enabled === true) {
    return (
      <Tooltip title="Two-factor authentication enabled">
        <VerifiedUserRoundedIcon
          sx={{ fontSize: 16, color: "success.main", verticalAlign: "middle" }}
        />
      </Tooltip>
    );
  }
  if (m.mfa_enabled === false) {
    return (
      <Tooltip
        title={
          required
            ? "No two-factor authentication: this member cannot open team vaults until they enable it"
            : "No two-factor authentication"
        }
      >
        <Chip
          size="small"
          label="No 2FA"
          color={required ? "error" : "warning"}
          variant="outlined"
          icon={<GppMaybeRoundedIcon />}
          sx={{ height: 20, fontSize: 11, fontWeight: 600, "& .MuiChip-label": { px: 0.75 } }}
        />
      </Tooltip>
    );
  }
  return null;
}
