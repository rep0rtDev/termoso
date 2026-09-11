import { Box, Typography } from "@mui/material";

export function LogoMark({ size = 32 }: { size?: number }) {
  return (
    <Box
      component="svg"
      viewBox="0 0 64 64"
      sx={{ width: size, height: size, flexShrink: 0 }}
      aria-hidden
    >
      <rect width="64" height="64" rx="14" fill="#12151F" />
      <path d="M16 20h32v8H36v20h-8V28H16z" fill="#2BB884" />
      <rect x="42" y="40" width="8" height="8" rx="2" fill="#5AA9E6" />
    </Box>
  );
}

export function Logo({ size = 32, showName = true }: { size?: number; showName?: boolean }) {
  return (
    <Box sx={{ display: "flex", alignItems: "center", gap: 1.25 }}>
      <LogoMark size={size} />
      {showName && (
        <Typography
          variant="h6"
          component="span"
          sx={{ fontWeight: 700, letterSpacing: "-0.02em", lineHeight: 1 }}
        >
          Termoso
        </Typography>
      )}
    </Box>
  );
}
