import { useState } from "react";
import { Box, Button, Chip, TextField, Tooltip, Typography } from "@mui/material";
import DeleteOutlineRoundedIcon from "@mui/icons-material/DeleteOutlineRounded";
import VerifiedUserOutlinedIcon from "@mui/icons-material/VerifiedUserOutlined";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useSnackbar } from "@/components/Snackbar";
import { Field, ToolIconButton } from "@/components/ui";
import * as ipc from "@/ipc/commands";
import { useHostKeyPins, useVaults } from "@/ipc/hooks";
import { errorMessage, type HostKeyPin, type Uuid } from "@/ipc/types";
import { monoFontFamily } from "@/theme/theme";
import { tr } from "@/i18n";

/**
 * Server key pins for the host's `address:port`, grouped by the vault that
 * holds them. Pins in a team vault sync to every member, so an admin can hand
 * the team the trusted fingerprint before anyone connects; a presented key
 * that contradicts any pin is refused as changed.
 */
export function ServerKeyField({
  vaultId,
  host,
  port,
  readOnly,
}: {
  vaultId: Uuid;
  host: string;
  port: number;
  readOnly: boolean;
}) {
  const qc = useQueryClient();
  const snackbar = useSnackbar();
  const vaults = useVaults();
  const pins = useHostKeyPins(host.trim(), port);
  const [paste, setPaste] = useState(false);
  const [line, setLine] = useState("");

  const invalidate = () => qc.invalidateQueries({ queryKey: ["knownHosts"] });
  const pin = useMutation({
    mutationFn: (publicKey: string | null) =>
      ipc.hostKeyPin({ vaultId, host: host.trim(), port, publicKey }),
    onSuccess: async () => {
      await invalidate();
      setPaste(false);
      setLine("");
    },
    onError: (e) => snackbar.error(errorMessage(e)),
  });
  const unpin = useMutation({
    mutationFn: (id: Uuid) => ipc.hostKeyUnpin(id),
    onSuccess: invalidate,
    onError: (e) => snackbar.error(errorMessage(e)),
  });

  const vaultName = (id: Uuid) => {
    const v = (vaults.data ?? []).find((x) => x.id === id);
    if (!v) return "another vault";
    return v.kind === "local" ? "this device" : v.name;
  };
  const list = pins.data ?? [];
  const here = list.filter((p) => p.vaultId === vaultId);
  const elsewhere = list.filter((p) => p.vaultId !== vaultId);
  const isTeam = (vaults.data ?? []).find((v) => v.id === vaultId)?.kind === "team";
  const busy = pin.isPending || unpin.isPending;
  const blank = host.trim() === "";

  return (
    <Field
      label={tr("Server key")}
      hint={
        isTeam
          ? tr(
              "Pinned keys sync to every team member: they connect without a fingerprint prompt, and a different key is refused.",
            )
          : tr("Keys accepted on first connection are pinned here; a different key is refused.")
      }
    >
      <Box sx={{ display: "flex", flexDirection: "column", gap: 0.75 }}>
        {blank && (
          <Typography variant="body2" color="text.secondary">
            {tr("Enter the address first.")}
          </Typography>
        )}
        {!blank && list.length === 0 && !pins.isPending && (
          <Typography variant="body2" color="text.secondary">
            {tr("Not pinned yet: the fingerprint is confirmed on first connection.")}
          </Typography>
        )}
        {here.map((p) => (
          <PinRow
            key={p.id}
            pin={p}
            where={isTeam ? "team" : undefined}
            onRemove={readOnly ? undefined : () => unpin.mutate(p.id)}
            disabled={busy}
          />
        ))}
        {elsewhere.map((p) => (
          <PinRow key={p.id} pin={p} where={vaultName(p.vaultId)} disabled={busy} />
        ))}
        {!readOnly && !blank && (
          <Box sx={{ display: "flex", gap: 1, flexWrap: "wrap", mt: 0.25 }}>
            {elsewhere.length > 0 && (
              <Button
                size="small"
                variant="outlined"
                startIcon={<VerifiedUserOutlinedIcon />}
                disabled={busy}
                onClick={() => pin.mutate(null)}
              >
                {isTeam ? tr("Pin for the team") : tr("Pin in this vault")}
              </Button>
            )}
            <Button size="small" variant="text" disabled={busy} onClick={() => setPaste((v) => !v)}>
              {paste ? tr("Cancel") : tr("Paste public key")}
            </Button>
          </Box>
        )}
        {paste && (
          <Box sx={{ display: "flex", gap: 1, alignItems: "flex-start" }}>
            <TextField
              fullWidth
              multiline
              minRows={2}
              autoFocus
              placeholder={tr("ssh-ed25519 AAAA… (from `ssh-keyscan -p PORT HOST`)")}
              value={line}
              onChange={(e) => setLine(e.target.value)}
              slotProps={{ htmlInput: { style: { fontFamily: monoFontFamily, fontSize: 12 } } }}
            />
            <Button
              size="small"
              variant="contained"
              disabled={busy || line.trim() === ""}
              onClick={() => pin.mutate(line)}
            >
              {tr("Pin")}
            </Button>
          </Box>
        )}
      </Box>
    </Field>
  );
}

function PinRow({
  pin,
  where,
  onRemove,
  disabled,
}: {
  pin: HostKeyPin;
  where?: string;
  onRemove?: () => void;
  disabled: boolean;
}) {
  return (
    <Box sx={{ display: "flex", alignItems: "center", gap: 1, minWidth: 0 }}>
      <Chip size="small" label={pin.keyType} sx={{ flexShrink: 0 }} />
      <Tooltip title={pin.publicKey} placement="top-start">
        <Typography
          variant="body2"
          sx={{
            fontFamily: monoFontFamily,
            fontSize: 12,
            overflow: "hidden",
            textOverflow: "ellipsis",
            whiteSpace: "nowrap",
            flex: 1,
            minWidth: 0,
          }}
        >
          {pin.fingerprint}
        </Typography>
      </Tooltip>
      {where && (
        <Typography variant="caption" color="text.secondary" sx={{ flexShrink: 0 }}>
          {where}
        </Typography>
      )}
      {onRemove && (
        <ToolIconButton title={tr("Unpin")} onClick={onRemove} disabled={disabled}>
          <DeleteOutlineRoundedIcon fontSize="small" />
        </ToolIconButton>
      )}
    </Box>
  );
}
