import { useState } from "react";
import {
  Alert,
  Box,
  Button,
  Checkbox,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  FormControlLabel,
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
import { useMutation } from "@tanstack/react-query";
import { useSnackbar } from "@/components/Snackbar";
import { Field, Mono } from "@/components/ui";
import * as ipc from "@/ipc/commands";
import { useInvalidateTeam } from "@/ipc/hooks";
import {
  errorMessage,
  type InviteResult,
  type LocalVault,
  type Team,
  type TeamRole,
  type Uuid,
} from "@/ipc/types";
import { looksLikeEmail, splitEmails, teamRoleHint, teamRoleLabel } from "./roles";

/**
 * "Invite members": several addresses at once, one team role for all of them
 * and the team vaults they should be let into once they accept. Ends with the
 * per-address outcome — a link to copy, or why it failed.
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
  const [text, setText] = useState("");
  const [role, setRole] = useState<TeamRole>("member");
  const [vaultIds, setVaultIds] = useState<Set<Uuid>>(() => new Set(vaults.map((v) => v.id)));
  const [results, setResults] = useState<InviteResult[] | null>(null);

  const emails = splitEmails(text);
  const bad = emails.filter((e) => !looksLikeEmail(e));

  const send = useMutation({
    mutationFn: () => ipc.teamInvite(team.id, emails, role, [...vaultIds]),
    onSuccess: (r) => {
      invalidate();
      setResults(r);
    },
    onError: (e) => snackbar.error(errorMessage(e)),
  });

  if (results) return <Results team={team} results={results} onClose={onClose} />;

  return (
    <>
      <DialogTitle>Invite members to {team.name}</DialogTitle>
      <DialogContent>
        <Stack spacing={2}>
          <Field
            label="E-mail addresses"
            hint="One or many — separate with commas, spaces or new lines. Each person gets their own link; the invitation is valid for 14 days."
          >
            <TextField
              autoFocus
              multiline
              minRows={2}
              maxRows={6}
              fullWidth
              value={text}
              onChange={(e) => setText(e.target.value)}
              placeholder="alice@example.com, bob@example.com"
              error={bad.length > 0}
              helperText={bad.length ? `Not an address: ${bad.join(", ")}` : undefined}
            />
          </Field>
          <Field label="Team role">
            <TextField
              select
              fullWidth
              value={role}
              onChange={(e) => setRole(e.target.value as TeamRole)}
            >
              {(["member", "admin"] as TeamRole[]).map((r) => (
                <MenuItem key={r} value={r}>
                  <Box>
                    <Typography variant="body2">{teamRoleLabel[r]}</Typography>
                    <Typography variant="caption" color="text.secondary">
                      {teamRoleHint[r]}
                    </Typography>
                  </Box>
                </MenuItem>
              ))}
            </TextField>
          </Field>
          {vaults.length > 0 && (
            <Field
              label="Vault access on joining"
              hint="They are listed as pending until a vault manager hands them the key — you will see a prompt on the Team page."
            >
              <Stack>
                {vaults.map((v) => (
                  <FormControlLabel
                    key={v.id}
                    control={
                      <Checkbox
                        checked={vaultIds.has(v.id)}
                        onChange={(e) => {
                          const next = new Set(vaultIds);
                          if (e.target.checked) next.add(v.id);
                          else next.delete(v.id);
                          setVaultIds(next);
                        }}
                      />
                    }
                    label={v.name}
                  />
                ))}
              </Stack>
            </Field>
          )}
        </Stack>
      </DialogContent>
      <DialogActions sx={{ px: 3, pb: 2 }}>
        <Button color="inherit" onClick={onClose} disabled={send.isPending}>
          Cancel
        </Button>
        <Button
          variant="contained"
          disabled={send.isPending || emails.length === 0 || bad.length > 0}
          onClick={() => send.mutate()}
        >
          {send.isPending
            ? "Inviting…"
            : emails.length > 1
              ? `Invite ${emails.length} people`
              : "Invite"}
        </Button>
      </DialogActions>
    </>
  );
}

function Results({
  team,
  results,
  onClose,
}: {
  team: Team;
  results: InviteResult[];
  onClose: () => void;
}) {
  const ok = results.filter((r) => r.url);
  return (
    <>
      <DialogTitle>
        {ok.length === results.length
          ? `Invited to ${team.name}`
          : `${ok.length} of ${results.length} invited`}
      </DialogTitle>
      <DialogContent>
        <Stack spacing={1.5}>
          {ok.length > 0 && (
            <Alert severity="info">
              An e-mail goes out when the server has mail set up. Copy a link to send it yourself —
              it works only for the address it was issued to.
            </Alert>
          )}
          <Stack spacing={1}>
            {results.map((r) => (
              <InviteResultRow key={r.email} result={r} />
            ))}
          </Stack>
        </Stack>
      </DialogContent>
      <DialogActions sx={{ px: 3, pb: 2 }}>
        <Button variant="contained" onClick={onClose}>
          Done
        </Button>
      </DialogActions>
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
              void navigator.clipboard
                .writeText(url)
                .then(() => snackbar.notify("Invitation link copied"));
            }}
          >
            <ContentCopyRoundedIcon fontSize="small" />
          </IconButton>
        </Tooltip>
      )}
    </Box>
  );
}
