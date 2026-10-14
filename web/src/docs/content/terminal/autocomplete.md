---
title: Autocomplete and shell integration
description: Suggestions from your own history and snippets, prompt markers and keyword highlighting — computed on the device, never in a cloud.
---

## Shell integration

Termoso understands the OSC 133 prompt markers that modern shells can emit. With them it knows where each prompt, command and output starts, which powers autocomplete, jumping between commands and the command history of a session. Nothing has to be installed on the server for basic suggestions; the markers make them precise.

## Autocomplete

As you type at a prompt, a popup suggests completions from:

- commands you ran before on this host (local history);
- your snippets;
- the remote shell history, if **Import shell history** is on.

Use <kbd>↑</kbd>/<kbd>↓</kbd> to choose and <kbd>Tab</kbd> or <kbd>Enter</kbd> to accept. Turn it off with **Settings → Terminal → Command autocomplete**.

## Keyword highlighting

**Keyword highlighting** colours words like `error`, `warning` or `failed` in the output so they stand out in long logs. It works on the stream, so it does not change what the server sends.

## Privacy

Every suggestion is computed on the device from data in your vault. Nothing you type is sent anywhere except to the server you are connected to. The optional AI suggestions are a separate, opt-in feature that only runs when you ask for it.
