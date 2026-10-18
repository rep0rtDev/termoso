import { Alert, Button, Link, Stack, Typography } from "@mui/material";
import { tr, trx } from "@/i18n";
import OpenInNewRoundedIcon from "@mui/icons-material/OpenInNewRounded";
import { Navigate, useLocation, useParams } from "react-router";
import { CopyField } from "@/components/CopyField";
import { AuthTitle } from "./common";
import { appJoinLink, isSessionId, secretFromHash, serverBase } from "./appLinks";

const RELEASES = "https://github.com/termoso/termoso/releases/latest";

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
        <AuthTitle title={tr("Incomplete sharing link")} />
        <Alert severity="warning">
          {trx(
            "This link is missing the part after {v1} that unlocks the shared terminal. Ask the host to send the complete link — the secret is never stored on the server, so it cannot be recovered here.",
            { v1: <b>#</b> },
          )}
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
        title={tr("Join a shared terminal")}
        subtitle={tr(
          "Somebody is sharing a live terminal with you. It opens in the Termoso app, end-to-end encrypted — the server only relays ciphertext.",
        )}
      />
      <Stack spacing={2}>
        <Button
          variant="contained"
          size="large"
          href={appLink}
          endIcon={<OpenInNewRoundedIcon />}
          data-testid="open-in-app"
        >
          {tr("Open in Termoso")}
        </Button>
        <Typography variant="body2" color="text.secondary">
          {trx(
            "Nothing happened? Paste this link into Termoso —  {v1} on Android, or the search field on desktop.",
            { v1: <i>{tr("Connections → Join shared terminal")}</i> },
          )}
        </Typography>
        <CopyField label={tr("Sharing link")} value={webLink} />
        <Typography variant="body2" color="text.secondary">
          {trx("No Termoso yet?  {v1} {t2}", {
            v1: (
              <Link href={RELEASES} target="_blank" rel="noreferrer">
                {tr("Download the desktop or Android app")}
              </Link>
            ),
            t2: tr(". Live sharing is not available in the browser."),
          })}
        </Typography>
      </Stack>
    </>
  );
}
