import { useState } from "react";
import {
  Box,
  Button,
  Chip,
  Container,
  IconButton,
  Link,
  Stack,
  Tooltip,
  Typography,
  alpha,
} from "@mui/material";
import ContentCopyRoundedIcon from "@mui/icons-material/ContentCopyRounded";
import CheckRoundedIcon from "@mui/icons-material/CheckRounded";
import { QRCodeSVG } from "qrcode.react";

import { emerald, monoFontFamily } from "@/theme/theme";
import { Card, LandingFooter, LandingHeader, REPO, Section, SectionTitle } from "./landing/Shell";

const EMAIL = "hello@termoso.dev";

interface Wallet {
  asset: string;
  name: string;
  networks: { label: string; address: string }[];
}

const WALLETS: Wallet[] = [
  {
    asset: "USDT",
    name: "Tether",
    networks: [
      { label: "TRC20", address: "TEhJ6vz7gFH2ayNgPLn9GgsxkET8ymhego" },
      { label: "TON", address: "UQC6qwqUldhlxNH_U4FGZtVHJuyCfhtQs1gYQ7rNrqr99st8" },
      { label: "ERC20", address: "0x505c0d03c1F82E4351aD853e2600A099660C5d9b" },
      { label: "BEP20", address: "0x505c0d03c1F82E4351aD853e2600A099660C5d9b" },
      { label: "Solana", address: "Gg8tdvNA3w8fz1v3emyj9vyzsjjdXGPkmaDQBgLkaqEz" },
    ],
  },
  {
    asset: "GRAM",
    name: "Gram, formerly Toncoin",
    networks: [{ label: "TON", address: "UQC6qwqUldhlxNH_U4FGZtVHJuyCfhtQs1gYQ7rNrqr99st8" }],
  },
  {
    asset: "BTC",
    name: "Bitcoin",
    networks: [{ label: "Bitcoin", address: "bc1qlvqm607rp0c92x496dejwpca42n6kst2neea8j" }],
  },
  {
    asset: "ETH",
    name: "Ether",
    networks: [{ label: "Ethereum", address: "0x505c0d03c1F82E4351aD853e2600A099660C5d9b" }],
  },
];

function WalletCard({ wallet }: { wallet: Wallet }) {
  const [net, setNet] = useState(0);
  const [copied, setCopied] = useState(false);
  const { label, address } = wallet.networks[net] ?? { label: "", address: "" };
  const copy = () => {
    void navigator.clipboard.writeText(address);
    setCopied(true);
    window.setTimeout(() => setCopied(false), 1600);
  };
  return (
    <Card sx={{ display: "flex", flexDirection: "column", gap: 2 }}>
      <Box sx={{ display: "flex", alignItems: "baseline", gap: 1 }}>
        <Typography variant="h6">{wallet.asset}</Typography>
        <Typography variant="body2" color="text.secondary">
          {wallet.name}
        </Typography>
      </Box>
      {wallet.networks.length > 1 && (
        <Stack direction="row" spacing={0.75} useFlexGap sx={{ flexWrap: "wrap" }}>
          {wallet.networks.map((n, i) => (
            <Chip
              key={n.label}
              label={n.label}
              size="small"
              variant={i === net ? "filled" : "outlined"}
              color={i === net ? "primary" : "default"}
              onClick={() => setNet(i)}
            />
          ))}
        </Stack>
      )}
      <Box sx={{ display: "flex", gap: 2, alignItems: "flex-start" }}>
        <Box sx={{ p: 1, bgcolor: "#fff", borderRadius: 1.5, lineHeight: 0, flexShrink: 0 }}>
          <QRCodeSVG value={address} size={88} level="M" />
        </Box>
        <Box sx={{ minWidth: 0, flex: 1 }}>
          <Typography variant="caption" color="text.secondary">
            {label} address
          </Typography>
          <Box sx={{ display: "flex", alignItems: "flex-start", gap: 0.5 }}>
            <Typography
              component="code"
              sx={{
                fontFamily: monoFontFamily,
                fontSize: "0.8125rem",
                lineHeight: 1.5,
                wordBreak: "break-all",
                flex: 1,
              }}
            >
              {address}
            </Typography>
            <Tooltip title={copied ? "Copied" : "Copy"}>
              <IconButton size="small" onClick={copy} aria-label={`Copy ${wallet.asset} address`}>
                {copied ? (
                  <CheckRoundedIcon fontSize="small" color="primary" />
                ) : (
                  <ContentCopyRoundedIcon fontSize="small" />
                )}
              </IconButton>
            </Tooltip>
          </Box>
        </Box>
      </Box>
    </Card>
  );
}

function Tier({ title, text, items }: { title: string; text: string; items: string[] }) {
  return (
    <Card sx={{ height: "100%" }}>
      <Typography variant="h6">{title}</Typography>
      <Typography variant="body2" color="text.secondary" sx={{ mt: 0.75 }}>
        {text}
      </Typography>
      <Stack component="ul" spacing={0.75} sx={{ mt: 2, pl: 2.25, mb: 0 }}>
        {items.map((it) => (
          <Typography key={it} component="li" variant="body2">
            {it}
          </Typography>
        ))}
      </Stack>
    </Card>
  );
}

const STEPS: [string, string][] = [
  [
    "Write",
    `Send a line to ${EMAIL}: who you are, how you use Termoso and what you have in mind — a one-off, a monthly amount or a support agreement.`,
  ],
  [
    "Agree",
    "You get an invoice, a sponsorship link or a short support agreement back, usually within two business days. No forms, no portals.",
  ],
  [
    "Done",
    "Sponsors are listed in the README and release notes unless they prefer to stay anonymous. Support work is scheduled directly with the maintainers.",
  ],
];

/** `/sponsor` — public page linked from the landing and GitHub's Sponsor button. */
export function SponsorPage() {
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
          <Section sx={{ pb: { xs: 4, md: 6 } }}>
            <Box sx={{ maxWidth: 720 }}>
              <Typography
                variant="overline"
                component="div"
                color="primary.main"
                sx={{ mb: 1.25, letterSpacing: "0.1em" }}
              >
                Sponsorship
              </Typography>
              <Typography
                variant="h1"
                component="h1"
                sx={{ fontSize: { xs: "2.25rem", md: "3.25rem" }, letterSpacing: "-0.03em" }}
              >
                Support Termoso
              </Typography>
              <Typography
                variant="body1"
                color="text.secondary"
                sx={{ mt: 2.5, fontSize: "1.125rem", lineHeight: 1.6 }}
              >
                Termoso is free software under AGPL-3.0 with no paid tier and no vendor behind it.
                The clients, the server and the hosted cloud are funded by the people and companies
                who rely on them. If it saves your team time, consider sponsoring; if your
                organisation runs it, we can back you with support and engineering.
              </Typography>
              <Stack direction={{ xs: "column", sm: "row" }} spacing={1.5} sx={{ mt: 4 }}>
                <Button
                  href={`mailto:${EMAIL}?subject=Sponsoring%20Termoso`}
                  variant="contained"
                  size="large"
                >
                  Write to {EMAIL}
                </Button>
                <Button
                  href={REPO}
                  target="_blank"
                  rel="noreferrer"
                  variant="outlined"
                  size="large"
                >
                  Star on GitHub
                </Button>
              </Stack>
            </Box>
          </Section>

          <Section id="tiers" sx={{ pt: 0 }}>
            <SectionTitle
              eyebrow="Ways to help"
              title="Pick what fits"
              text="Everything is open source either way — sponsorship buys maintainer time, not features behind a wall."
            />
            <Box
              sx={{
                display: "grid",
                gridTemplateColumns: { xs: "1fr", md: "repeat(3, 1fr)" },
                gap: 2,
              }}
            >
              <Tier
                title="Individual sponsor"
                text="Any amount, one-off or monthly. You keep the project alive and get a say in what comes next."
                items={[
                  "Name in the README and release notes (or stay anonymous)",
                  "Direct line to the maintainers for questions",
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
          </Section>

          <Section id="how" sx={{ pt: 0 }}>
            <SectionTitle
              eyebrow="How it works"
              title="No checkout form, on purpose"
              text="Sponsorship and support are agreed by e-mail, like any other engineering contract."
            />
            <Box
              sx={{
                display: "grid",
                gridTemplateColumns: { xs: "1fr", md: "repeat(3, 1fr)" },
                gap: { xs: 3, md: 4 },
              }}
            >
              {STEPS.map(([title, text], i) => (
                <Box key={title} sx={{ display: "flex", gap: 2 }}>
                  <Typography
                    sx={{
                      fontFamily: monoFontFamily,
                      color: "primary.main",
                      fontSize: "0.875rem",
                      pt: 0.5,
                    }}
                  >
                    0{i + 1}
                  </Typography>
                  <Box>
                    <Typography variant="h6">{title}</Typography>
                    <Typography variant="body2" color="text.secondary" sx={{ mt: 0.75 }}>
                      {text}
                    </Typography>
                  </Box>
                </Box>
              ))}
            </Box>
            <Typography variant="body2" color="text.secondary" sx={{ mt: 4 }}>
              Security reports go to the address in{" "}
              <Link href={`${REPO}/blob/main/SECURITY.md`} target="_blank" rel="noreferrer">
                SECURITY.md
              </Link>
              , not here.
            </Typography>
          </Section>

          <Section id="crypto" sx={{ pt: 0 }}>
            <SectionTitle
              eyebrow="Crypto"
              title="Or send it directly"
              text="One-off contributions without an invoice. Pick the network you already use."
            />
            <Box
              sx={{
                display: "grid",
                gridTemplateColumns: { xs: "1fr", md: "repeat(2, 1fr)" },
                gap: 2,
              }}
            >
              {WALLETS.map((w) => (
                <WalletCard key={w.asset} wallet={w} />
              ))}
            </Box>
            <Typography variant="caption" color="text.disabled" sx={{ display: "block", mt: 2 }}>
              Double-check the network before sending — transfers on the wrong network cannot be
              recovered. Want a mention in the README? Drop a line to {EMAIL} after the transfer.
            </Typography>
          </Section>
        </Container>
      </Box>
      <LandingFooter />
    </Box>
  );
}
