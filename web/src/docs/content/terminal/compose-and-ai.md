---
title: Compose, voice input and AI agents
description: Write long prompts and multi-line commands in a proper text box, dictate them, paste images and files into the terminal, and start Claude Code or another coding agent the moment a host connects.
---

Terminals were designed for one line at a time; working with an AI coding agent over SSH from a phone is anything but. The Android terminal has a few tools for that.

## Compose

The **Compose** key (the pencil-on-lines icon in the expanded key panel, also available as a key-panel action) opens a text box above the keyboard where you can write and edit freely — several lines, a long prompt for an agent, a `heredoc`, a commit message.

![The Compose box](/docs/img/android-compose.png)

- **Send** types the text into the session followed by <kbd>Enter</kbd>.
- **Paste only** types it without <kbd>Enter</kbd>, so you can review it at the prompt or add to it.
- Newlines are sent as they are; agents and shells that accept multi-line input (Claude Code, `python`, `psql`) receive the whole block.
- The box keeps its contents while the session is open, so a prompt you are still shaping survives a look at the terminal.

## Voice input

The microphone in the key panel and in the Compose box dictate through the system speech recogniser. Termoso asks the recogniser to work **offline first**; on devices with on-device models (Pixels, most recent Samsung and Xiaomi phones with the language pack installed) nothing leaves the phone. Where no offline model exists, the device's default recogniser is used — that is a property of the phone, not of Termoso, and it is the same engine the keyboard's microphone uses.

Dictated text lands in the Compose box when it is open, otherwise in the terminal at the cursor. The recogniser is only started while you hold or tap the microphone; Termoso has no microphone access the rest of the time, and the permission can be denied without affecting anything else.

## Pasting images and files

Pick an image or a file from the keyboard's sticker or clipboard panel, share one from another app, or use the **Send file…** paper clip in the key panel. Termoso asks once (**Send photo.jpg to web-01?**), uploads the file over the session's own SFTP channel to **`/tmp` on the host**, types the resulting path at the cursor and deletes its local scratch copy.

Names are made shell-safe and get a numeric suffix if the name is taken (`screenshot-2.png`), so the path can be used as is — `claude` and `gemini` pick up an image path typed into their prompt, and `cat /tmp/config.yaml` works the moment it is typed.

> [!NOTE]
> Files go to `/tmp` because it exists everywhere and is cleaned by the host. Move anything you want to keep.

## AI coding agent on connect

In **Host details → AI coding agent** choose **Claude Code**, **Gemini CLI**, **Codex CLI**, **OpenCode** or **Aider**. When the host connects, Termoso waits for the prompt and starts the agent, so a phone tap takes you straight into the agent session rather than a shell.

![Choosing an agent for a host](/docs/img/android-ai-agent.png)

The agent has to be installed on the host and on `PATH`; the setting only runs its command. Combine it with a **startup snippet** when a project directory or environment variables are needed first — the snippet runs before the agent is started. The choice is part of the host, so it syncs and can live in a team vault.

## Ask AI

**Ask AI** (the sparkle button, <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>A</kbd> on the desktop) turns a sentence into a shell command, shows the command, and inserts it only when you accept. It is off until you sign in to a server that has `TERMOSO_AI__*` configured — the free cloud and self-hosted servers alike — and nothing is sent unless you press the button. The prompt and the command are not stored by the server.

Autocomplete, by contrast, is entirely local and does not involve any model: see [Autocomplete](/docs/terminal/autocomplete).
