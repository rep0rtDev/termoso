---
title: Session logs
description: Record terminal sessions, encrypted on the device, kept locally or on the server you chose.
---

## Recording

Turn on **Settings → Session recording → Record terminal sessions** and every terminal session is written to a log as it happens; **Keep recordings for** sets how many days they are kept (0 keeps them forever). Typed passwords are masked by the terminal's no-echo mode, so they do not end up in recordings.

Recordings and the connection history are listed under **Logs** in the sidebar. Open a recording to read or search it and **bookmark** lines you want to come back to.

![Logs](/docs/img/desktop-logs.png)

## Where logs are stored

Recordings are encrypted with your master key and stay on the device that made them. On servers with storage configured (`TERMOSO_S3__*`, see [Self-hosting](/docs/self-hosting/configuration)) the apps can keep session logs on the server too — encrypted on the device before upload, so the server only stores an opaque file.
