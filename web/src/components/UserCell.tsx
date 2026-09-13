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
      <Avatar sx={{ width: 32, height: 32, fontSize: 13 }}>{initial}</Avatar>
      <Box sx={{ minWidth: 0 }}>
        <Typography variant="body1" sx={{ fontWeight: 500, lineHeight: 1.35 }} noWrap>
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
