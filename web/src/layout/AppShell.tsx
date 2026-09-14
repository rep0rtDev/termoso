import { useState } from "react";
import {
  Avatar,
  Box,
  Chip,
  Divider,
  Drawer,
  IconButton,
  ListItemIcon,
  ListItemText,
  Menu,
  MenuItem,
  Tooltip,
  Typography,
  useMediaQuery,
  useTheme,
} from "@mui/material";
import MenuRoundedIcon from "@mui/icons-material/MenuRounded";
import PersonOutlineRoundedIcon from "@mui/icons-material/PersonOutlineRounded";
import ShieldOutlinedIcon from "@mui/icons-material/ShieldOutlined";
import DevicesOutlinedIcon from "@mui/icons-material/DevicesOutlined";
import GroupsOutlinedIcon from "@mui/icons-material/GroupsOutlined";
import LockOutlinedIcon from "@mui/icons-material/LockOutlined";
import DashboardOutlinedIcon from "@mui/icons-material/DashboardOutlined";
import PeopleAltOutlinedIcon from "@mui/icons-material/PeopleAltOutlined";
import TuneRoundedIcon from "@mui/icons-material/TuneRounded";
import LogoutRoundedIcon from "@mui/icons-material/LogoutRounded";
import DeleteOutlineRoundedIcon from "@mui/icons-material/DeleteOutlineRounded";
import KeyRoundedIcon from "@mui/icons-material/KeyRounded";
import OpenInNewRoundedIcon from "@mui/icons-material/OpenInNewRounded";
import { NavLink, Outlet, useLocation, useNavigate } from "react-router";
import { Logo } from "@/components/Logo";
import { ThemeToggle } from "./ThemeToggle";
import { useAuthState } from "@/auth/store";
import { logout } from "@/auth/flows";
import { useServerInfo } from "@/api/hooks";
import { UnlockDialog } from "@/auth/UnlockDialog";
import { ResetScheduledBanner } from "./ResetScheduledBanner";
import { sizes } from "@/theme/theme";

interface NavItem {
  to: string;
  label: string;
  icon: React.ReactNode;
  end?: boolean;
}

const accountNav: NavItem[] = [
  { to: "/account", label: "Account", icon: <PersonOutlineRoundedIcon /> },
  { to: "/security", label: "Security", icon: <ShieldOutlinedIcon /> },
  { to: "/devices", label: "Devices", icon: <DevicesOutlinedIcon /> },
];

const workspaceNav: NavItem[] = [
  { to: "/team", label: "Teams", icon: <GroupsOutlinedIcon /> },
  { to: "/vaults", label: "Vaults", icon: <LockOutlinedIcon /> },
];

const adminNav: NavItem[] = [
  { to: "/admin", label: "Overview", icon: <DashboardOutlinedIcon />, end: true },
  { to: "/admin/users", label: "Users", icon: <PeopleAltOutlinedIcon /> },
  { to: "/admin/teams", label: "Teams", icon: <GroupsOutlinedIcon /> },
  { to: "/admin/settings", label: "Server settings", icon: <TuneRoundedIcon /> },
];

function NavSection({
  title,
  items,
  onNavigate,
}: {
  title?: string;
  items: NavItem[];
  onNavigate: () => void;
}) {
  const location = useLocation();
  return (
    <Box sx={{ px: 1.5, pt: title ? 2 : 0.5 }}>
      {title && (
        <Typography
          variant="overline"
          component="div"
          sx={{ color: "text.disabled", px: 1.25, mb: 0.5, lineHeight: 1.6 }}
        >
          {title}
        </Typography>
      )}
      <Box sx={{ display: "flex", flexDirection: "column", gap: 0.25 }}>
        {items.map((it) => {
          const selected = it.end
            ? location.pathname === it.to
            : location.pathname === it.to || location.pathname.startsWith(it.to + "/");
          return (
            <Box
              key={it.to}
              component={NavLink}
              to={it.to}
              onClick={onNavigate}
              sx={{
                display: "flex",
                alignItems: "center",
                gap: 1.25,
                height: sizes.control,
                px: 1.25,
                borderRadius: 1.5,
                textDecoration: "none",
                color: selected ? "text.primary" : "text.secondary",
                bgcolor: selected ? "surface.highest" : "transparent",
                fontWeight: 500,
                fontSize: "0.875rem",
                "&:hover": {
                  bgcolor: selected ? "surface.highest" : "action.hover",
                  color: "text.primary",
                },
                "& svg": { fontSize: 18, color: selected ? "text.primary" : "text.secondary" },
              }}
            >
              {it.icon}
              {it.label}
            </Box>
          );
        })}
      </Box>
    </Box>
  );
}

export function AppShell() {
  const theme = useTheme();
  const isDesktop = useMediaQuery(theme.breakpoints.up("md"));
  const [mobileOpen, setMobileOpen] = useState(false);
  const [menuAnchor, setMenuAnchor] = useState<HTMLElement | null>(null);
  const { session, privateKey } = useAuthState();
  const info = useServerInfo();
  const navigate = useNavigate();
  const user = session?.user;
  const closeDrawer = () => setMobileOpen(false);

  const drawer = (
    <Box
      sx={{
        display: "flex",
        flexDirection: "column",
        height: "100%",
        bgcolor: "surface.base",
      }}
    >
      <Box
        component={NavLink}
        to="/"
        sx={{
          height: sizes.topbar,
          display: "flex",
          alignItems: "center",
          px: 2.5,
          textDecoration: "none",
          color: "inherit",
          flexShrink: 0,
        }}
      >
        <Logo size={26} />
      </Box>
      <Box sx={{ flex: 1, overflowY: "auto", pb: 2 }}>
        <NavSection items={accountNav} onNavigate={closeDrawer} />
        {info.data?.features.teams !== false && (
          <NavSection title="Workspace" items={workspaceNav} onNavigate={closeDrawer} />
        )}
        {user?.is_admin && (
          <NavSection title="Administration" items={adminNav} onNavigate={closeDrawer} />
        )}
      </Box>
      <Box
        sx={{
          px: 2.5,
          py: 1.5,
          display: "flex",
          alignItems: "center",
          gap: 1,
          color: "text.disabled",
        }}
      >
        <Typography variant="caption" sx={{ flex: 1 }} noWrap>
          {info.data ? `${info.data.name} · v${info.data.version}` : "Termoso"}
        </Typography>
        <Tooltip
          title={
            privateKey
              ? "Encryption keys unlocked in this tab"
              : "Encryption keys locked — they unlock with your password when needed"
          }
        >
          <KeyRoundedIcon
            sx={{ fontSize: 16, color: privateKey ? "primary.main" : "text.disabled" }}
          />
        </Tooltip>
      </Box>
    </Box>
  );

  const initials = (user?.display_name ?? user?.email ?? "?").slice(0, 1).toUpperCase();

  return (
    <Box sx={{ display: "flex", minHeight: "100vh", bgcolor: "surface.lowest" }}>
      <Box component="nav" sx={{ width: { md: sizes.sidebar }, flexShrink: { md: 0 } }}>
        <Drawer
          variant={isDesktop ? "permanent" : "temporary"}
          open={isDesktop || mobileOpen}
          onClose={closeDrawer}
          slotProps={{
            paper: {
              sx: { width: sizes.sidebar, border: 0, bgcolor: "surface.base" },
            },
          }}
          ModalProps={{ keepMounted: true }}
        >
          {drawer}
        </Drawer>
      </Box>

      <Box component="main" sx={{ flex: 1, minWidth: 0, display: "flex", flexDirection: "column" }}>
        <Box
          component="header"
          sx={{
            position: "sticky",
            top: 0,
            zIndex: (t) => t.zIndex.appBar,
            height: sizes.topbar,
            display: "flex",
            alignItems: "center",
            gap: 1,
            px: { xs: 1.5, md: 3 },
            bgcolor: "surface.lowest",
          }}
        >
          {!isDesktop && (
            <IconButton onClick={() => setMobileOpen(true)} aria-label="Open navigation">
              <MenuRoundedIcon />
            </IconButton>
          )}
          <Box sx={{ flex: 1 }} />
          {user && !user.email_verified && (
            <Chip
              size="small"
              color="warning"
              variant="outlined"
              label="Email not verified"
              onClick={() => navigate("/account#email")}
              sx={{ cursor: "pointer" }}
            />
          )}
          <ThemeToggle />
          <Tooltip title={user?.email ?? ""}>
            <IconButton
              onClick={(e) => setMenuAnchor(e.currentTarget)}
              aria-label="Account menu"
              sx={{ ml: 0.5 }}
            >
              <Avatar
                sx={{
                  width: 26,
                  height: 26,
                  fontSize: 12,
                  bgcolor: "primary.main",
                  color: "primary.contrastText",
                }}
              >
                {initials}
              </Avatar>
            </IconButton>
          </Tooltip>
          <Menu
            anchorEl={menuAnchor}
            open={menuAnchor !== null}
            onClose={() => setMenuAnchor(null)}
            anchorOrigin={{ vertical: "bottom", horizontal: "right" }}
            transformOrigin={{ vertical: "top", horizontal: "right" }}
            slotProps={{ paper: { sx: { minWidth: 240 } } }}
          >
            <Box sx={{ px: 1.5, py: 1 }}>
              <Typography variant="subtitle2" noWrap>
                {user?.display_name ?? user?.email}
              </Typography>
              <Typography variant="caption" color="text.secondary" noWrap component="div">
                {user?.email}
              </Typography>
            </Box>
            <Divider sx={{ my: 0.5 }} />
            <MenuItem
              component="a"
              href="https://github.com/rep0rtDev/termoso/releases"
              target="_blank"
              rel="noreferrer"
              onClick={() => setMenuAnchor(null)}
            >
              <ListItemIcon>
                <OpenInNewRoundedIcon fontSize="small" />
              </ListItemIcon>
              <ListItemText>Download the app</ListItemText>
            </MenuItem>
            <MenuItem
              onClick={() => {
                setMenuAnchor(null);
                void navigate("/delete-account");
              }}
            >
              <ListItemIcon>
                <DeleteOutlineRoundedIcon fontSize="small" />
              </ListItemIcon>
              <ListItemText>Delete account</ListItemText>
            </MenuItem>
            <Divider sx={{ my: 0.5 }} />
            <MenuItem
              onClick={() => {
                setMenuAnchor(null);
                void logout();
              }}
            >
              <ListItemIcon>
                <LogoutRoundedIcon fontSize="small" />
              </ListItemIcon>
              <ListItemText>Sign out</ListItemText>
            </MenuItem>
          </Menu>
        </Box>
        <ResetScheduledBanner />
        <Box
          sx={{
            px: { xs: 2, sm: 3, md: 4 },
            pt: { xs: 1, md: 2 },
            pb: 6,
            maxWidth: sizes.content + 64,
            width: "100%",
            mx: "auto",
            flex: 1,
          }}
        >
          <Outlet />
        </Box>
      </Box>
      <UnlockDialog />
    </Box>
  );
}
