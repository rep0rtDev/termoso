import { Box } from "@mui/material";
import type { ReactNode } from "react";

export function IconTile({
  children,
  size = 36,
  active = false,
}: {
  children: ReactNode;
  size?: number;
  active?: boolean;
}) {
  return (
    <Box
      sx={{
        width: size,
        height: size,
        flexShrink: 0,
        borderRadius: 2,
        display: "grid",
        placeItems: "center",
        bgcolor: "surface.highest",
        color: active ? "primary.main" : "text.secondary",
        "& svg": { fontSize: Math.round(size * 0.55) },
      }}
    >
      {children}
    </Box>
  );
}
