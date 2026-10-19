import { Button } from "@mui/material";
import { tr } from "@/i18n";
import { Link as RouterLink } from "react-router";
import { EmptyState } from "@/components/EmptyState";

export function NotFoundPage({ to = "/account", label }: { to?: string; label?: string }) {
  return (
    <EmptyState
      title={tr("Page not found")}
      description={tr("The page you were looking for does not exist.")}
      action={
        <Button component={RouterLink} to={to} variant="contained">
          {label ?? tr("Go to account")}
        </Button>
      }
    />
  );
}
