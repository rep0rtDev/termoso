import { Button } from "@mui/material";
import { Link as RouterLink } from "react-router";
import { EmptyState } from "@/components/EmptyState";

export function NotFoundPage() {
  return (
    <EmptyState
      title="Page not found"
      description="The page you were looking for does not exist."
      action={
        <Button component={RouterLink} to="/account" variant="contained">
          Go to account
        </Button>
      }
    />
  );
}
