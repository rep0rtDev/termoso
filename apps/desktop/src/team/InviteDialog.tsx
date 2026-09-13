import { useState } from "react";
import { copyToClipboard } from "@/lib/clipboard";
import {
  Alert,
  Box,
  Button,
  Chip,
  Dialog,
  IconButton,
  MenuItem,
  Stack,
  TextField,
  Tooltip,
  Typography,
} from "@mui/material";
import ContentCopyRoundedIcon from "@mui/icons-material/ContentCopyRounded";
import CheckCircleRoundedIcon from "@mui/icons-material/CheckCircleRounded";
import ErrorOutlineRoundedIcon from "@mui/icons-material/ErrorOutlineRounded";
import GroupsRoundedIcon from "@mui/icons-material/GroupsRounded";
import LinkRoundedIcon from "@mui/icons-material/LinkRounded";
import CloseRoundedIcon from "@mui/icons-material/CloseRounded";
import { useMutation } from "@tanstack/react-query";
import { useSnackbar } from "@/components/Snackbar";
import { Mono } from "@/components/ui";
import * as ipc from "@/ipc/commands";
import { useAccount, useInvalidateTeam } from "@/ipc/hooks";
import {
  errorMessage,
  type InviteResult,
  type LocalVault,
  type Team,
  type TeamRole,
  type Uuid,
} from "@/ipc/types";
import { looksLikeEmail, teamRoleLabel } from "./roles";
import { ShareDataDialog } from "./ShareDataDialog";

/**
 * “Invite your teammates” (Termius flow): one e-mail per row, `+ Add another`, a compact
 * role / vault-access line, then the per-address links and an optional “Share data” step.
 * Invitations are valid for 14 days and only work for the address they were issued to.
 */
export function InviteDialog({
  team,
  vaults,
  open,
  onClose,
}: {
  team: Team;
  /** Team vaults the current user manages (candidates for access on join). */
  vaults: LocalVault[];
  open: boolean;
  onClose: () => void;
}) {
  return (
    <Dialog open={open} onClose={onClose} maxWidth="sm" fullWidth>
      {open && <Body team={team} vaults={vaults} onClose={onClose} />}
    </Dialog>
  );
}

function Body({
  team,
  vaults,
  onClose,
}: {
  team: Team;
  vaults: LocalVault[];
  onClose: () => void;
}) {
  const snackbar = useSnackbar();
  const invalidate = useInvalidateTeam();
  const account = useAccount();
  const [rows, setRows] = useState<string[]>([""]);
  const [role, setRole] = useState<TeamRole>("member");
  const [vaultIds, setVaultIds] = useState<Set<Uuid>>(() => new Set(vaults.map((v) => v.id)));
  const [results, setResults] = useState<InviteResult[] | null>(null);
  const [share, setShare] = useState(false);

  const emails = [...new Set(rows.map((r) => r.trim()).filter(Boolean))];
  const bad = emails.filter((e) => !looksLikeEmail(e));
  const ready = emails.length > 0 && bad.length === 0;

  const send = useMutation({
    mutationFn: ({ copy }: { copy: boolean }) =>
      ipc.teamInvite(team.id, emails, role, [...vaultIds]).then((r) => ({ r, copy })),
    onSuccess: ({ r, copy }) => {
      invalidate();
      setResults(r);
      const url = r.find((x) => x.url)?.url;
      if (copy && url) {
        void copyToClipboard(url)
          .then(() => snackbar.notify("Invitation link copied"))
          .catch(() => snackbar.error("Clipboard is not available"));
      }
    },
    onError: (e) => snackbar.error(errorMessage(e)),
  });

  const personal = (account.data?.vaults ?? []).find((v) => v.kind === "personal" && v.unlocked);
  const shareTarget = vaults.find((v) => v.unlocked) ?? null;

  if (results) {
    return (
      <>
        <Results
          team={team}
          results={results}
          onClose={onClose}
          onShare={personal && shareTarget ? () => setShare(true) : undefined}
        />
        <ShareDataDialog
          open={share}
          source={personal ?? null}
          target={shareTarget}
          onClose={() => setShare(false)}
          onDone={onClose}
        />
      </>
    );
  }

  return (
    <>
      <Box sx={{ px: 4, pt: 4, pb: 3, display: "flex", flexDirection: "column", gap: 1 }}>
        <Box sx={{ display: "flex", justifyContent: "center", mb: 1 }}>
          <Box
            sx={{
              width: 56,
              height: 56,
              borderRadius: "50%",
              display: "grid",
              placeItems: "center",
              bgcolor: "surface.high",
              color: "primary.main",
            }}
          >
            <GroupsRoundedIcon sx={{ fontSize: 34 }} />
          </Box>
        </Box>
        <Typography variant="h5" sx={{ fontWeight: 700, textAlign: "center" }}>
          Invite your teammates
        </Typography>
        <Typography
          variant="body2"
          color="text.secondary"
          sx={{ textAlign: "center", maxWidth: 440, mx: "auto" }}
        >
          Manage infrastructure together in shared team vaults. Keep your teammates on the same page
          and boost their productivity.
        </Typography>
        <Box sx={{ borderTop: "1px solid", borderColor: "divider", my: 1.5 }} />
        <Stack spacing={1.25}>
          {rows.map((value, i) => {
            const v = value.trim();
            const invalid = v.length > 0 && !looksLikeEmail(v);
            return (
              <Box key={i} sx={{ display: "flex", alignItems: "center", gap: 0.5 }}>
                <TextField
                  autoFocus={i === rows.length - 1}
                  fullWidth
                  size="small"
                  type="email"
                  label={v ? "Email" : undefined}
                  placeholder="Email"
                  value={value}
                  error={invalid}
                  helperText={invalid ? "Not an e-mail address" : undefined}
                  onChange={(e) => setRows((r) => r.map((x, j) => (j === i ? e.target.value : x)))}
                  onKeyDown={(e) => {
                    if (e.key === "Enter" && v && !invalid && i === rows.length - 1) {
                      setRows((r) => [...r, ""]);
                    }
                  }}
                />
                {rows.length > 1 && (
                  <IconButton
                    size="small"
                    aria-label="Remove"
                    onClick={() => setRows((r) => r.filter((_, j) => j !== i))}
                  >
                    <CloseRoundedIcon fontSize="small" />
                  </IconButton>
                )}
              </Box>
            );
          })}
          <Button
            size="small"
            variant="text"
            sx={{ alignSelf: "flex-start", px: 0.5 }}
            disabled={rows[rows.length - 1]?.trim() === ""}
            onClick={() => setRows((r) => [...r, ""])}
          >
            + Add another
          </Button>
        </Stack>
        <Box sx={{ display: "flex", alignItems: "center", gap: 1, flexWrap: "wrap", mt: 1.5 }}>
          <Typography variant="caption" color="text.secondary">
            Invite as
          </Typography>
          <TextField
            select
            size="small"
            value={role}
            onChange={(e) => setRole(e.target.value as TeamRole)}
            sx={{ minWidth: 120, "& .MuiSelect-select": { py: 0.5, fontSize: 13 } }}
          >
            {(["member", "admin"] as TeamRole[]).map((r) => (
              <MenuItem key={r} value={r}>
                {teamRoleLabel[r]}
              </MenuItem>
            ))}
          </TextField>
          {vaults.length > 0 && (
            <>
              <Typography variant="caption" color="text.secondary" sx={{ ml: 1 }}>
                with access to
              </Typography>
              {vaults.map((v) => {
                const on = vaultIds.has(v.id);
                return (
                  <Chip
                    key={v.id}
                    size="small"
                    icon={<GroupsRoundedIcon />}
                    label={v.name}
                    color={on ? "primary" : "default"}
                    variant={on ? "filled" : "outlined"}
                    onClick={() => {
                      const next = new Set(vaultIds);
                      if (on) next.delete(v.id);
                      else next.add(v.id);
                      setVaultIds(next);
                    }}
                  />
                );
              })}
            </>
          )}
        </Box>
      </Box>
      <Footer>
        <Button color="inherit" onClick={onClose} disabled={send.isPending}>
          Later
        </Button>
        <Box sx={{ flex: 1 }} />
        <Tooltip title="Send the invitation and copy the first link to the clipboard">
          <span>
            <Button
              variant="tonal"
              endIcon={<LinkRoundedIcon />}
              disabled={!ready || send.isPending}
              onClick={() => send.mutate({ copy: true })}
            >
              Copy invitation link
            </Button>
          </span>
        </Tooltip>
        <Button
          variant="contained"
          disabled={!ready || send.isPending}
          onClick={() => send.mutate({ copy: false })}
        >
          {send.isPending ? "Inviting…" : "Continue"}
        </Button>
      </Footer>
    </>
  );
}

function Footer({ children }: { children: React.ReactNode }) {
  return (
    <Box
      sx={{
        display: "flex",
        alignItems: "center",
        gap: 1,
        px: 3,
        py: 1.5,
        borderTop: "1px solid",
        borderColor: "divider",
      }}
    >
      {children}
    </Box>
  );
}

function Results({
  team,
  results,
  onClose,
  onShare,
}: {
  team: Team;
  results: InviteResult[];
  onClose: () => void;
  onShare?: () => void;
}) {
  const ok = results.filter((r) => r.url);
  return (
    <>
      <Box sx={{ px: 4, pt: 3, pb: 2 }}>
        <Typography variant="h6" sx={{ fontWeight: 700, mb: 1 }}>
          {ok.length === results.length
            ? `Invited to ${team.name}`
            : `${ok.length} of ${results.length} invited`}
        </Typography>
        <Stack spacing={1.5}>
          {ok.length > 0 && (
            <Alert severity="info">
              An e-mail goes out when the server has mail set up. Copy a link to send it yourself —
              it works only for the address it was issued to and expires in 14 days.
            </Alert>
          )}
          <Stack spacing={1}>
            {results.map((r) => (
              <InviteResultRow key={r.email} result={r} />
            ))}
          </Stack>
        </Stack>
      </Box>
      <Footer>
        <Button color="inherit" onClick={onClose}>
          {onShare ? "Later" : "Done"}
        </Button>
        <Box sx={{ flex: 1 }} />
        {onShare && (
          <Button variant="contained" onClick={onShare}>
            Share data
          </Button>
        )}
      </Footer>
    </>
  );
}

export function InviteResultRow({ result: r }: { result: InviteResult }) {
  const snackbar = useSnackbar();
  return (
    <Box
      sx={{
        display: "flex",
        alignItems: "center",
        gap: 1.25,
        px: 1.5,
        py: 1,
        borderRadius: 2,
        bgcolor: "surface.high",
      }}
    >
      {r.url ? (
        <CheckCircleRoundedIcon fontSize="small" color="success" />
      ) : (
        <ErrorOutlineRoundedIcon fontSize="small" color="error" />
      )}
      <Box sx={{ flex: 1, minWidth: 0 }}>
        <Typography variant="body2" noWrap sx={{ fontWeight: 500 }}>
          {r.email}
        </Typography>
        {r.url ? (
          <Mono
            secondary
            sx={{ display: "block", overflow: "hidden", textOverflow: "ellipsis", fontSize: 11 }}
          >
            {r.url}
          </Mono>
        ) : (
          <Typography variant="caption" color="error" sx={{ display: "block" }}>
            {r.error ?? "Failed"}
          </Typography>
        )}
      </Box>
      {r.url && (
        <Tooltip title="Copy invitation link">
          <IconButton
            size="small"
            onClick={() => {
              const url = r.url;
              if (!url) return;
              void copyToClipboard(url)
                .then(() => snackbar.notify("Invitation link copied"))
                .catch(() => snackbar.error("Clipboard is not available"));
            }}
          >
            <ContentCopyRoundedIcon fontSize="small" />
          </IconButton>
        </Tooltip>
      )}
    </Box>
  );
}
