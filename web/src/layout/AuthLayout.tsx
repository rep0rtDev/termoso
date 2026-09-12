import { Box, Link, Typography } from "@mui/material";
import { Link as RouterLink, Outlet } from "react-router";
import { Logo } from "@/components/Logo";
import { ThemeToggle } from "./ThemeToggle";
import { useServerInfo } from "@/api/hooks";

export function AuthLayout() {
  const info = useServerInfo();
  return (
    <Box
      sx={{
        minHeight: "100vh",
        display: "flex",
        flexDirection: "column",
        bgcolor: "surface.lowest",
      }}
    >
      <Box
        component="header"
        sx={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          px: { xs: 2, sm: 4 },
          height: 60,
        }}
      >
        <Box
          component={RouterLink}
          to="/"
          sx={{ textDecoration: "none", color: "inherit", display: "flex" }}
        >
          <Logo size={26} />
        </Box>
        <ThemeToggle />
      </Box>
      <Box
        sx={{
          flex: 1,
          display: "flex",
          alignItems: "flex-start",
          justifyContent: "center",
          px: 2,
          pt: { xs: 2, sm: 6 },
          pb: 6,
        }}
      >
        <Box
          sx={{
            width: "100%",
            maxWidth: 440,
            bgcolor: "surface.base",
            borderRadius: 3,
            p: { xs: 3, sm: 4 },
          }}
        >
          <Outlet />
        </Box>
      </Box>
      <Box component="footer" sx={{ textAlign: "center", py: 2.5, color: "text.disabled" }}>
        <Typography variant="caption">
          {info.data?.name ?? "Termoso"} · self-hosted, open source ·{" "}
          <Link
            href="https://github.com/rep0rtDev/termoso"
            target="_blank"
            rel="noreferrer"
            color="inherit"
          >
            source
          </Link>
        </Typography>
      </Box>
    </Box>
  );
}
