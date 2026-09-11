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
    <Box sx={{ textAlign: "center", py: 6, px: 2, color: "text.secondary" }}>
      {icon && <Box sx={{ fontSize: 40, mb: 1, "& svg": { fontSize: 40 } }}>{icon}</Box>}
      <Typography variant="h5" color="text.primary">
        {title}
      </Typography>
      {description && (
        <Typography variant="body2" sx={{ mt: 0.5, maxWidth: 420, mx: "auto" }}>
          {description}
        </Typography>
      )}
      {action && <Box sx={{ mt: 2 }}>{action}</Box>}
    </Box>
  );
}
