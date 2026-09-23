import { Chip, Tooltip } from "@mui/material";
import VerifiedUserRoundedIcon from "@mui/icons-material/VerifiedUserRounded";
import GppMaybeRoundedIcon from "@mui/icons-material/GppMaybeRounded";
import type { TeamMember } from "@/ipc/types";
import { tr } from "@/i18n";

/** Second-factor state of a member as admins see it; hidden when the server did not tell us. */
export function MfaBadge({ m, required }: { m: TeamMember; required: boolean }) {
  if (m.mfa_enabled === true) {
    return (
      <Tooltip title={tr("Two-factor authentication enabled")}>
        <VerifiedUserRoundedIcon sx={{ fontSize: 16, color: "success.main" }} />
      </Tooltip>
    );
  }
  if (m.mfa_enabled === false) {
    return (
      <Tooltip
        title={
          required
            ? tr(
                "No two-factor authentication: this member cannot open team vaults until they enable it",
              )
            : tr("No two-factor authentication")
        }
      >
        <Chip
          size="small"
          label={tr("No 2FA")}
          color={required ? "error" : "warning"}
          variant="outlined"
          icon={<GppMaybeRoundedIcon />}
          sx={{ height: 18, fontSize: 10, fontWeight: 600, "& .MuiChip-label": { px: 0.75 } }}
        />
      </Tooltip>
    );
  }
  return null;
}
