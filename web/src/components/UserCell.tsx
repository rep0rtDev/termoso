import { Avatar, Box, Typography } from "@mui/material";

interface Props {
  email: string;
  displayName?: string;
  /** Marks the current user. */
  you?: boolean;
}

export function UserCell({ email, displayName, you }: Props) {
  const initial = (displayName ?? email).trim().charAt(0).toUpperCase();
  return (
    <Box sx={{ display: "flex", alignItems: "center", gap: 1.5, minWidth: 0 }}>
      <Avatar sx={{ width: 34, height: 34, fontSize: 14, bgcolor: "secondary.dark" }}>
        {initial}
      </Avatar>
      <Box sx={{ minWidth: 0 }}>
        <Typography variant="body2" sx={{ fontWeight: 600 }} noWrap>
          {displayName ?? email}
          {you && (
            <Typography component="span" variant="caption" color="text.secondary" sx={{ ml: 0.75 }}>
              (you)
            </Typography>
          )}
        </Typography>
        {displayName && (
          <Typography variant="caption" color="text.secondary" noWrap sx={{ display: "block" }}>
            {email}
          </Typography>
        )}
      </Box>
    </Box>
  );
}
