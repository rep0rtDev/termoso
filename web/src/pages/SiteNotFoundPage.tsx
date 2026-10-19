import { Box, Button, Container, Stack, Typography, alpha } from "@mui/material";
import { Link as RouterLink } from "react-router";
import { emerald } from "@/theme/theme";
import { LandingFooter, LandingHeader, Section } from "./landing/Shell";

export function SiteNotFoundPage() {
  return (
    <Box sx={{ minHeight: "100vh", bgcolor: "surface.lowest", overflowX: "hidden" }}>
      <LandingHeader />
      <Box sx={{ position: "relative" }}>
        <Box
          aria-hidden
          sx={{
            position: "absolute",
            inset: 0,
            pointerEvents: "none",
            background: `radial-gradient(60% 50% at 50% 0%, ${alpha(emerald.main, 0.18)} 0%, transparent 70%)`,
          }}
        />
        <Container maxWidth="lg" sx={{ position: "relative" }}>
          <Section sx={{ minHeight: "60vh", display: "flex", alignItems: "center" }}>
            <Box sx={{ maxWidth: 640 }}>
              <Typography
                variant="overline"
                component="div"
                color="primary.main"
                sx={{ mb: 1.25, letterSpacing: "0.1em" }}
              >
                404
              </Typography>
              <Typography
                variant="h1"
                component="h1"
                sx={{ fontSize: { xs: "2.25rem", md: "3.25rem" }, letterSpacing: "-0.03em" }}
              >
                Page not found
              </Typography>
              <Typography
                variant="body1"
                color="text.secondary"
                sx={{ mt: 2.5, fontSize: "1.125rem", lineHeight: 1.6 }}
              >
                The address may be mistyped, or the page has moved. The docs and the download links
                are one click away.
              </Typography>
              <Stack direction={{ xs: "column", sm: "row" }} spacing={1.5} sx={{ mt: 4 }}>
                <Button component={RouterLink} to="/" variant="contained" size="large">
                  Back to the start
                </Button>
                <Button component={RouterLink} to="/docs" variant="outlined" size="large">
                  Open the docs
                </Button>
              </Stack>
            </Box>
          </Section>
        </Container>
      </Box>
      <LandingFooter />
    </Box>
  );
}
