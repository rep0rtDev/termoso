import { createBrowserRouter, Navigate } from "react-router";
import { AppShell } from "./layout/AppShell";
import { AuthLayout } from "./layout/AuthLayout";
import { RequireAdmin, RequireAuth, RequireGuest } from "./layout/guards";
import { LoginPage } from "./pages/auth/LoginPage";
import { MfaPage } from "./pages/auth/MfaPage";
import { DeviceApprovePage } from "./pages/auth/DeviceApprovePage";
import { SignupPage } from "./pages/auth/SignupPage";
import { RecoveryKeyPage } from "./pages/auth/RecoveryKeyPage";
import { StartOverPage } from "./pages/auth/StartOverPage";
import { StartOverCancelPage, StartOverFinishPage } from "./pages/auth/StartOverFinishPage";
import { ForgotPasswordPage } from "./pages/auth/ForgotPasswordPage";
import { InvitePage } from "./pages/auth/InvitePage";
import { SsoCallbackPage } from "./pages/auth/SsoCallbackPage";
import { AccountPage } from "./pages/account/AccountPage";
import { SecurityPage } from "./pages/account/SecurityPage";
import { DevicesPage } from "./pages/account/DevicesPage";
import { DeleteAccountPage } from "./pages/account/DeleteAccountPage";
import { TeamsPage } from "./pages/team/TeamsPage";
import { TeamPage } from "./pages/team/TeamPage";
import { VaultsPage } from "./pages/vaults/VaultsPage";
import { VaultPage } from "./pages/vaults/VaultPage";
import { AdminOverviewPage } from "./pages/admin/AdminOverviewPage";
import { AdminUsersPage } from "./pages/admin/AdminUsersPage";
import { AdminTeamsPage } from "./pages/admin/AdminTeamsPage";
import { AdminSettingsPage } from "./pages/admin/AdminSettingsPage";
import { NotFoundPage } from "./pages/NotFoundPage";
import { LandingPage } from "./pages/LandingPage";

export const router = createBrowserRouter([
  { path: "/", element: <LandingPage /> },
  {
    element: <AuthLayout />,
    children: [
      {
        element: <RequireGuest />,
        children: [
          { path: "/login", element: <LoginPage /> },
          { path: "/login/mfa", element: <MfaPage /> },
          { path: "/login/approve", element: <DeviceApprovePage /> },
          { path: "/signup", element: <SignupPage /> },
          { path: "/forgot-password", element: <ForgotPasswordPage /> },
          { path: "/start-over", element: <StartOverPage /> },
        ],
      },
      { path: "/start-over/cancel/:token", element: <StartOverCancelPage /> },
      { path: "/start-over/:token", element: <StartOverFinishPage /> },
      { path: "/signup/recovery-key", element: <RecoveryKeyPage /> },
      { path: "/invite/:token", element: <InvitePage /> },
      { path: "/sso/callback", element: <SsoCallbackPage /> },
    ],
  },
  {
    element: <RequireAuth />,
    children: [
      {
        element: <AppShell />,
        children: [
          { path: "/account", element: <AccountPage /> },
          { path: "/confirm/email", element: <Navigate to="/account#email" replace /> },
          { path: "/security", element: <SecurityPage /> },
          { path: "/devices", element: <DevicesPage /> },
          { path: "/delete-account", element: <DeleteAccountPage /> },
          { path: "/team", element: <TeamsPage /> },
          { path: "/team/:id", element: <TeamPage /> },
          { path: "/vaults", element: <VaultsPage /> },
          { path: "/vaults/:id", element: <VaultPage /> },
          {
            element: <RequireAdmin />,
            children: [
              { path: "/admin", element: <AdminOverviewPage /> },
              { path: "/admin/users", element: <AdminUsersPage /> },
              { path: "/admin/teams", element: <AdminTeamsPage /> },
              { path: "/admin/settings", element: <AdminSettingsPage /> },
            ],
          },
          { path: "*", element: <NotFoundPage /> },
        ],
      },
    ],
  },
]);
