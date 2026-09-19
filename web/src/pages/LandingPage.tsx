import { useState, type ReactNode } from "react";
import {
  Accordion,
  AccordionDetails,
  AccordionSummary,
  Box,
  Button,
  type ButtonProps,
  Container,
  Link,
  Stack,
  Typography,
  alpha,
} from "@mui/material";
import ExpandMoreRoundedIcon from "@mui/icons-material/ExpandMoreRounded";
import ArrowForwardRoundedIcon from "@mui/icons-material/ArrowForwardRounded";
import ShieldOutlinedIcon from "@mui/icons-material/ShieldOutlined";
import LockOutlinedIcon from "@mui/icons-material/LockOutlined";
import KeyOutlinedIcon from "@mui/icons-material/KeyOutlined";
import VisibilityOffOutlinedIcon from "@mui/icons-material/VisibilityOffOutlined";
import DnsOutlinedIcon from "@mui/icons-material/DnsOutlined";
import CodeOutlinedIcon from "@mui/icons-material/CodeOutlined";
import GroupsOutlinedIcon from "@mui/icons-material/GroupsOutlined";
import FolderOutlinedIcon from "@mui/icons-material/FolderOutlined";
import SwapHorizOutlinedIcon from "@mui/icons-material/SwapHorizOutlined";
import HistoryOutlinedIcon from "@mui/icons-material/HistoryOutlined";
import DevicesOutlinedIcon from "@mui/icons-material/DevicesOutlined";
import { Link as RouterLink } from "react-router";
import { Logo } from "@/components/Logo";
import { ThemeToggle } from "@/layout/ThemeToggle";
import { useAuthState } from "@/auth/store";
import { useServerInfo } from "@/api/hooks";
import { emerald, monoFontFamily } from "@/theme/theme";

const REPO = "https://github.com/rep0rtDev/termoso";
const RELEASES = `${REPO}/releases/latest`;
const README = `${REPO}#readme`;

const HEADER = 60;

// ───────────────────────────── content ─────────────────────────────

const terminalPoints = [
  {
    title: "Connect with one click",
    text: "Hosts remember address, port, credentials, jump host and proxy. Type to search, press Enter to connect.",
  },
  {
    title: "Autocomplete and shell history",
    text: "Shell integration marks every prompt; suggestions come from your own history and snippets, never from a cloud.",
  },
  {
    title: "Tabs, splits and broadcast",
    text: "Up to sixteen panes per tab with a draggable divider, input broadcast to all of them, tabs you can drag to reorder.",
  },
  {
    title: "Themes and fonts",
    text: "Sixty-four terminal themes, per-host colors, bundled Nerd Fonts, distro icons detected on first login.",
  },
];

const teamPoints: { icon: ReactNode; title: string; text: string }[] = [
  {
    icon: <FolderOutlinedIcon />,
    title: "Shared vaults",
    text: "Each vault has its own key, sealed to every member's public key. Rotate on removal.",
  },
  {
    icon: <GroupsOutlinedIcon />,
    title: "Roles and invites",
    text: "Owner, manager, member. Invite by email; revoke access from the cabinet at any time.",
  },
  {
    icon: <DevicesOutlinedIcon />,
    title: "Device approval",
    text: "A new device signs in only after a confirmation from an already trusted one.",
  },
];

const tools: { icon: ReactNode; title: string; text: string }[] = [
  {
    icon: <KeyOutlinedIcon />,
    title: "Keychain",
    text: "Generate Ed25519, ECDSA and RSA keys, import existing ones, export a public key to a host in one action. Works with your system ssh-agent.",
  },
  {
    icon: <SwapHorizOutlinedIcon />,
    title: "SFTP and port forwarding",
    text: "Two-pane SFTP with queued transfers and permissions editing. Local, remote and dynamic tunnels with live status.",
  },
  {
    icon: <CodeOutlinedIcon />,
    title: "Snippets and logs",
    text: "Snippet packages with variables, run straight into a live session. Session logs with bookmarks, stored where you decide.",
  },
];

const security: { icon: ReactNode; title: string; text: string }[] = [
  {
    icon: <LockOutlinedIcon />,
    title: "End-to-end encrypted",
    text: "Hosts, keys and passwords are encrypted on your device with XChaCha20-Poly1305. The server only ever stores ciphertext.",
  },
  {
    icon: <ShieldOutlinedIcon />,
    title: "OPAQUE sign-in",
    text: "Your password is never sent anywhere — not even hashed. TOTP, security keys and a 24-word recovery key on top.",
  },
  {
    icon: <VisibilityOffOutlinedIcon />,
    title: "Zero telemetry",
    text: "No analytics, crash reporting or hidden network calls. Metrics are opt-in and visible only to whoever runs the server.",
  },
  {
    icon: <DnsOutlinedIcon />,
    title: "Your server, your rules",
    text: "One Docker Compose file: PostgreSQL, Redis, S3-compatible storage. Or use the free cloud. Or stay entirely offline.",
  },
  {
    icon: <HistoryOutlinedIcon />,
    title: "Auditable by design",
    text: "AGPL-3.0. Every request the client makes is in the repository; the protocol is documented and testable.",
  },
  {
    icon: <KeyOutlinedIcon />,
    title: "Post-quantum ready",
    text: "ML-KEM hybrid key exchange for SSH where the server supports it, with a badge in the tab so you can see it.",
  },
];

const steps = [
  {
    n: "01",
    title: "Deploy the server",
    text: "Clone the repository, set a few environment variables and start the stack.",
    code: "docker compose up -d",
  },
  {
    n: "02",
    title: "Create an account",
    text: "Sign up in this cabinet. Your password never leaves the browser; keep the recovery key somewhere safe.",
    code: null,
  },
  {
    n: "03",
    title: "Connect the app",
    text: "Install the desktop app, point it at your server URL and sign in. Hosts sync encrypted.",
    code: null,
  },
];

const faq = [
  {
    q: "Is Termoso free?",
    a: "Yes. The client and the server are open source under AGPL-3.0. There are no tiers, seats or paid features; the same build serves one person and a whole team.",
  },
  {
    q: "Do I need an account?",
    a: "No. The desktop app works fully offline with a local encrypted vault. An account only adds sync between devices and sharing with a team.",
  },
  {
    q: "Where is my data stored?",
    a: "On your device, encrypted. If you sign in, an encrypted copy is kept on the server you chose — your own or the hosted one — and the server cannot decrypt it.",
  },
  {
    q: "Can the server operator read my passwords?",
    a: "No. Keys are derived from your password on the device and never leave it. What the server receives is ciphertext, plus the metadata needed to sync it.",
  },
  {
    q: "Does the app phone home?",
    a: "Never on its own. The only network calls are to the SSH hosts you connect to, the server you signed in to, and an update check you start yourself.",
  },
  {
    q: "What if I lose my password?",
    a: "Use the 24-word recovery key created at sign-up. Nobody else can reset it for you — that is the point.",
  },
];

// ───────────────────────────── pieces ─────────────────────────────

function SectionTitle({
  eyebrow,
  title,
  text,
  center,
}: {
  eyebrow?: string;
  title: string;
  text?: string;
  center?: boolean;
}) {
  return (
    <Box
      sx={{
        maxWidth: 620,
        mb: { xs: 4, md: 6 },
        mx: center ? "auto" : 0,
        textAlign: center ? "center" : "left",
      }}
    >
      {eyebrow && (
        <Typography
          variant="overline"
          component="div"
          color="primary.main"
          sx={{ mb: 1.25, letterSpacing: "0.1em" }}
        >
          {eyebrow}
        </Typography>
      )}
      <Typography
        variant="h2"
        component="h2"
        sx={{ fontSize: { xs: "1.75rem", md: "2.25rem" }, letterSpacing: "-0.02em" }}
      >
        {title}
      </Typography>
      {text && (
        <Typography
          variant="body1"
          color="text.secondary"
          sx={{ mt: 1.75, fontSize: "1.0625rem", lineHeight: 1.6 }}
        >
          {text}
        </Typography>
      )}
    </Box>
  );
}

function Section({
  id,
  children,
  sx,
}: {
  id?: string;
  children: ReactNode;
  sx?: Record<string, unknown>;
}) {
  return (
    <Box id={id} component="section" sx={{ py: { xs: 8, md: 12 }, scrollMarginTop: HEADER, ...sx }}>
      {children}
    </Box>
  );
}

function Card({ children, sx }: { children: ReactNode; sx?: Record<string, unknown> }) {
  return (
    <Box
      sx={{
        bgcolor: "surface.base",
        borderRadius: 3,
        p: { xs: 3, md: 3.5 },
        border: 1,
        borderColor: "border.light",
        ...sx,
      }}
    >
      {children}
    </Box>
  );
}

function IconBox({ children }: { children: ReactNode }) {
  return (
    <Box
      sx={{
        width: 40,
        height: 40,
        borderRadius: 2,
        flexShrink: 0,
        display: "grid",
        placeItems: "center",
        bgcolor: alpha(emerald.main, 0.12),
        color: "primary.main",
        "& svg": { fontSize: 22 },
      }}
    >
      {children}
    </Box>
  );
}

function PlatformChip({ label, soon }: { label: string; soon?: boolean }) {
  return (
    <Box
      sx={{
        px: 1.5,
        height: 30,
        display: "inline-flex",
        alignItems: "center",
        gap: 0.75,
        borderRadius: 999,
        bgcolor: "surface.high",
        color: soon ? "text.disabled" : "text.primary",
        fontSize: 13,
        fontWeight: 500,
        border: 1,
        borderColor: "border.light",
      }}
    >
      {label}
      {soon && (
        <Box component="span" sx={{ fontSize: 11, color: "text.disabled" }}>
          soon
        </Box>
      )}
    </Box>
  );
}

/** Frame shared by every mock window. */
function Window({ children, sx }: { children: ReactNode; sx?: Record<string, unknown> }) {
  return (
    <Box
      aria-hidden
      sx={{
        borderRadius: 3,
        bgcolor: "surface.base",
        overflow: "hidden",
        border: 1,
        borderColor: "border.basic",
        boxShadow: "0 30px 80px -20px rgba(0,0,0,0.55)",
        fontSize: 12,
        userSelect: "none",
        ...sx,
      }}
    >
      {children}
    </Box>
  );
}

function TitleBar({ tabs, active = 0 }: { tabs: string[]; active?: number }) {
  return (
    <Box
      sx={{
        height: 38,
        display: "flex",
        alignItems: "center",
        gap: 0.5,
        px: 1.5,
        bgcolor: "surface.lowest",
      }}
    >
      {tabs.map((t, i) => (
        <Box
          key={t}
          sx={{
            px: 1.25,
            py: 0.5,
            borderRadius: 1,
            bgcolor: i === active ? "surface.high" : "transparent",
            color: i === active ? "text.primary" : "text.secondary",
            display: "flex",
            alignItems: "center",
            gap: 0.75,
          }}
        >
          {i === active && i > 1 && (
            <Box sx={{ width: 6, height: 6, borderRadius: "50%", bgcolor: "primary.main" }} />
          )}
          {t}
        </Box>
      ))}
      <Box sx={{ ml: "auto", display: "flex", alignItems: "center", gap: 0.5 }}>
        <Box
          sx={{
            width: 20,
            height: 20,
            borderRadius: 1.25,
            bgcolor: "primary.dark",
            color: "primary.contrastText",
            display: "grid",
            placeItems: "center",
            fontSize: 10,
            fontWeight: 700,
            boxShadow: `0 0 0 2px var(--mui-palette-surface-lowest), 0 0 0 3px ${emerald.main}`,
          }}
        >
          A
        </Box>
        <Box
          sx={{
            ml: 0.5,
            width: 22,
            height: 20,
            borderRadius: "0 5px 5px 0",
            bgcolor: "surface.highest",
            color: "text.secondary",
            display: "grid",
            placeItems: "center",
            fontSize: 14,
          }}
        >
          +
        </Box>
      </Box>
    </Box>
  );
}

const MOCK_HOSTS: { name: string; sub: string; color: string; glyph: string }[] = [
  { name: "prod-api-01", sub: "deploy · 10.0.1.12", color: "#E95420", glyph: "U" },
  { name: "db-primary", sub: "postgres · 10.0.2.5", color: "#336791", glyph: "P" },
  { name: "edge-eu-west", sub: "root · edge.example.io", color: "#0D597F", glyph: "A" },
  { name: "build-runner", sub: "ci · 10.0.3.40", color: "#A81D33", glyph: "D" },
  { name: "bastion", sub: "ops · bastion.example.io", color: "#EE0000", glyph: "R" },
  { name: "nas", sub: "admin · 192.168.1.20", color: "#FCC624", glyph: "L" },
];

/** Hosts screen — the first thing the app shows. */
function HostsMock() {
  const nav = ["Hosts", "Keychain", "Port forwarding", "Snippets", "Known hosts", "Logs"];
  return (
    <Window>
      <TitleBar tabs={["Vaults", "SFTP"]} />
      <Box sx={{ display: "flex", minHeight: { xs: 300, md: 380 } }}>
        <Box
          sx={{
            width: 148,
            flexShrink: 0,
            p: 1,
            display: { xs: "none", sm: "flex" },
            flexDirection: "column",
            gap: 0.25,
            bgcolor: "surface.lowest",
          }}
        >
          {nav.map((l, i) => (
            <Box
              key={l}
              sx={{
                px: 1.25,
                py: 0.75,
                borderRadius: 1,
                bgcolor: i === 0 ? "surface.high" : "transparent",
                color: i === 0 ? "text.primary" : "text.secondary",
              }}
            >
              {l}
            </Box>
          ))}
        </Box>
        <Box sx={{ flex: 1, p: 2, minWidth: 0 }}>
          <Box
            sx={{
              height: 32,
              borderRadius: 1.5,
              bgcolor: "surface.high",
              display: "flex",
              alignItems: "center",
              px: 1.5,
              color: "text.disabled",
              mb: 2,
            }}
          >
            Search or type user@host to connect
          </Box>
          <Typography
            variant="caption"
            color="text.secondary"
            sx={{ display: "block", mb: 1, fontWeight: 600 }}
          >
            Groups
          </Typography>
          <Box
            sx={{
              display: "grid",
              gridTemplateColumns: { xs: "1fr 1fr", md: "repeat(3, 1fr)" },
              gap: 1,
              mb: 2,
            }}
          >
            {[
              ["Production", "6 hosts"],
              ["Staging", "3 hosts"],
              ["Home lab", "4 hosts"],
            ].map(([n, c]) => (
              <Box
                key={n}
                sx={{
                  display: "flex",
                  alignItems: "center",
                  gap: 1,
                  p: 1,
                  borderRadius: 1.5,
                  bgcolor: "surface.high",
                }}
              >
                <Box
                  sx={{
                    width: 28,
                    height: 28,
                    borderRadius: 1,
                    bgcolor: "surface.highest",
                    color: "text.secondary",
                    display: "grid",
                    placeItems: "center",
                  }}
                >
                  <FolderOutlinedIcon sx={{ fontSize: 16 }} />
                </Box>
                <Box sx={{ minWidth: 0 }}>
                  <Box sx={{ fontWeight: 600 }}>{n}</Box>
                  <Box sx={{ color: "text.disabled", fontSize: 11 }}>{c}</Box>
                </Box>
              </Box>
            ))}
          </Box>
          <Typography
            variant="caption"
            color="text.secondary"
            sx={{ display: "block", mb: 1, fontWeight: 600 }}
          >
            Hosts
          </Typography>
          <Box
            sx={{
              display: "grid",
              gridTemplateColumns: { xs: "1fr 1fr", md: "repeat(3, 1fr)" },
              gap: 1,
            }}
          >
            {MOCK_HOSTS.map((h) => (
              <Box
                key={h.name}
                sx={{
                  display: "flex",
                  alignItems: "center",
                  gap: 1,
                  p: 1,
                  borderRadius: 1.5,
                  bgcolor: "surface.high",
                }}
              >
                <Box
                  sx={{
                    width: 28,
                    height: 28,
                    borderRadius: 1,
                    bgcolor: h.color,
                    color: "#fff",
                    display: "grid",
                    placeItems: "center",
                    fontWeight: 700,
                    fontSize: 12,
                  }}
                >
                  {h.glyph}
                </Box>
                <Box sx={{ minWidth: 0 }}>
                  <Box sx={{ fontWeight: 600, whiteSpace: "nowrap" }}>{h.name}</Box>
                  <Box
                    sx={{
                      color: "text.disabled",
                      fontSize: 11,
                      whiteSpace: "nowrap",
                      overflow: "hidden",
                      textOverflow: "ellipsis",
                    }}
                  >
                    {h.sub}
                  </Box>
                </Box>
              </Box>
            ))}
          </Box>
        </Box>
      </Box>
    </Window>
  );
}

function Prompt({ cmd }: { cmd: ReactNode }) {
  return (
    <Box component="span" sx={{ display: "block" }}>
      <Box component="span" sx={{ color: "#5FD0A4" }}>
        deploy@prod-api-01
      </Box>
      <Box component="span" sx={{ color: "#5AA9E6" }}>
        {" "}
        ~/app
      </Box>{" "}
      $ {cmd}
    </Box>
  );
}

/** Terminal with the autocomplete popup open. */
function TerminalMock() {
  const dim = (s: string) => (
    <Box component="span" sx={{ display: "block", color: "#8D91A5" }}>
      {s}
    </Box>
  );
  return (
    <Window>
      <TitleBar tabs={["Vaults", "SFTP", "prod-api-01", "db-primary"]} active={2} />
      <Box
        sx={{
          position: "relative",
          minHeight: 340,
          bgcolor: "#0F1320",
          color: "#E4E7F0",
          p: 2,
          fontFamily: monoFontFamily,
          fontSize: 12.5,
          lineHeight: 1.7,
        }}
      >
        <Prompt cmd="git pull --ff-only" />
        {dim("Updating 3f1c9e2..a81d0f7")}
        {dim("Fast-forward")}
        {dim(" src/server.rs | 14 ++++++++------")}
        <Prompt cmd="cargo build --release" />
        {dim("   Compiling termoso-server v0.1.0")}
        {dim("    Finished `release` profile in 41.2s")}
        <Prompt
          cmd={
            <>
              sys
              <Box
                component="span"
                sx={{
                  display: "inline-block",
                  width: 7,
                  height: 14,
                  bgcolor: "#E4E7F0",
                  verticalAlign: "-2px",
                }}
              />
            </>
          }
        />
        <Box
          sx={{
            position: "absolute",
            left: 196,
            top: 196,
            width: 340,
            whiteSpace: "nowrap",
            borderRadius: 1.5,
            bgcolor: "#1C2032",
            border: "1px solid rgba(255,255,255,0.08)",
            boxShadow: "0 12px 30px rgba(0,0,0,0.4)",
            overflow: "hidden",
            fontFamily: "inherit",
          }}
        >
          {(
            [
              ["systemctl restart api", "history · 2 min ago"],
              ["systemctl status api", "history"],
              ["sysctl -a | grep vm.", "snippet · Tuning"],
            ] as [string, string][]
          ).map(([c, s], i) => (
            <Box
              key={c}
              sx={{
                px: 1.5,
                py: 0.75,
                display: "flex",
                justifyContent: "space-between",
                gap: 2,
                bgcolor: i === 0 ? alpha(emerald.main, 0.16) : "transparent",
              }}
            >
              <Box component="span">
                <Box component="span" sx={{ color: "#5FD0A4" }}>
                  sys
                </Box>
                {c.slice(3)}
              </Box>
              <Box component="span" sx={{ color: "#8D91A5", fontSize: 11, whiteSpace: "nowrap" }}>
                {s}
              </Box>
            </Box>
          ))}
        </Box>
      </Box>
    </Window>
  );
}

/** Team vault with members, as seen in the cabinet. */
function TeamMock() {
  const members: [string, string, string][] = [
    ["Alex Ivanov", "Owner", "#2BB884"],
    ["Mia Chen", "Manager", "#5AA9E6"],
    ["Omar Haddad", "Member", "#F8AA4B"],
    ["Sara Kim", "Member", "#C86DD7"],
  ];
  return (
    <Window>
      <TitleBar tabs={["Vaults", "SFTP"]} />
      <Box sx={{ p: 2.5, minHeight: 300 }}>
        <Box sx={{ display: "flex", alignItems: "center", gap: 1.5, mb: 2 }}>
          <Box
            sx={{
              width: 36,
              height: 36,
              borderRadius: 1.5,
              bgcolor: alpha(emerald.main, 0.16),
              color: "primary.main",
              display: "grid",
              placeItems: "center",
            }}
          >
            <FolderOutlinedIcon sx={{ fontSize: 20 }} />
          </Box>
          <Box>
            <Box sx={{ fontWeight: 600, fontSize: 14 }}>Production</Box>
            <Box sx={{ color: "text.disabled" }}>Team vault · 6 hosts · 3 keys · key v4</Box>
          </Box>
          <Box
            sx={{
              ml: "auto",
              px: 1.25,
              py: 0.5,
              borderRadius: 1,
              bgcolor: "primary.main",
              color: "primary.contrastText",
              fontWeight: 600,
            }}
          >
            Invite
          </Box>
        </Box>
        <Box sx={{ borderRadius: 2, bgcolor: "surface.high", overflow: "hidden" }}>
          {members.map(([n, r, c], i) => (
            <Box
              key={n}
              sx={{
                display: "flex",
                alignItems: "center",
                gap: 1.25,
                px: 1.5,
                py: 1.125,
                borderTop: i ? 1 : 0,
                borderColor: "border.light",
              }}
            >
              <Box
                sx={{
                  width: 26,
                  height: 26,
                  borderRadius: 1,
                  bgcolor: c,
                  color: "#fff",
                  display: "grid",
                  placeItems: "center",
                  fontWeight: 700,
                  fontSize: 11,
                }}
              >
                {n[0]}
              </Box>
              <Box sx={{ flex: 1, fontWeight: 500 }}>{n}</Box>
              <Box
                sx={{
                  px: 1,
                  py: 0.25,
                  borderRadius: 999,
                  bgcolor: "surface.highest",
                  color: "text.secondary",
                  fontSize: 11,
                }}
              >
                {r}
              </Box>
              <Box sx={{ color: "text.disabled", fontSize: 11 }}>
                {i === 3 ? "key pending" : "sealed"}
              </Box>
            </Box>
          ))}
        </Box>
      </Box>
    </Window>
  );
}

function Stat({ value, label }: { value: string; label: string }) {
  return (
    <Box sx={{ textAlign: "center", px: 2, py: 2.5 }}>
      <Typography
        variant="h3"
        component="div"
        sx={{ fontSize: { xs: "1.5rem", md: "1.75rem" }, letterSpacing: "-0.02em" }}
      >
        {value}
      </Typography>
      <Typography variant="body2" color="text.secondary" sx={{ mt: 0.5 }}>
        {label}
      </Typography>
    </Box>
  );
}

// ───────────────────────────── page ─────────────────────────────

/**
 * Button into the cabinet. Same-origin deployments route client-side; when the
 * landing has its own origin (`landing_only`) the cabinet lives at `web_url`,
 * so the button becomes a plain cross-origin link.
 */
function CabinetButton({
  to,
  ...props
}: Pick<ButtonProps, "variant" | "size" | "sx" | "endIcon" | "children"> & { to: string }) {
  const info = useServerInfo().data;
  if (info?.landing_only && info.web_url) {
    return <Button {...props} href={`${info.web_url.replace(/\/+$/, "")}${to}`} />;
  }
  return <Button {...props} component={RouterLink} to={to} />;
}

export function LandingPage() {
  const { session } = useAuthState();
  const info = useServerInfo();
  const registrationOpen = info.data?.registration_open !== false;
  const [open, setOpen] = useState<number | null>(0);

  const primaryCta = session ? (
    <CabinetButton
      to="/account"
      variant="contained"
      size="large"
      endIcon={<ArrowForwardRoundedIcon />}
    >
      Open cabinet
    </CabinetButton>
  ) : registrationOpen ? (
    <CabinetButton
      to="/signup"
      variant="contained"
      size="large"
      endIcon={<ArrowForwardRoundedIcon />}
    >
      Create a free account
    </CabinetButton>
  ) : (
    <CabinetButton
      to="/login"
      variant="contained"
      size="large"
      endIcon={<ArrowForwardRoundedIcon />}
    >
      Sign in
    </CabinetButton>
  );

  return (
    <Box sx={{ minHeight: "100vh", bgcolor: "surface.lowest", overflowX: "hidden" }}>
      <Box
        component="header"
        sx={{
          position: "sticky",
          top: 0,
          zIndex: 10,
          bgcolor: "color-mix(in srgb, var(--mui-palette-surface-lowest) 82%, transparent)",
          backdropFilter: "blur(12px)",
          borderBottom: 1,
          borderColor: "border.light",
        }}
      >
        <Container
          maxWidth="lg"
          sx={{ height: HEADER, display: "flex", alignItems: "center", gap: 1 }}
        >
          <Logo size={26} />
          <Stack direction="row" spacing={0.5} sx={{ ml: 5, display: { xs: "none", md: "flex" } }}>
            {[
              ["Product", "#product"],
              ["Security", "#security"],
              ["Self-hosting", "#self-hosting"],
              ["Download", "#download"],
              ["FAQ", "#faq"],
            ].map(([label, href]) => (
              <Button key={href} href={href} variant="text" sx={{ color: "text.secondary" }}>
                {label}
              </Button>
            ))}
            <Button
              href={REPO}
              target="_blank"
              rel="noreferrer"
              variant="text"
              sx={{ color: "text.secondary" }}
            >
              GitHub
            </Button>
          </Stack>
          <Box sx={{ flex: 1 }} />
          <ThemeToggle />
          {session ? (
            <CabinetButton to="/account" variant="contained" sx={{ ml: 1 }}>
              Open cabinet
            </CabinetButton>
          ) : (
            <>
              <CabinetButton to="/login" variant="text" sx={{ ml: 1 }}>
                Log in
              </CabinetButton>
              {registrationOpen && (
                <CabinetButton to="/signup" variant="contained">
                  Sign up
                </CabinetButton>
              )}
            </>
          )}
        </Container>
      </Box>

      <Box component="main">
        {/* Hero */}
        <Box sx={{ position: "relative" }}>
          <Box
            aria-hidden
            sx={{
              position: "absolute",
              inset: 0,
              pointerEvents: "none",
              background: `radial-gradient(60% 50% at 50% 0%, ${alpha(emerald.main, 0.22)} 0%, transparent 70%)`,
            }}
          />
          <Container
            maxWidth="lg"
            sx={{
              position: "relative",
              pt: { xs: 8, md: 12 },
              pb: { xs: 6, md: 8 },
              textAlign: "center",
            }}
          >
            <Typography
              variant="overline"
              component="div"
              color="primary.main"
              sx={{ mb: 2.5, letterSpacing: "0.1em" }}
            >
              Open source · Self-hosted · No telemetry
            </Typography>
            <Typography
              variant="h1"
              component="h1"
              sx={{
                fontSize: { xs: "2.5rem", sm: "3.25rem", md: "4rem" },
                lineHeight: 1.05,
                letterSpacing: "-0.03em",
                fontWeight: 600,
                maxWidth: 900,
                mx: "auto",
              }}
            >
              The SSH client that reports to you, not on you.
            </Typography>
            <Typography
              variant="body1"
              color="text.secondary"
              sx={{
                mt: 3,
                fontSize: { xs: "1.0625rem", md: "1.25rem" },
                maxWidth: 640,
                mx: "auto",
                lineHeight: 1.55,
              }}
            >
              Terminal, SFTP, keys and snippets in end-to-end encrypted vaults. Free cloud, your own
              server, or no server at all.
            </Typography>
            <Stack
              direction="row"
              spacing={1.5}
              useFlexGap
              sx={{ mt: 4.5, justifyContent: "center", flexWrap: "wrap" }}
            >
              {primaryCta}
              <Button href="#download" variant="outlined" size="large">
                Download the app
              </Button>
            </Stack>
            <Stack
              direction="row"
              spacing={1}
              useFlexGap
              sx={{ mt: 4, justifyContent: "center", flexWrap: "wrap" }}
            >
              <PlatformChip label="Linux" />
              <PlatformChip label="Windows" />
              <PlatformChip label="Android" soon />
              <PlatformChip label="macOS" soon />
            </Stack>

            <Box sx={{ mt: { xs: 6, md: 8 }, mx: "auto", maxWidth: 1040, textAlign: "left" }}>
              <HostsMock />
            </Box>
          </Container>
        </Box>

        <Container maxWidth="lg">
          {/* Facts strip */}
          <Box
            sx={{
              mt: { xs: 2, md: 4 },
              display: "grid",
              gridTemplateColumns: { xs: "1fr 1fr", md: "repeat(4, 1fr)" },
              bgcolor: "surface.base",
              borderRadius: 3,
              border: 1,
              borderColor: "border.light",
              "& > * + *": { borderLeft: { md: 1 }, borderColor: { md: "border.light" } },
            }}
          >
            <Stat value="AGPL-3.0" label="Client and server, one license" />
            <Stat value="0" label="Trackers, analytics, crash reports" />
            <Stat value="E2E" label="XChaCha20-Poly1305 on device" />
            <Stat value="1 file" label="docker compose to self-host" />
          </Box>

          {/* Product: terminal */}
          <Section id="product">
            <Box
              sx={{
                display: "grid",
                gridTemplateColumns: { xs: "1fr", md: "4fr 7fr" },
                gap: { xs: 5, md: 8 },
                alignItems: "center",
              }}
            >
              <Box>
                <SectionTitle
                  eyebrow="Terminal"
                  title="A terminal that keeps you productive"
                  text="Everything you expect from a modern SSH client — native, fast, and without a subscription in the way."
                />
                <Stack spacing={0}>
                  {terminalPoints.map((p, i) => (
                    <Box
                      key={p.title}
                      sx={{
                        py: 2,
                        pl: 2.5,
                        borderLeft: 2,
                        borderColor: i === 0 ? "primary.main" : "border.light",
                      }}
                    >
                      <Typography
                        variant="subtitle1"
                        sx={{ color: i === 0 ? "text.primary" : "text.secondary" }}
                      >
                        {p.title}
                      </Typography>
                      <Typography variant="body2" color="text.secondary" sx={{ mt: 0.5 }}>
                        {p.text}
                      </Typography>
                    </Box>
                  ))}
                </Stack>
              </Box>
              <TerminalMock />
            </Box>
          </Section>

          {/* Product: teams */}
          <Section sx={{ pt: 0 }}>
            <Box
              sx={{
                display: "grid",
                gridTemplateColumns: { xs: "1fr", md: "7fr 5fr" },
                gap: { xs: 5, md: 8 },
                alignItems: "center",
              }}
            >
              <Box sx={{ order: { xs: 2, md: 1 } }}>
                <TeamMock />
              </Box>
              <Box sx={{ order: { xs: 1, md: 2 } }}>
                <SectionTitle
                  eyebrow="Teams"
                  title="Built for teams that own their infrastructure"
                  text="A shared vault is the single source of truth for the team. Access is cryptographic, not a checkbox on a server."
                />
                <Stack spacing={2.5}>
                  {teamPoints.map((p) => (
                    <Box key={p.title} sx={{ display: "flex", gap: 2 }}>
                      <IconBox>{p.icon}</IconBox>
                      <Box>
                        <Typography variant="subtitle1">{p.title}</Typography>
                        <Typography variant="body2" color="text.secondary" sx={{ mt: 0.5 }}>
                          {p.text}
                        </Typography>
                      </Box>
                    </Box>
                  ))}
                </Stack>
              </Box>
            </Box>
          </Section>

          {/* Product: tools */}
          <Section sx={{ pt: 0 }}>
            <SectionTitle
              eyebrow="Toolbox"
              title="Keys, files, tunnels, snippets"
              text="The parts of the job around the shell, handled in the same window."
            />
            <Box
              sx={{
                display: "grid",
                gridTemplateColumns: { xs: "1fr", md: "repeat(3, 1fr)" },
                gap: 2,
              }}
            >
              {tools.map((f) => (
                <Card key={f.title}>
                  <IconBox>{f.icon}</IconBox>
                  <Typography variant="subtitle1" sx={{ mt: 2 }}>
                    {f.title}
                  </Typography>
                  <Typography variant="body2" color="text.secondary" sx={{ mt: 0.75 }}>
                    {f.text}
                  </Typography>
                </Card>
              ))}
            </Box>
          </Section>
        </Container>

        {/* Security */}
        <Box sx={{ position: "relative", bgcolor: "surface.base" }}>
          <Box
            aria-hidden
            sx={{
              position: "absolute",
              inset: 0,
              pointerEvents: "none",
              background: `radial-gradient(50% 40% at 50% 0%, ${alpha(emerald.main, 0.14)} 0%, transparent 70%)`,
            }}
          />
          <Container maxWidth="lg" sx={{ position: "relative" }}>
            <Section id="security">
              <Box sx={{ display: "grid", placeItems: "center", mb: 3 }}>
                <Box
                  sx={{
                    width: 64,
                    height: 64,
                    borderRadius: 3,
                    display: "grid",
                    placeItems: "center",
                    bgcolor: alpha(emerald.main, 0.14),
                    color: "primary.main",
                    boxShadow: `0 0 0 8px ${alpha(emerald.main, 0.06)}`,
                  }}
                >
                  <ShieldOutlinedIcon sx={{ fontSize: 32 }} />
                </Box>
              </Box>
              <SectionTitle
                center
                title="Security you can read, not just trust"
                text="Termoso is built so that the operator of the server — even when that is you — cannot read stored hosts, keys or passwords."
              />
              <Box
                sx={{
                  display: "grid",
                  gridTemplateColumns: { xs: "1fr", sm: "1fr 1fr", md: "repeat(3, 1fr)" },
                  gap: 2,
                }}
              >
                {security.map((f) => (
                  <Card key={f.title} sx={{ bgcolor: "surface.lowest" }}>
                    <Box sx={{ display: "flex", alignItems: "center", gap: 1.5 }}>
                      <IconBox>{f.icon}</IconBox>
                      <Typography variant="subtitle1">{f.title}</Typography>
                    </Box>
                    <Typography variant="body2" color="text.secondary" sx={{ mt: 1.5 }}>
                      {f.text}
                    </Typography>
                  </Card>
                ))}
              </Box>
            </Section>
          </Container>
        </Box>

        <Container maxWidth="lg">
          {/* Self-hosting */}
          <Section id="self-hosting">
            <SectionTitle
              eyebrow="Self-hosting"
              title="Up and running in three steps"
              text="Everything ships as containers. There is nothing to license and no account with a vendor. Prefer not to run anything? Use the free cloud, or stay offline."
            />
            <Box
              sx={{
                display: "grid",
                gridTemplateColumns: { xs: "1fr", md: "repeat(3, 1fr)" },
                gap: 2,
              }}
            >
              {steps.map((s) => (
                <Card key={s.n}>
                  <Typography
                    variant="caption"
                    sx={{ fontFamily: monoFontFamily, color: "primary.main", fontWeight: 600 }}
                  >
                    {s.n}
                  </Typography>
                  <Typography variant="subtitle1" sx={{ mt: 1 }}>
                    {s.title}
                  </Typography>
                  <Typography variant="body2" color="text.secondary" sx={{ mt: 0.75 }}>
                    {s.text}
                  </Typography>
                  {s.code && (
                    <Box
                      sx={{
                        mt: 2,
                        px: 1.5,
                        py: 1,
                        borderRadius: 1.5,
                        bgcolor: "surface.lowest",
                        fontFamily: monoFontFamily,
                        fontSize: "0.8125rem",
                      }}
                    >
                      <Box component="span" sx={{ color: "text.disabled" }}>
                        ${" "}
                      </Box>
                      {s.code}
                    </Box>
                  )}
                </Card>
              ))}
            </Box>
            <Typography variant="body2" color="text.secondary" sx={{ mt: 3 }}>
              Full instructions and the configuration reference are in the{" "}
              <Link href={README} target="_blank" rel="noreferrer">
                README
              </Link>
              .
            </Typography>
          </Section>

          {/* Download / CTA */}
          <Section id="download" sx={{ pt: 0 }}>
            <Box
              sx={{
                position: "relative",
                overflow: "hidden",
                borderRadius: 4,
                bgcolor: "surface.base",
                border: 1,
                borderColor: "border.light",
                px: { xs: 3, md: 6 },
                py: { xs: 6, md: 9 },
                textAlign: "center",
              }}
            >
              <Box
                aria-hidden
                sx={{
                  position: "absolute",
                  inset: 0,
                  pointerEvents: "none",
                  background: `radial-gradient(50% 70% at 50% 100%, ${alpha(emerald.main, 0.22)} 0%, transparent 70%)`,
                }}
              />
              <Box sx={{ position: "relative" }}>
                <Typography
                  variant="h2"
                  component="h2"
                  sx={{ fontSize: { xs: "1.75rem", md: "2.5rem" }, letterSpacing: "-0.02em" }}
                >
                  Get the desktop app
                </Typography>
                <Typography
                  variant="body1"
                  color="text.secondary"
                  sx={{ mt: 1.5, maxWidth: 560, mx: "auto", fontSize: "1.0625rem" }}
                >
                  Signed builds for Linux (deb, rpm, AppImage) and Windows (msi, exe). Updates are
                  checked only when you ask, against your own server or GitHub.
                </Typography>
                <Stack
                  direction={{ xs: "column", sm: "row" }}
                  spacing={1.5}
                  sx={{ mt: 4, justifyContent: "center" }}
                >
                  <Button
                    href={RELEASES}
                    target="_blank"
                    rel="noreferrer"
                    variant="contained"
                    size="large"
                  >
                    Download for Linux
                  </Button>
                  <Button
                    href={RELEASES}
                    target="_blank"
                    rel="noreferrer"
                    variant="outlined"
                    size="large"
                  >
                    Download for Windows
                  </Button>
                </Stack>
                <Typography
                  variant="caption"
                  color="text.disabled"
                  sx={{ display: "block", mt: 2 }}
                >
                  Android and macOS builds are in development.
                </Typography>
              </Box>
            </Box>
          </Section>

          {/* FAQ */}
          <Section id="faq" sx={{ pt: 0 }}>
            <SectionTitle center title="Frequently asked questions" />
            <Box sx={{ maxWidth: 760, mx: "auto" }}>
              {faq.map((f, i) => (
                <Accordion
                  key={f.q}
                  disableGutters
                  elevation={0}
                  expanded={open === i}
                  onChange={(_, v) => setOpen(v ? i : null)}
                  sx={{
                    bgcolor: "transparent",
                    borderBottom: 1,
                    borderColor: "border.light",
                    "&::before": { display: "none" },
                  }}
                >
                  <AccordionSummary
                    expandIcon={<ExpandMoreRoundedIcon />}
                    sx={{ px: 0, minHeight: 56, "& .MuiAccordionSummary-content": { my: 1.5 } }}
                  >
                    <Typography variant="subtitle1">{f.q}</Typography>
                  </AccordionSummary>
                  <AccordionDetails sx={{ px: 0, pt: 0, pb: 2.5 }}>
                    <Typography variant="body1" color="text.secondary">
                      {f.a}
                    </Typography>
                  </AccordionDetails>
                </Accordion>
              ))}
            </Box>
          </Section>
        </Container>
      </Box>

      <Box component="footer" sx={{ borderTop: 1, borderColor: "border.light", py: 6 }}>
        <Container maxWidth="lg">
          <Box
            sx={{
              display: "grid",
              gridTemplateColumns: { xs: "1fr 1fr", md: "2fr 1fr 1fr 1fr" },
              gap: 4,
            }}
          >
            <Box sx={{ gridColumn: { xs: "1 / -1", md: "auto" } }}>
              <Logo size={24} />
              <Typography variant="body2" color="text.secondary" sx={{ mt: 1.5, maxWidth: 300 }}>
                Software should report to you, not on you. Free, open source, self-hosted.
              </Typography>
            </Box>
            {(
              [
                {
                  title: "Product",
                  links: [
                    ["Download", RELEASES],
                    ["Releases", `${REPO}/releases`],
                    ["Security", "#security"],
                  ],
                },
                {
                  title: "Self-hosting",
                  links: [
                    ["Quick start", README],
                    ["Docker Compose", `${REPO}/blob/main/deploy/docker-compose.yml`],
                    ["Releasing", `${REPO}/blob/main/docs/RELEASING.md`],
                  ],
                },
                {
                  title: "Project",
                  links: [
                    ["GitHub", REPO],
                    ["Issues", `${REPO}/issues`],
                    ["License (AGPL-3.0)", `${REPO}/blob/main/LICENSE`],
                  ],
                },
              ] as { title: string; links: [string, string][] }[]
            ).map((col) => (
              <Box key={col.title}>
                <Typography variant="subtitle2" sx={{ mb: 1.5 }}>
                  {col.title}
                </Typography>
                <Stack spacing={1}>
                  {col.links.map(([label, href]) => (
                    <Link
                      key={label}
                      href={href}
                      target={href.startsWith("#") ? undefined : "_blank"}
                      rel="noreferrer"
                      variant="body2"
                      color="text.secondary"
                      underline="hover"
                    >
                      {label}
                    </Link>
                  ))}
                </Stack>
              </Box>
            ))}
          </Box>
          <Typography
            variant="caption"
            color="text.disabled"
            sx={{ display: "block", mt: 5, pt: 3, borderTop: 1, borderColor: "border.light" }}
          >
            {info.data ? `${info.data.name} · server v${info.data.version}` : "Termoso"} · AGPL-3.0
            · No cookies, no trackers.
          </Typography>
        </Container>
      </Box>
    </Box>
  );
}
