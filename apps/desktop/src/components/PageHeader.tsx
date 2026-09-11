import { Box } from "@mui/material";
import type { ReactNode } from "react";
import { Toolbar } from "./ui";

/** Page toolbar under the top bar: primary actions left, filters / view controls right. */
export function PageHeader({ actions, trailing }: { actions?: ReactNode; trailing?: ReactNode }) {
  return <Toolbar trailing={trailing}>{actions}</Toolbar>;
}

export function PageBody({ children, padded = true }: { children: ReactNode; padded?: boolean }) {
  return (
    <Box
      sx={{
        flex: 1,
        minHeight: 0,
        overflowY: "auto",
        px: padded ? 2 : 0,
        py: padded ? 2 : 0,
        display: "flex",
        flexDirection: "column",
        gap: 1.5,
      }}
    >
      {children}
    </Box>
  );
}

export function Page({ children }: { children: ReactNode }) {
  return (
    <Box sx={{ flex: 1, minHeight: 0, display: "flex", flexDirection: "column" }}>{children}</Box>
  );
}
