import { Box, CircularProgress } from "@mui/material";

export function Loading({ minHeight = 200 }: { minHeight?: number | string }) {
  return (
    <Box sx={{ display: "grid", placeItems: "center", minHeight }}>
      <CircularProgress size={28} />
    </Box>
  );
}
