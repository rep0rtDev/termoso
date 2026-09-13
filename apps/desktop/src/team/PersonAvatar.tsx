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

/** Termius-like per-person tile colours; picked by a stable hash of `seed` (usually the e-mail). */
const HUES = [
  "#E8B27A",
  "#F5C518",
  "#F7A8D8",
  "#7CC4FA",
  "#8ED08E",
  "#C7A4F5",
  "#F58F8F",
  "#6ED7C5",
];

export function personColor(seed: string): string {
  let h = 0;
  for (const ch of seed.toLowerCase()) h = (h * 31 + ch.charCodeAt(0)) >>> 0;
  return HUES[h % HUES.length] ?? "#7CC4FA";
}

/**
 * Square initials tile for a person. `kind`: `"account"` is a signed-in user,
 * `"guest"` the empty state when nobody is signed in, `"invite"` an e-mail
 * address that has not joined yet. Pass `seed` (e-mail) to get the person's
 * stable colour tile with their initial, like Termius, instead of the accent.
 */
export function PersonAvatar({
  label,
  size = AVATAR,
  kind = "account",
  seed,
}: {
  label: string;
  size?: number;
  kind?: "account" | "guest" | "invite";
  seed?: string;
}) {
  const tinted = seed !== undefined && kind !== "guest";
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
        bgcolor: tinted
          ? personColor(seed)
          : kind === "account"
            ? "primary.dark"
            : "surface.highest",
        color: tinted ? "#fff" : kind === "account" ? "primary.contrastText" : "text.secondary",
        flexShrink: 0,
      }}
    >
      {kind === "account" || (tinted && label) ? (
        label
      ) : kind === "invite" ? (
        <MailOutlineRoundedIcon sx={{ fontSize: Math.round(size * 0.55) }} />
      ) : (
        <PersonOutlineRoundedIcon sx={{ fontSize: Math.round(size * 0.6) }} />
      )}
    </Box>
  );
}
