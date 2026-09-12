import { Box, Typography } from "@mui/material";
import type { ReactNode } from "react";

interface Props {
  icon?: ReactNode;
  title: string;
  description?: ReactNode;
  action?: ReactNode;
}

export function EmptyState({ icon, title, description, action }: Props) {
  return (
    <Box sx={{ textAlign: "center", py: 6, px: 2 }}>
      {icon && (
        <Box
          sx={{
            width: 44,
            height: 44,
            borderRadius: 2,
            mx: "auto",
            mb: 1.5,
            display: "grid",
            placeItems: "center",
            bgcolor: "surface.highest",
            color: "text.secondary",
            "& svg": { fontSize: 22 },
          }}
        >
          {icon}
        </Box>
      )}
      <Typography variant="subtitle1">{title}</Typography>
      {description && (
        <Typography
          variant="body2"
          color="text.secondary"
          sx={{ mt: 0.5, maxWidth: 400, mx: "auto" }}
        >
          {description}
        </Typography>
      )}
      {action && <Box sx={{ mt: 2 }}>{action}</Box>}
    </Box>
  );
}
