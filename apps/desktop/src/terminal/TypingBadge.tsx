import { useEffect, useState } from "react";
import Box from "@mui/material/Box";
import Tooltip from "@mui/material/Tooltip";
import { PersonAvatar } from "@/team/PersonAvatar";
import type { Uuid } from "@/ipc/types";
import { useShare, useTyping } from "./multiplayer";
import { cursorPixels, getRuntime } from "./store";

const SIZE = 18;

/**
 * Multiplayer: a small avatar of whoever is typing right now, pinned just
 * above the cursor cell. Follows the cursor while output arrives; the store
 * drops the entry shortly after the person stops typing.
 */
export function TypingBadge({ paneId }: { paneId: Uuid }) {
  const typing = useTyping(paneId);
  const share = useShare(paneId);
  const [pos, setPos] = useState<{ left: number; top: number } | null>(null);
  const active = typing !== null;

  useEffect(() => {
    if (!active) return;
    const rt = getRuntime(paneId);
    if (!rt) return;
    const place = () => {
      const p = cursorPixels(rt);
      setPos(p ? { left: p.left, top: Math.max(0, p.top - SIZE - 2) } : null);
    };
    const first = requestAnimationFrame(place);
    const sub = rt.term.onRender(place);
    return () => {
      cancelAnimationFrame(first);
      sub.dispose();
    };
  }, [active, paneId]);

  if (!typing || !pos) return null;
  const who = share?.participants.find((p) => p.userId === typing.userId);
  if (!who) return null;
  const label = who.displayName ?? who.email;
  return (
    <Tooltip title={label} placement="top">
      <Box
        sx={{
          position: "absolute",
          left: pos.left,
          top: pos.top,
          pointerEvents: "auto",
          zIndex: 3,
          borderRadius: "50%",
          boxShadow: (t) => `0 0 0 1.5px ${t.palette.background.paper}`,
        }}
      >
        <PersonAvatar label={label} size={SIZE} userId={who.userId} avatar={who.avatar} />
      </Box>
    </Tooltip>
  );
}
