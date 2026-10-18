import { Button } from "@mui/material";
import { tr } from "@/i18n";
import { Link as RouterLink } from "react-router";
import { EmptyState } from "@/components/EmptyState";

export function NotFoundPage() {
  return (
    <EmptyState
      title={tr("Page not found")}
      description={tr("The page you were looking for does not exist.")}
      action={
        <Button component={RouterLink} to="/account" variant="contained">
          {tr("Go to account")}
        </Button>
      }
    />
  );
}
