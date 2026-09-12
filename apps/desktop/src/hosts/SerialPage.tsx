import { useState } from "react";
import {
  Box,
  Button,
  Collapse,
  IconButton,
  InputAdornment,
  Menu,
  MenuItem,
  TextField,
  ToggleButton,
  ToggleButtonGroup,
  Typography,
} from "@mui/material";
import UsbRoundedIcon from "@mui/icons-material/UsbRounded";
import SettingsRoundedIcon from "@mui/icons-material/SettingsRounded";
import TerminalRoundedIcon from "@mui/icons-material/TerminalRounded";
import ExpandMoreRoundedIcon from "@mui/icons-material/ExpandMoreRounded";
import RefreshRoundedIcon from "@mui/icons-material/RefreshRounded";
import { Field, IconTile, ToolIconButton } from "@/components/ui";
import { useSerialPorts } from "@/ipc/hooks";
import {
  SERIAL_BAUD_RATES,
  SERIAL_CHARSETS,
  defaultSerialLine,
  errorMessage,
  type SerialLine,
  type SerialPortInfo,
} from "@/ipc/types";
import { openTerminal } from "@/terminal/store";
import { goHome } from "@/app/navigation";
import { monoFontFamily } from "@/theme/theme";

const WIDTH = 520;

function describePort(p: SerialPortInfo) {
  const parts = [p.manufacturer, p.product].filter((s): s is string => !!s);
  return parts.length > 0 ? parts.join(" ") : p.kind === "unknown" ? null : p.kind.toUpperCase();
}

/**
 * Termius' Serial tab: a centred card with the device, baud rate and an
 * Advanced fold-out, then Close / Connect. Nothing here is saved — a serial
 * console is a local, one-off connection, not a host in the vault.
 */
export function SerialPage() {
  const ports = useSerialPorts(true);
  const list = ports.data ?? [];
  const [typed, setTyped] = useState<string | null>(null);
  const [line, setLine] = useState<SerialLine>(defaultSerialLine);
  const [advanced, setAdvanced] = useState(false);
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);

  // A single discovered device is preselected until the user types or picks another.
  const path = typed ?? (list.length === 1 ? (list[0]?.path ?? "") : "");
  const setPath = (p: string) => setTyped(p);

  const set = <K extends keyof SerialLine>(k: K, v: SerialLine[K]) =>
    setLine((l) => ({ ...l, [k]: v }));
  const bauds = SERIAL_BAUD_RATES.includes(line.baudRate as (typeof SERIAL_BAUD_RATES)[number])
    ? [...SERIAL_BAUD_RATES]
    : [line.baudRate, ...SERIAL_BAUD_RATES];
  const charsets = SERIAL_CHARSETS.some((c) => c.value === line.charset)
    ? SERIAL_CHARSETS
    : [{ value: line.charset, label: line.charset || "UTF-8" }, ...SERIAL_CHARSETS];
  const canConnect = path.trim().length > 0;

  const connect = () => {
    if (!canConnect) return;
    openTerminal({ kind: "serial", path: path.trim(), line });
  };

  return (
    <Box
      sx={{
        flex: 1,
        minHeight: 0,
        overflowY: "auto",
        display: "flex",
        justifyContent: "center",
        px: 3,
        py: 6,
      }}
    >
      <Box
        component="form"
        onSubmit={(e) => {
          e.preventDefault();
          connect();
        }}
        sx={{ width: WIDTH, maxWidth: "100%", display: "flex", flexDirection: "column", gap: 2 }}
      >
        <Box sx={{ display: "flex", alignItems: "center", gap: 2 }}>
          <IconTile size={52}>
            <UsbRoundedIcon />
          </IconTile>
          <Typography variant="h6">Serial</Typography>
        </Box>

        <Box sx={{ display: "flex", alignItems: "center", my: 1 }}>
          <Box
            sx={{
              width: 32,
              height: 32,
              borderRadius: "50%",
              display: "grid",
              placeItems: "center",
              bgcolor: "secondary.main",
              color: "#fff",
            }}
          >
            <SettingsRoundedIcon sx={{ fontSize: 18 }} />
          </Box>
          <Box sx={{ flex: 1, height: 4, bgcolor: "surface.highest" }} />
          <Box
            sx={{
              width: 32,
              height: 32,
              borderRadius: "50%",
              display: "grid",
              placeItems: "center",
              bgcolor: "surface.highest",
              color: "text.secondary",
            }}
          >
            <TerminalRoundedIcon sx={{ fontSize: 18 }} />
          </Box>
        </Box>

        <Field
          label="Serial Port"
          hint={
            ports.isError
              ? errorMessage(ports.error)
              : list.length === 0 && !ports.isPending
                ? "No serial devices found — plug one in and rescan, or type the path."
                : undefined
          }
        >
          <Box sx={{ display: "flex", gap: 1, alignItems: "center" }}>
            <TextField
              autoFocus
              value={path}
              onChange={(e) => setPath(e.target.value)}
              placeholder={
                list.length > 0 ? "Pick a device or type a path" : "/dev/ttyUSB0 or COM3"
              }
              sx={{ flex: 1 }}
              slotProps={{
                input: {
                  sx: { fontFamily: monoFontFamily },
                  endAdornment:
                    list.length > 0 ? (
                      <InputAdornment position="end">
                        <IconButton
                          size="small"
                          edge="end"
                          aria-label="Pick a device"
                          onClick={(e) => setAnchor(e.currentTarget)}
                        >
                          <ExpandMoreRoundedIcon fontSize="small" />
                        </IconButton>
                      </InputAdornment>
                    ) : undefined,
                },
              }}
            />
            <ToolIconButton
              title="Rescan devices"
              disabled={ports.isFetching}
              onClick={() => void ports.refetch()}
            >
              <RefreshRoundedIcon fontSize="small" />
            </ToolIconButton>
          </Box>
          <Menu
            open={anchor !== null}
            anchorEl={anchor}
            onClose={() => setAnchor(null)}
            slotProps={{ paper: { sx: { minWidth: 320 } } }}
          >
            {list.map((p) => (
              <MenuItem
                key={p.path}
                selected={p.path === path}
                onClick={() => {
                  setPath(p.path);
                  setAnchor(null);
                }}
              >
                <Box component="span" sx={{ fontFamily: monoFontFamily }}>
                  {p.path}
                </Box>
                {describePort(p) && (
                  <Typography
                    component="span"
                    variant="caption"
                    color="text.secondary"
                    sx={{ ml: 1 }}
                  >
                    {describePort(p)}
                  </Typography>
                )}
              </MenuItem>
            ))}
          </Menu>
        </Field>

        <Field label="Baud rate">
          <TextField
            select
            value={line.baudRate}
            onChange={(e) => set("baudRate", Number(e.target.value))}
            slotProps={{ input: { sx: { fontFamily: monoFontFamily } } }}
          >
            {bauds.map((b) => (
              <MenuItem key={b} value={b} sx={{ fontFamily: monoFontFamily }}>
                {b}
              </MenuItem>
            ))}
          </TextField>
        </Field>

        <Box sx={{ bgcolor: "surface.high", borderRadius: 2 }}>
          <Box
            component="button"
            type="button"
            onClick={() => setAdvanced((v) => !v)}
            aria-expanded={advanced}
            sx={{
              all: "unset",
              boxSizing: "border-box",
              width: "100%",
              display: "flex",
              alignItems: "center",
              px: 2,
              height: 48,
              cursor: "pointer",
              borderRadius: 2,
              "&:hover": { bgcolor: "surface.highest" },
            }}
          >
            <Typography variant="subtitle2" sx={{ flex: 1, fontWeight: 600 }}>
              Advanced
            </Typography>
            <ExpandMoreRoundedIcon
              sx={{
                color: "text.secondary",
                transition: "transform 120ms",
                transform: advanced ? "rotate(180deg)" : "rotate(-90deg)",
              }}
            />
          </Box>
          <Collapse in={advanced}>
            <Box sx={{ px: 2, pb: 2, display: "flex", flexDirection: "column", gap: 1.5 }}>
              <Field label="Charset">
                <TextField
                  select
                  value={line.charset}
                  onChange={(e) => set("charset", e.target.value)}
                >
                  {charsets.map((c) => (
                    <MenuItem key={c.value} value={c.value}>
                      {c.label}
                    </MenuItem>
                  ))}
                </TextField>
              </Field>
              <Field label="Data bits">
                <ToggleButtonGroup
                  exclusive
                  fullWidth
                  value={line.dataBits}
                  onChange={(_e, v: SerialLine["dataBits"] | null) => {
                    if (v) set("dataBits", v);
                  }}
                >
                  {([8, 7, 6, 5] as const).map((b) => (
                    <ToggleButton key={b} value={b}>
                      {b}
                    </ToggleButton>
                  ))}
                </ToggleButtonGroup>
              </Field>
              <Field label="Stop bits">
                <ToggleButtonGroup
                  exclusive
                  fullWidth
                  value={line.stopBits}
                  onChange={(_e, v: SerialLine["stopBits"] | null) => {
                    if (v) set("stopBits", v);
                  }}
                >
                  {([1, 2] as const).map((b) => (
                    <ToggleButton key={b} value={b}>
                      {b}
                    </ToggleButton>
                  ))}
                </ToggleButtonGroup>
              </Field>
              <Field label="Flow Control">
                <ToggleButtonGroup
                  exclusive
                  fullWidth
                  value={line.flowControl}
                  onChange={(_e, v: SerialLine["flowControl"] | null) => {
                    if (v) set("flowControl", v);
                  }}
                >
                  <ToggleButton value="none">None</ToggleButton>
                  <ToggleButton value="software">XON/XOFF</ToggleButton>
                  <ToggleButton value="hardware">RTS/CTS</ToggleButton>
                </ToggleButtonGroup>
              </Field>
              <Field label="Parity">
                <ToggleButtonGroup
                  exclusive
                  fullWidth
                  value={line.parity}
                  onChange={(_e, v: SerialLine["parity"] | null) => {
                    if (v) set("parity", v);
                  }}
                >
                  <ToggleButton value="none">None</ToggleButton>
                  <ToggleButton value="even">Even</ToggleButton>
                  <ToggleButton value="odd">Odd</ToggleButton>
                </ToggleButtonGroup>
              </Field>
            </Box>
          </Collapse>
        </Box>

        <Box sx={{ display: "flex", justifyContent: "space-between", mt: 1 }}>
          <Button variant="tonal" onClick={goHome} sx={{ minWidth: 88 }}>
            Close
          </Button>
          <Button type="submit" variant="contained" disabled={!canConnect} sx={{ minWidth: 112 }}>
            Connect
          </Button>
        </Box>
      </Box>
    </Box>
  );
}
