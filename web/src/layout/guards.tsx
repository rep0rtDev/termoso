import { Navigate, Outlet, useLocation } from "react-router";
import { useAuthState } from "@/auth/store";

export function RequireAuth() {
  const { session } = useAuthState();
  const location = useLocation();
  if (!session) {
    const next = location.pathname + location.search;
    return <Navigate to={`/login?next=${encodeURIComponent(next)}`} replace />;
  }
  return <Outlet />;
}

export function RequireGuest() {
  const { session } = useAuthState();
  if (session) return <Navigate to="/account" replace />;
  return <Outlet />;
}

export function RequireAdmin() {
  const { session } = useAuthState();
  if (!session?.user.is_admin) return <Navigate to="/account" replace />;
  return <Outlet />;
}
