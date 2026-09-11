import { Box, Container, Link, Paper, Typography } from "@mui/material";
import { Outlet } from "react-router";
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
        background: `radial-gradient(1200px 600px at 10% -10%, rgba(var(--mui-palette-primary-mainChannel) / 0.14), transparent 60%),
          radial-gradient(900px 500px at 110% 110%, rgba(var(--mui-palette-secondary-mainChannel) / 0.12), transparent 60%)`,
      }}
    >
      <Box
        component="header"
        sx={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          px: { xs: 2, sm: 4 },
          py: 2,
        }}
      >
        <Logo />
        <ThemeToggle />
      </Box>
      <Container maxWidth="sm" sx={{ flex: 1, display: "flex", alignItems: "center", py: 4 }}>
        <Paper variant="outlined" sx={{ width: "100%", p: { xs: 3, sm: 4 }, borderRadius: 4 }}>
          <Outlet />
        </Paper>
      </Container>
      <Box component="footer" sx={{ textAlign: "center", py: 2, color: "text.secondary" }}>
        <Typography variant="caption">
          {info.data?.name ?? "Termoso"} · self-hosted, open source ·{" "}
          <Link href="https://github.com/rep0rtDev/termoso" target="_blank" rel="noreferrer">
            source
          </Link>
        </Typography>
      </Box>
    </Box>
  );
}
