import { Box, Typography } from "@mui/material";
import type { ReactNode } from "react";

interface Props {
  title: string;
  subtitle?: ReactNode;
  actions?: ReactNode;
  /** Bottom margin in theme spacing units. */
  mb?: number;
}

export function PageHeader({ title, subtitle, actions, mb = 3 }: Props) {
  return (
    <Box
      sx={{
        display: "flex",
        alignItems: { xs: "flex-start", sm: "center" },
        flexDirection: { xs: "column", sm: "row" },
        justifyContent: "space-between",
        gap: 2,
        mb,
      }}
    >
      <Box>
        <Typography variant="h1" component="h1" sx={{ fontSize: { xs: "1.5rem", sm: "1.75rem" } }}>
          {title}
        </Typography>
        {subtitle && (
          <Typography variant="body2" color="text.secondary" sx={{ mt: 0.5 }}>
            {subtitle}
          </Typography>
        )}
      </Box>
      {actions && <Box sx={{ display: "flex", gap: 1, flexWrap: "wrap" }}>{actions}</Box>}
    </Box>
  );
}
