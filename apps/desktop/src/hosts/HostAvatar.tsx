import DnsRoundedIcon from "@mui/icons-material/DnsRounded";
import TerminalRoundedIcon from "@mui/icons-material/TerminalRounded";
import type { HostCard } from "@/ipc/types";
import { IconTile } from "@/components/ui";
import { sizes } from "@/theme/theme";

/** Recognisable OS names get a small brand colour on the tile; everything else stays neutral. */
const osColors: [RegExp, string][] = [
  [/ubuntu/i, "#DD4814"],
  [/debian/i, "#A81D33"],
  [/fedora|red ?hat|rhel|centos|rocky|alma/i, "#CC0000"],
  [/arch/i, "#1793D1"],
  [/alpine/i, "#0D597F"],
  [/suse/i, "#73BA25"],
  [/freebsd|openbsd|netbsd/i, "#AB2B28"],
  [/mac|darwin/i, "#8E8E93"],
  [/windows/i, "#0078D4"],
];

function osColor(os: string | null): string | undefined {
  if (!os) return undefined;
  return osColors.find(([re]) => re.test(os))?.[1];
}

export function HostAvatar({ host, size = sizes.tile }: { host: HostCard; size?: number }) {
  return (
    <IconTile size={size} color={osColor(host.osName)}>
      {host.protocol === "telnet" ? <TerminalRoundedIcon /> : <DnsRoundedIcon />}
    </IconTile>
  );
}
