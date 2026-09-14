import { Alert, Button, Link, Stack, Typography } from "@mui/material";
import OpenInNewRoundedIcon from "@mui/icons-material/OpenInNewRounded";
import { Navigate, useLocation, useParams } from "react-router";
import { CopyField } from "@/components/CopyField";
import { AuthTitle } from "./common";
import { appJoinLink, isSessionId, secretFromHash, serverBase } from "./appLinks";

const RELEASES = "https://github.com/rep0rtDev/termoso/releases/latest";

/**
 * Landing page for `https://<server>/join/<id>#<secret>` live-sharing links.
 * The page never talks to the API: the secret stays in the fragment and the
 * only thing it does is hand the link to an installed client.
 */
export function JoinPage() {
  const { session = "" } = useParams();
  const { hash } = useLocation();

  if (!isSessionId(session)) return <Navigate to="/login" replace />;

  const secret = secretFromHash(hash);
  if (!secret) {
    return (
      <>
        <AuthTitle title="Incomplete sharing link" />
        <Alert severity="warning">
          This link is missing the part after <b>#</b> that unlocks the shared terminal. Ask the
          host to send the complete link — the secret is never stored on the server, so it cannot be
          recovered here.
        </Alert>
      </>
    );
  }

  const server = serverBase(window.location, "/join/");
  const webLink = `${server}join/${session}#${secret}`;
  const appLink = appJoinLink(session, server, secret);

  return (
    <>
      <AuthTitle
        title="Join a shared terminal"
        subtitle="Somebody is sharing a live terminal with you. It opens in the Termoso app, end-to-end encrypted — the server only relays ciphertext."
      />
      <Stack spacing={2}>
        <Button
          variant="contained"
          size="large"
          href={appLink}
          endIcon={<OpenInNewRoundedIcon />}
          data-testid="open-in-app"
        >
          Open in Termoso
        </Button>
        <Typography variant="body2" color="text.secondary">
          Nothing happened? Paste this link into Termoso — <i>Connections → Join shared terminal</i>{" "}
          on Android, or the search field on desktop.
        </Typography>
        <CopyField label="Sharing link" value={webLink} />
        <Typography variant="body2" color="text.secondary">
          No Termoso yet?{" "}
          <Link href={RELEASES} target="_blank" rel="noreferrer">
            Download the desktop or Android app
          </Link>
          . Live sharing is not available in the browser.
        </Typography>
      </Stack>
    </>
  );
}
