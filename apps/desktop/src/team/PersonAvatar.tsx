import { useEffect, useState } from "react";
import { Box } from "@mui/material";
import PersonOutlineRoundedIcon from "@mui/icons-material/PersonOutlineRounded";
import MailOutlineRoundedIcon from "@mui/icons-material/MailOutlineRounded";
import { userAvatar } from "@/ipc/commands";
import type { Uuid } from "@/ipc/types";

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

/** Blob URLs of pictures already fetched, keyed by `user:tag`; the tag changes with the picture. */
const pictures = new Map<string, Promise<string | null>>();
/** Cache key currently held per user, so a replaced picture releases its blob. */
const currentKey = new Map<Uuid, string>();

function fetchPicture(userId: Uuid, tag: string): Promise<string | null> {
  const k = `${userId}:${tag}`;
  let p = pictures.get(k);
  if (p) return p;
  const previous = currentKey.get(userId);
  if (previous !== undefined && previous !== k) {
    const old = pictures.get(previous);
    pictures.delete(previous);
    void old?.then((url) => {
      if (url) URL.revokeObjectURL(url);
    });
  }
  currentKey.set(userId, k);
  p = userAvatar(userId, tag)
    .then((bytes) =>
      bytes.byteLength > 0 ? URL.createObjectURL(new Blob([bytes], { type: "image/webp" })) : null,
    )
    .catch(() => null);
  pictures.set(k, p);
  return p;
}

/** The user's picture as an `<img>` source, or `undefined` while loading / when they have none. */
export function useAvatarUrl(userId: Uuid | undefined, tag: string | null | undefined) {
  const key = userId && tag ? `${userId}:${tag}` : undefined;
  const [loaded, setLoaded] = useState<{ key: string; url: string }>();
  useEffect(() => {
    if (!userId || !tag) return;
    const k = `${userId}:${tag}`;
    const p = fetchPicture(userId, tag);
    let live = true;
    void p.then((url) => {
      if (live && url) setLoaded({ key: k, url });
    });
    return () => {
      live = false;
    };
  }, [userId, tag]);
  return key !== undefined && loaded?.key === key ? loaded.url : undefined;
}

/**
 * Square initials tile for a person. `kind`: `"account"` is a signed-in user,
 * `"guest"` the empty state when nobody is signed in, `"invite"` an e-mail
 * address that has not joined yet. Pass `seed` (e-mail) to get the person's
 * stable colour tile with their initial, like Termius, instead of the accent.
 * With `userId` + `avatar` (the picture tag) the uploaded picture replaces
 * the initials once it has loaded.
 */
export function PersonAvatar({
  label,
  size = AVATAR,
  kind = "account",
  seed,
  userId,
  avatar,
}: {
  label: string;
  size?: number;
  kind?: "account" | "guest" | "invite";
  seed?: string;
  userId?: Uuid;
  avatar?: string | null;
}) {
  const tinted = seed !== undefined && kind !== "guest";
  const picture = useAvatarUrl(userId, avatar);
  return (
    <Box
      sx={{
        width: size,
        height: size,
        borderRadius: size >= AVATAR ? "7px" : "6px",
        display: "grid",
        placeItems: "center",
        overflow: "hidden",
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
      {picture ? (
        <Box
          component="img"
          src={picture}
          alt=""
          draggable={false}
          sx={{ width: "100%", height: "100%", objectFit: "cover", display: "block" }}
        />
      ) : kind === "account" || (tinted && label) ? (
        label
      ) : kind === "invite" ? (
        <MailOutlineRoundedIcon sx={{ fontSize: Math.round(size * 0.55) }} />
      ) : (
        <PersonOutlineRoundedIcon sx={{ fontSize: Math.round(size * 0.6) }} />
      )}
    </Box>
  );
}
