import { useState } from "react";
import {
  AppBar,
  Avatar,
  Box,
  Chip,
  Divider,
  Drawer,
  IconButton,
  List,
  ListItemButton,
  ListItemIcon,
  ListItemText,
  ListSubheader,
  Menu,
  MenuItem,
  Toolbar,
  Tooltip,
  Typography,
  useMediaQuery,
  useTheme,
} from "@mui/material";
import MenuRoundedIcon from "@mui/icons-material/MenuRounded";
import PersonRoundedIcon from "@mui/icons-material/PersonRounded";
import ShieldRoundedIcon from "@mui/icons-material/ShieldRounded";
import DevicesRoundedIcon from "@mui/icons-material/DevicesRounded";
import GroupsRoundedIcon from "@mui/icons-material/GroupsRounded";
import LockRoundedIcon from "@mui/icons-material/LockRounded";
import AdminPanelSettingsRoundedIcon from "@mui/icons-material/AdminPanelSettingsRounded";
import PeopleAltRoundedIcon from "@mui/icons-material/PeopleAltRounded";
import TuneRoundedIcon from "@mui/icons-material/TuneRounded";
import LogoutRoundedIcon from "@mui/icons-material/LogoutRounded";
import DeleteForeverRoundedIcon from "@mui/icons-material/DeleteForeverRounded";
import KeyRoundedIcon from "@mui/icons-material/KeyRounded";
import { NavLink, Outlet, useLocation, useNavigate } from "react-router";
import { Logo } from "@/components/Logo";
import { ThemeToggle } from "./ThemeToggle";
import { useAuthState } from "@/auth/store";
import { logout } from "@/auth/flows";
import { useServerInfo } from "@/api/hooks";
import { UnlockDialog } from "@/auth/UnlockDialog";

const DRAWER_WIDTH = 264;

interface NavItem {
  to: string;
  label: string;
  icon: React.ReactNode;
  end?: boolean;
}

const accountNav: NavItem[] = [
  { to: "/account", label: "Account", icon: <PersonRoundedIcon /> },
  { to: "/security", label: "Security", icon: <ShieldRoundedIcon /> },
  { to: "/devices", label: "Devices", icon: <DevicesRoundedIcon /> },
];

const workspaceNav: NavItem[] = [
  { to: "/team", label: "Teams", icon: <GroupsRoundedIcon /> },
  { to: "/vaults", label: "Vaults", icon: <LockRoundedIcon /> },
];

const adminNav: NavItem[] = [
  { to: "/admin", label: "Overview", icon: <AdminPanelSettingsRoundedIcon />, end: true },
  { to: "/admin/users", label: "Users", icon: <PeopleAltRoundedIcon /> },
  { to: "/admin/teams", label: "Teams", icon: <GroupsRoundedIcon /> },
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
    <List
      dense
      subheader={
        title ? (
          <ListSubheader disableSticky sx={{ bgcolor: "transparent", lineHeight: "32px" }}>
            {title}
          </ListSubheader>
        ) : undefined
      }
    >
      {items.map((it) => {
        const selected = it.end
          ? location.pathname === it.to
          : location.pathname === it.to || location.pathname.startsWith(it.to + "/");
        return (
          <ListItemButton
            key={it.to}
            component={NavLink}
            to={it.to}
            selected={selected}
            onClick={onNavigate}
          >
            <ListItemIcon>{it.icon}</ListItemIcon>
            <ListItemText primary={it.label} />
          </ListItemButton>
        );
      })}
    </List>
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
    <Box sx={{ display: "flex", flexDirection: "column", height: "100%" }}>
      <Toolbar sx={{ px: 2.5 }}>
        <Logo />
      </Toolbar>
      <Box sx={{ flex: 1, overflowY: "auto", pb: 2 }}>
        <NavSection items={accountNav} onNavigate={closeDrawer} />
        {info.data?.features.teams !== false && (
          <NavSection title="Workspace" items={workspaceNav} onNavigate={closeDrawer} />
        )}
        {user?.is_admin && (
          <NavSection title="Administration" items={adminNav} onNavigate={closeDrawer} />
        )}
      </Box>
      <Divider />
      <Box sx={{ p: 2, display: "flex", alignItems: "center", gap: 1, color: "text.secondary" }}>
        <Typography variant="caption" sx={{ flex: 1 }} noWrap>
          {info.data ? `${info.data.name} · v${info.data.version}` : "Termoso"}
        </Typography>
        <Tooltip
          title={
            privateKey
              ? "Encryption keys unlocked in this tab"
              : "Encryption keys locked — sign in with password to unlock"
          }
        >
          <KeyRoundedIcon fontSize="small" color={privateKey ? "primary" : "disabled"} />
        </Tooltip>
      </Box>
    </Box>
  );

  const initials = (user?.display_name ?? user?.email ?? "?").slice(0, 1).toUpperCase();

  return (
    <Box sx={{ display: "flex", minHeight: "100vh" }}>
      <AppBar
        position="fixed"
        color="transparent"
        sx={{
          backdropFilter: "blur(12px)",
          bgcolor: "rgba(var(--mui-palette-background-defaultChannel) / 0.8)",
          borderBottom: 1,
          borderColor: "divider",
          width: { md: `calc(100% - ${DRAWER_WIDTH}px)` },
          ml: { md: `${DRAWER_WIDTH}px` },
        }}
      >
        <Toolbar sx={{ gap: 1 }}>
          {!isDesktop && (
            <IconButton
              edge="start"
              onClick={() => setMobileOpen(true)}
              aria-label="Open navigation"
            >
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
            />
          )}
          <ThemeToggle />
          <Tooltip title={user?.email ?? ""}>
            <IconButton
              onClick={(e) => setMenuAnchor(e.currentTarget)}
              sx={{ p: 0.5 }}
              aria-label="Account menu"
            >
              <Avatar
                sx={{
                  width: 32,
                  height: 32,
                  bgcolor: "primary.main",
                  color: "primary.contrastText",
                  fontSize: 14,
                  fontWeight: 700,
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
            slotProps={{ paper: { sx: { minWidth: 240, mt: 1 } } }}
          >
            <Box sx={{ px: 2, py: 1.5 }}>
              <Typography variant="subtitle2" noWrap>
                {user?.display_name ?? user?.email}
              </Typography>
              <Typography variant="caption" color="text.secondary" noWrap component="div">
                {user?.email}
              </Typography>
            </Box>
            <Divider />
            <MenuItem
              onClick={() => {
                setMenuAnchor(null);
                void navigate("/delete-account");
              }}
            >
              <ListItemIcon>
                <DeleteForeverRoundedIcon fontSize="small" />
              </ListItemIcon>
              <ListItemText>Delete account</ListItemText>
            </MenuItem>
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
        </Toolbar>
      </AppBar>

      <Box component="nav" sx={{ width: { md: DRAWER_WIDTH }, flexShrink: { md: 0 } }}>
        <Drawer
          variant={isDesktop ? "permanent" : "temporary"}
          open={isDesktop || mobileOpen}
          onClose={closeDrawer}
          slotProps={{
            paper: {
              sx: {
                width: DRAWER_WIDTH,
                borderRight: 1,
                borderColor: "divider",
                bgcolor: "background.paper",
              },
            },
          }}
          ModalProps={{ keepMounted: true }}
        >
          {drawer}
        </Drawer>
      </Box>

      <Box component="main" sx={{ flex: 1, minWidth: 0, display: "flex", flexDirection: "column" }}>
        <Toolbar />
        <Box
          sx={{ p: { xs: 2, sm: 3, md: 4 }, maxWidth: 1040, width: "100%", mx: "auto", flex: 1 }}
        >
          <Outlet />
        </Box>
      </Box>
      <UnlockDialog />
    </Box>
  );
}
