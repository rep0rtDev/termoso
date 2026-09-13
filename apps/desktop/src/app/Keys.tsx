import { Box } from "@mui/material";
import { chordParts } from "./keymap";

/** Shortcut rendered as key caps: `Ctrl` `Shift` `K`. */
export function Keys({ chord }: { chord: string }) {
  const parts = chordParts(chord);
  return (
    <Box sx={{ display: "flex", gap: 0.5, flexShrink: 0 }}>
      {parts.map((p, i) => (
        <Box
          key={i}
          component="kbd"
          sx={{
            fontFamily: "inherit",
            fontSize: 11,
            lineHeight: "18px",
            px: 0.75,
            borderRadius: 1,
            bgcolor: "surface.highest",
            color: "text.secondary",
            boxShadow: "inset 0 -1px 0 rgba(0,0,0,0.25)",
          }}
        >
          {p}
        </Box>
      ))}
    </Box>
  );
}
