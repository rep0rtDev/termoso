import { useState } from "react";
import { Box, Button, Typography } from "@mui/material";
import CheckRoundedIcon from "@mui/icons-material/CheckRounded";
import ChevronRightRoundedIcon from "@mui/icons-material/ChevronRightRounded";
import { useAccount, useHosts, useTeamInvites, useTeams } from "@/ipc/hooks";
import type { LocalVault, Team } from "@/ipc/types";
import { goToSettingsWith } from "@/app/navigation";
import { ShareDataDialog } from "./ShareDataDialog";
import { tr } from "@/i18n";

const HIDE_KEY = "termoso.teamSteps.hidden";

/** “Team steps — 1 of 3 done” onboarding cards above the hosts, like Termius. */
export function TeamSteps() {
  const account = useAccount();
  const signedIn = !!account.data?.account;
  const teams = useTeams(signedIn);
  const team = teams.data?.[0] ?? null;
  const vaults = account.data?.vaults ?? [];
  const [hidden, setHidden] = useState(() => localStorage.getItem(HIDE_KEY) === "1");
  if (!signedIn || !team || hidden) return null;
  return (
    <Steps
      team={team}
      vaults={vaults}
      onHide={() => {
        localStorage.setItem(HIDE_KEY, "1");
        setHidden(true);
      }}
    />
  );
}

function Steps({ team, vaults, onHide }: { team: Team; vaults: LocalVault[]; onHide: () => void }) {
  const personal = vaults.find((v) => v.kind === "personal" && v.unlocked) ?? null;
  const teamVault = vaults.find((v) => v.kind === "team" && v.team_id === team.id) ?? null;
  const teamHosts = useHosts(teamVault?.unlocked ? teamVault.id : null);
  const invites = useTeamInvites(team.id);
  const [share, setShare] = useState(false);

  const vaultDone = teamVault !== null;
  const shareDone = vaultDone && (teamHosts.data?.length ?? 0) > 0;
  const inviteDone = team.member_count > 1 || (invites.data?.length ?? 0) > 0;
  const done = [vaultDone, shareDone, inviteDone].filter(Boolean).length;
  if (done === 3) return null;
  const current = !vaultDone ? 0 : !shareDone ? 1 : 2;

  return (
    <Box sx={{ mb: 2.5 }}>
      <Box sx={{ display: "flex", alignItems: "center", mb: 1.25 }}>
        <Typography variant="body2" sx={{ fontWeight: 600, flex: 1 }}>
          {tr("Team steps")}{" "}
          <Box component="span" sx={{ color: "text.secondary", fontWeight: 400 }}>
            - {tr("{done} of 3 done", { done })}
          </Box>
        </Typography>
        <Button size="small" color="inherit" onClick={onHide} sx={{ opacity: 0.7 }}>
          {tr("Hide")}
        </Button>
      </Box>
      <Box sx={{ display: "grid", gridTemplateColumns: "repeat(3, minmax(0, 1fr))", gap: 2.5 }}>
        <StepCard
          title={tr("Enable team vault")}
          text={tr("Set up a team vault to share data easily and securely.")}
          done={vaultDone}
          active={current === 0}
          onClick={() => goToSettingsWith({ kind: "newVault", teamId: team.id })}
        />
        <StepCard
          title={tr("Share data")}
          text={tr("Select the information you want your team to access.")}
          done={shareDone}
          active={current === 1}
          disabled={!teamVault?.unlocked || !personal}
          onClick={() => setShare(true)}
        />
        <StepCard
          title={tr("Invite team members")}
          text={tr("Grant access to the shared vault to your teammates.")}
          done={inviteDone}
          active={current === 2}
          onClick={() => goToSettingsWith({ kind: "invite" })}
        />
      </Box>
      <ShareDataDialog
        open={share}
        source={personal}
        target={teamVault}
        onClose={() => setShare(false)}
      />
    </Box>
  );
}

function StepCard({
  title,
  text,
  done,
  active,
  disabled,
  onClick,
}: {
  title: string;
  text: string;
  done: boolean;
  active: boolean;
  disabled?: boolean;
  onClick: () => void;
}) {
  const inert = done || disabled;
  return (
    <Box
      role={inert ? undefined : "button"}
      tabIndex={inert ? -1 : 0}
      onClick={inert ? undefined : onClick}
      onKeyDown={(e) => {
        if (!inert && (e.key === "Enter" || e.key === " ")) onClick();
      }}
      sx={{
        borderRadius: 2,
        border: "1px solid",
        borderColor: active ? "primary.main" : "divider",
        bgcolor: done ? "surface.base" : "surface.high",
        opacity: done ? 0.7 : disabled ? 0.55 : 1,
        cursor: inert ? "default" : "pointer",
        overflow: "hidden",
        "&:hover": inert ? undefined : { bgcolor: "surface.highest" },
      }}
    >
      <Box
        sx={{
          display: "flex",
          alignItems: "center",
          px: 1.5,
          height: 38,
          borderBottom: "1px solid",
          borderColor: "divider",
        }}
      >
        <Typography
          variant="body2"
          sx={{ fontWeight: 600, flex: 1, color: done ? "text.secondary" : "text.primary" }}
        >
          {title}
        </Typography>
        {done ? (
          <CheckRoundedIcon sx={{ fontSize: 16, color: "text.secondary" }} />
        ) : (
          <ChevronRightRoundedIcon sx={{ fontSize: 18, color: "text.secondary" }} />
        )}
      </Box>
      <Typography
        variant="body2"
        color="text.secondary"
        sx={{ px: 1.5, py: 1.25, lineHeight: 1.4 }}
      >
        {text}
      </Typography>
    </Box>
  );
}
