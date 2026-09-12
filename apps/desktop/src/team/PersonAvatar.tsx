import { Box } from "@mui/material";
import PersonOutlineRoundedIcon from "@mui/icons-material/PersonOutlineRounded";
import MailOutlineRoundedIcon from "@mui/icons-material/MailOutlineRounded";

export const AVATAR = 28;

export function initialsOf(name: string | null | undefined, email: string): string {
  const n = name?.trim();
  if (n) {
    return n
      .split(/\s+/)
      .slice(0, 2)
      .map((p) => p[0] ?? "")
      .join("")
      .toUpperCase();
  }
  return email.slice(0, 1).toUpperCase();
}

/**
 * Square initials tile for a person. `kind`: `"account"` is a signed-in user,
 * `"guest"` the empty state when nobody is signed in, `"invite"` an e-mail
 * address that has not joined yet.
 */
export function PersonAvatar({
  label,
  size = AVATAR,
  kind = "account",
}: {
  label: string;
  size?: number;
  kind?: "account" | "guest" | "invite";
}) {
  return (
    <Box
      sx={{
        width: size,
        height: size,
        borderRadius: size >= AVATAR ? "7px" : "6px",
        display: "grid",
        placeItems: "center",
        fontSize: Math.round(size * 0.42),
        fontWeight: 600,
        letterSpacing: "0.02em",
        bgcolor: kind === "account" ? "primary.dark" : "surface.highest",
        color: kind === "account" ? "primary.contrastText" : "text.secondary",
        flexShrink: 0,
      }}
    >
      {kind === "account" ? (
        label
      ) : kind === "invite" ? (
        <MailOutlineRoundedIcon sx={{ fontSize: Math.round(size * 0.55) }} />
      ) : (
        <PersonOutlineRoundedIcon sx={{ fontSize: Math.round(size * 0.6) }} />
      )}
    </Box>
  );
}
