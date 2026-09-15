import { useEffect, useState } from "react";
import { Avatar } from "@mui/material";
import { accountApi } from "@/api/endpoints";

/**
 * Object URLs for downloaded pictures, keyed by `user:tag`. The tag changes
 * with the picture, so an entry never goes stale; the browser's HTTP cache
 * (the server marks tag-pinned responses immutable) covers reloads.
 */
const urls = new Map<string, Promise<string | null>>();

function avatarUrl(userId: string, tag: string): Promise<string | null> {
  const key = `${userId}:${tag}`;
  let p = urls.get(key);
  if (!p) {
    p = accountApi
      .avatar(userId, tag)
      .then((blob) => (blob ? URL.createObjectURL(blob) : null))
      .catch(() => null);
    urls.set(key, p);
  }
  return p;
}

/** Resolves to an `<img>` source for the user's picture, or `undefined` while loading / when none. */
export function useAvatarUrl(userId: string | undefined, tag: string | undefined) {
  const key = userId && tag ? `${userId}:${tag}` : undefined;
  const [loaded, setLoaded] = useState<{ key: string; url: string } | undefined>();
  useEffect(() => {
    if (!userId || !tag) return;
    let live = true;
    void avatarUrl(userId, tag).then((url) => {
      if (live && url) setLoaded({ key: `${userId}:${tag}`, url });
    });
    return () => {
      live = false;
    };
  }, [userId, tag]);
  return key !== undefined && loaded?.key === key ? loaded.url : undefined;
}

interface Props {
  userId: string;
  /** `UserProfile.avatar` / `TeamMember.avatar`; letter fallback when absent. */
  tag?: string;
  email: string;
  displayName?: string;
  size?: number;
}

export function UserAvatar({ userId, tag, email, displayName, size = 32 }: Props) {
  const url = useAvatarUrl(userId, tag);
  const initial = (displayName ?? email).trim().charAt(0).toUpperCase();
  return (
    <Avatar src={url} alt="" sx={{ width: size, height: size, fontSize: Math.round(size * 0.42) }}>
      {initial}
    </Avatar>
  );
}
