import { Card, CardContent, Box, Typography, Divider } from "@mui/material";
import type { ReactNode } from "react";

interface Props {
  id?: string;
  title: ReactNode;
  description?: ReactNode;
  actions?: ReactNode;
  children?: ReactNode;
  danger?: boolean;
  disablePadding?: boolean;
}

export function Section({
  id,
  title,
  description,
  actions,
  children,
  danger,
  disablePadding,
}: Props) {
  return (
    <Card
      id={id}
      sx={{
        mb: 2.5,
        borderColor: danger ? "error.main" : undefined,
        overflow: "visible",
      }}
    >
      <Box
        sx={{
          display: "flex",
          alignItems: { xs: "flex-start", sm: "center" },
          flexDirection: { xs: "column", sm: "row" },
          justifyContent: "space-between",
          gap: 1.5,
          px: 3,
          py: 2,
        }}
      >
        <Box>
          <Typography variant="h5" component="h2" color={danger ? "error.main" : undefined}>
            {title}
          </Typography>
          {description && (
            <Typography variant="body2" color="text.secondary" sx={{ mt: 0.25 }}>
              {description}
            </Typography>
          )}
        </Box>
        {actions && <Box sx={{ display: "flex", gap: 1, flexWrap: "wrap" }}>{actions}</Box>}
      </Box>
      {children !== undefined && children !== null && (
        <>
          <Divider />
          {disablePadding ? (
            children
          ) : (
            <CardContent sx={{ px: 3, py: 2.5 }}>{children}</CardContent>
          )}
        </>
      )}
    </Card>
  );
}
