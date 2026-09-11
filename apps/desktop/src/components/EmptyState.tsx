import { Box, Typography } from "@mui/material";
import type { ReactNode } from "react";

interface Props {
  icon?: ReactNode;
  title: string;
  description?: ReactNode;
  action?: ReactNode;
  compact?: boolean;
}

export function EmptyState({ icon, title, description, action, compact }: Props) {
  return (
    <Box
      sx={{
        textAlign: "center",
        py: compact ? 4 : 8,
        px: 2,
        color: "text.secondary",
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
      }}
    >
      {icon && (
        <Box
          sx={{
            width: 48,
            height: 48,
            borderRadius: 2,
            bgcolor: "surface.high",
            display: "grid",
            placeItems: "center",
            mb: 1.5,
            "& svg": { fontSize: 24, color: "text.secondary" },
          }}
        >
          {icon}
        </Box>
      )}
      <Typography variant="subtitle1" color="text.primary">
        {title}
      </Typography>
      {description && (
        <Typography variant="body2" sx={{ mt: 0.5, maxWidth: 380 }}>
          {description}
        </Typography>
      )}
      {action && <Box sx={{ mt: 2 }}>{action}</Box>}
    </Box>
  );
}
