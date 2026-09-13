import { Box, Typography } from "@mui/material";
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

/** Card one layer above the page: header row (title, description, actions) + optional body. */
export function Section({
  id,
  title,
  description,
  actions,
  children,
  danger,
  disablePadding,
}: Props) {
  const hasBody = children !== undefined && children !== null && children !== false;
  return (
    <Box
      id={id}
      sx={{
        mb: 2,
        bgcolor: "surface.high",
        borderRadius: 2.5,
        overflow: "hidden",
        scrollMarginTop: 72,
        ...(danger && { boxShadow: (t) => `inset 0 0 0 1px ${t.palette.error.main}40` }),
      }}
    >
      <Box
        sx={{
          display: "flex",
          alignItems: { xs: "flex-start", sm: "center" },
          flexDirection: { xs: "column", sm: "row" },
          justifyContent: "space-between",
          gap: 1.5,
          px: 2.5,
          py: 2,
          minHeight: 64,
        }}
      >
        <Box sx={{ minWidth: 0 }}>
          <Typography
            variant="subtitle1"
            component="h2"
            color={danger ? "error.main" : "text.primary"}
          >
            {title}
          </Typography>
          {description && (
            <Typography variant="body2" color="text.secondary" sx={{ mt: 0.25 }}>
              {description}
            </Typography>
          )}
        </Box>
        {actions && (
          <Box sx={{ display: "flex", gap: 1, flexWrap: "wrap", flexShrink: 0 }}>{actions}</Box>
        )}
      </Box>
      {hasBody && (
        <Box
          sx={{
            borderTop: 1,
            borderColor: "border.light",
            ...(disablePadding ? {} : { px: 2.5, py: 2 }),
            "&:empty": { display: "none" },
          }}
        >
          {children}
        </Box>
      )}
    </Box>
  );
}

/** Label/value row inside a Section body (settings-style). */
export function SettingRow({
  label,
  description,
  control,
  divider = true,
}: {
  label: ReactNode;
  description?: ReactNode;
  control: ReactNode;
  divider?: boolean;
}) {
  return (
    <Box
      sx={{
        display: "flex",
        alignItems: "center",
        gap: 2,
        py: 1.25,
        ...(divider && { borderBottom: 1, borderColor: "border.light" }),
        "&:last-of-type": { borderBottom: 0 },
      }}
    >
      <Box sx={{ flex: 1, minWidth: 0 }}>
        <Typography variant="body1">{label}</Typography>
        {description && (
          <Typography variant="body2" color="text.secondary" sx={{ mt: 0.25 }}>
            {description}
          </Typography>
        )}
      </Box>
      <Box sx={{ flexShrink: 0, display: "flex", alignItems: "center", gap: 1 }}>{control}</Box>
    </Box>
  );
}
