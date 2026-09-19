import { Navigate } from "react-router";
import { useAuthState } from "@/auth/store";
import { useServerInfo } from "@/api/hooks";
import { LandingPage } from "./LandingPage";

/**
 * `/`: the landing page, unless the server hides it on this origin
 * (`TERMOSO_LANDING=false`, or the landing lives on `TERMOSO_LANDING_URL`),
 * in which case the cabinet starts at sign-in. The server already redirects
 * a fresh page load; this covers client-side navigation back to `/`.
 */
export function RootPage() {
  const info = useServerInfo();
  const { session } = useAuthState();
  if (info.isPending) return null;
  if (info.data?.landing === false) {
    return <Navigate to={session ? "/account" : "/login"} replace />;
  }
  return <LandingPage />;
}
