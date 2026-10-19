import ArrowBackRoundedIcon from "@mui/icons-material/ArrowBackRounded";
import { Box, Button, Container, Divider, Link, Stack, Typography } from "@mui/material";
import { Link as RouterLink } from "react-router";

import { Logo } from "@/components/Logo";

const EMAIL = "hello@termoso.dev";
const REPO = "https://github.com/termoso/termoso";

function Tier({ title, text, items }: { title: string; text: string; items: string[] }) {
  return (
    <Box
      sx={{
        p: 3,
        borderRadius: 3,
        bgcolor: "surface.low",
        border: 1,
        borderColor: "border.light",
        height: "100%",
      }}
    >
      <Typography variant="h6">{title}</Typography>
      <Typography variant="body2" color="text.secondary" sx={{ mt: 0.75 }}>
        {text}
      </Typography>
      <Stack component="ul" spacing={0.75} sx={{ mt: 2, pl: 2.25, m: 0 }}>
        {items.map((it) => (
          <Typography key={it} component="li" variant="body2">
            {it}
          </Typography>
        ))}
      </Stack>
    </Box>
  );
}

/** `/sponsor` — public page linked from the landing and GitHub's Sponsor button. */
export function SponsorPage() {
  return (
    <Box sx={{ minHeight: "100vh", bgcolor: "surface.lowest" }}>
      <Box component="header" sx={{ borderBottom: 1, borderColor: "border.light" }}>
        <Container maxWidth="md" sx={{ height: 60, display: "flex", alignItems: "center", gap: 2 }}>
          <Link component={RouterLink} to="/" underline="none" sx={{ display: "flex" }}>
            <Logo size={26} />
          </Link>
          <Box sx={{ flex: 1 }} />
          <Button
            component={RouterLink}
            to="/"
            size="small"
            color="inherit"
            startIcon={<ArrowBackRoundedIcon />}
          >
            Back to termoso.dev
          </Button>
        </Container>
      </Box>

      <Container maxWidth="md" sx={{ py: { xs: 6, md: 10 } }}>
        <Typography
          variant="overline"
          sx={{ color: "primary.main", fontWeight: 600, letterSpacing: 1 }}
        >
          Sponsorship
        </Typography>
        <Typography variant="h3" sx={{ mt: 1, fontWeight: 700, letterSpacing: -0.5 }}>
          Support Termoso
        </Typography>
        <Typography variant="body1" color="text.secondary" sx={{ mt: 2, maxWidth: 680 }}>
          Termoso is free software under AGPL-3.0 with no paid tier and no vendor behind it.
          Everything — the clients, the server, the hosted cloud — is funded by the people and
          companies who rely on it. If it saves your team time, consider sponsoring; if your
          organisation runs it, we can back you with support and engineering.
        </Typography>
        <Stack direction={{ xs: "column", sm: "row" }} spacing={1.5} sx={{ mt: 3 }}>
          <Button
            href={`mailto:${EMAIL}?subject=Sponsoring%20Termoso`}
            variant="contained"
            size="large"
          >
            Write to {EMAIL}
          </Button>
          <Button href={REPO} target="_blank" rel="noreferrer" variant="outlined" size="large">
            Star on GitHub
          </Button>
        </Stack>

        <Box
          sx={{
            mt: { xs: 6, md: 8 },
            display: "grid",
            gridTemplateColumns: { xs: "1fr", md: "repeat(3, 1fr)" },
            gap: 2,
          }}
        >
          <Tier
            title="Individual sponsor"
            text="Any amount, one-off or monthly. You keep the project alive and get a say in what comes next."
            items={[
              "Name or logo in the README and release notes (or stay anonymous)",
              "Early access to pre-release builds",
              "Vote on roadmap priorities",
            ]}
          />
          <Tier
            title="Company sponsor"
            text="For teams that use Termoso every day and want it to stay maintained."
            items={[
              "Logo on termoso.dev and in the README",
              "Private channel with the maintainers",
              "Feature requests reviewed first",
              "Security advisories ahead of public disclosure",
            ]}
          />
          <Tier
            title="Enterprise support"
            text="For organisations running their own Termoso server. Scoped per agreement."
            items={[
              "Deployment and upgrade assistance, HA and backups",
              "SLA-backed support for the self-hosted server",
              "SSO / SCIM / SIEM integration work",
              "Security reviews and compliance documentation",
              "Features built on a schedule",
            ]}
          />
        </Box>

        <Divider sx={{ my: { xs: 6, md: 8 } }} />

        <Typography variant="h5">How it works</Typography>
        <Typography variant="body2" color="text.secondary" sx={{ mt: 1.5, maxWidth: 680 }}>
          There is no checkout form on purpose. Write to{" "}
          <Link href={`mailto:${EMAIL}`}>{EMAIL}</Link> and say who you are, how you use Termoso and
          what you have in mind — a one-off contribution, a monthly amount, or a support agreement
          for your organisation. You get an invoice or a sponsorship link back, usually within two
          business days. Security reports go to the address in{" "}
          <Link href={`${REPO}/blob/main/SECURITY.md`} target="_blank" rel="noreferrer">
            SECURITY.md
          </Link>{" "}
          instead.
        </Typography>
      </Container>

      <Box component="footer" sx={{ borderTop: 1, borderColor: "border.light", py: 4 }}>
        <Container maxWidth="md">
          <Typography variant="body2" color="text.secondary">
            Termoso · self-hosted, open source ·{" "}
            <Link href={REPO} target="_blank" rel="noreferrer">
              source
            </Link>
          </Typography>
        </Container>
      </Box>
    </Box>
  );
}
