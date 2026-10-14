---
title: Terminal sharing
description: Invite someone into a live terminal session, end-to-end encrypted, from the desktop or Android.
---

Terminal sharing (multiplayer) lets other people watch — or type into — one of your sessions in real time.

## Sharing a session

Open the session's menu and choose **Share this terminal**. Termoso creates a link like

```
https://app.termoso.dev/join/<id>#<secret>
```

The part after `#` is the session key. Browsers never send the fragment to the server, so the server that relays the session never sees it.

## Joining

Open the link: it opens the desktop or Android app (Android App Links), or **Join shared terminal** in Connections accepts a pasted link. Viewers see the terminal as it is; the host decides whether they may type.

## Security

The terminal stream is encrypted on the host's device with the per-session key and decrypted on the viewers' devices. The server forwards frames it cannot read. Closing the share, or the session, disconnects everyone.
