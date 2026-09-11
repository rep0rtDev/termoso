import { Box, Typography } from "@mui/material";
import type { ReactNode } from "react";

interface Props {
  title: string;
  description?: ReactNode;
  actions?: ReactNode;
}

export function PageHeader({ title, description, actions }: Props) {
  return (
    <Box
      sx={{
        px: 2.5,
        py: 1.75,
        borderBottom: 1,
        borderColor: "divider",
        display: "flex",
        alignItems: "center",
        gap: 2,
      }}
    >
      <Box sx={{ flex: 1, minWidth: 0 }}>
        <Typography variant="h5">{title}</Typography>
        {description && (
          <Typography variant="body2" color="text.secondary">
            {description}
          </Typography>
        )}
      </Box>
      {actions && <Box sx={{ display: "flex", gap: 1, flexShrink: 0 }}>{actions}</Box>}
    </Box>
  );
}

export function PageBody({ children }: { children: ReactNode }) {
  return <Box sx={{ flex: 1, minHeight: 0, overflowY: "auto", px: 2.5, pb: 3 }}>{children}</Box>;
}

export function Page({ children }: { children: ReactNode }) {
  return (
    <Box sx={{ flex: 1, minHeight: 0, display: "flex", flexDirection: "column" }}>{children}</Box>
  );
}
