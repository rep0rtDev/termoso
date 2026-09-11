import DnsRoundedIcon from "@mui/icons-material/DnsRounded";
import TerminalRoundedIcon from "@mui/icons-material/TerminalRounded";
import SvgIcon, { type SvgIconProps } from "@mui/material/SvgIcon";
import type { HostCard } from "@/ipc/types";
import { IconTile } from "@/components/ui";
import { sizes } from "@/theme/theme";
import { distroIcon, type DistroIcon } from "./distroIcons";

/** Renders a distro glyph as a regular MUI icon (inherits `fontSize` / `color`). */
export function DistroGlyph({ icon, ...props }: { icon: DistroIcon } & SvgIconProps) {
  return (
    <SvgIcon viewBox="0 0 24 24" titleAccess={icon.title} {...props}>
      <path d={icon.path} />
    </SvgIcon>
  );
}

/** Glyph for a host's detected OS, or the protocol fallback while unknown. */
export function HostGlyph({
  osName,
  protocol,
  ...props
}: {
  osName: string | null | undefined;
  protocol: HostCard["protocol"];
} & SvgIconProps) {
  const icon = distroIcon(osName);
  if (icon) return <DistroGlyph icon={icon} {...props} />;
  return protocol === "telnet" ? <TerminalRoundedIcon {...props} /> : <DnsRoundedIcon {...props} />;
}

/** Host tile: brand-coloured with the distro logo once the OS is known, neutral before. */
export function HostAvatar({ host, size = sizes.tile }: { host: HostCard; size?: number }) {
  const icon = distroIcon(host.osName);
  return (
    <IconTile size={size} color={icon?.color}>
      <HostGlyph osName={host.osName} protocol={host.protocol} />
    </IconTile>
  );
}
