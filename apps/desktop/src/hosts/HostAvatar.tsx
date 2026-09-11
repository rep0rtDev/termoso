import { Avatar } from "@mui/material";
import TerminalRoundedIcon from "@mui/icons-material/TerminalRounded";
import type { HostCard } from "@/ipc/types";

const palette = ["#2BB884", "#5AA9E6", "#F2C94C", "#B07CF2", "#F28C5A", "#5FD0A4", "#8CC5F0"];

function hue(s: string): string {
  let h = 0;
  for (const c of s) h = (h * 31 + c.charCodeAt(0)) >>> 0;
  return palette[h % palette.length] ?? "#2BB884";
}

export function HostAvatar({ host, size = 36 }: { host: HostCard; size?: number }) {
  const color = hue(host.id);
  return (
    <Avatar
      variant="rounded"
      sx={{
        width: size,
        height: size,
        bgcolor: `${color}26`,
        color,
        fontSize: size * 0.42,
        fontWeight: 700,
        borderRadius: size / 4,
      }}
    >
      {host.protocol === "telnet" ? (
        <TerminalRoundedIcon fontSize="small" />
      ) : (
        (host.label.trim()[0] ?? host.address[0] ?? "?").toUpperCase()
      )}
    </Avatar>
  );
}
