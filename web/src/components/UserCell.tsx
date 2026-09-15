import { Box, Typography } from "@mui/material";
import { UserAvatar } from "./UserAvatar";

interface Props {
  userId: string;
  email: string;
  displayName?: string;
  avatar?: string;
  /** Marks the current user. */
  you?: boolean;
}

export function UserCell({ userId, email, displayName, avatar, you }: Props) {
  return (
    <Box sx={{ display: "flex", alignItems: "center", gap: 1.5, minWidth: 0 }}>
      <UserAvatar userId={userId} tag={avatar} email={email} displayName={displayName} />
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
