---
title: Linux on Android
description: A real Alpine or Ubuntu userland for the local terminal on your phone — apt, apk, python, git, ssh — without root.
---

The plain Android shell has no package manager and almost no tools. Since 0.8.0 the local terminal can instead run a complete Linux distribution inside the app: an **official Alpine or Ubuntu root filesystem** downloaded from the distribution's own mirror, unpacked into Termoso's private storage and executed under **proot**. Nothing is modified on the phone, root is not needed, and uninstalling the app removes every trace.

![Linux environment settings](/docs/img/android-linux-env.png)

## Installing

1. Open **Settings → Linux environment**.
2. Pick a distribution:

   | Distribution          | Download    | Package manager | Good for                                                                                         |
   | --------------------- | ----------- | --------------- | ------------------------------------------------------------------------------------------------ |
   | **Alpine Linux 3.21** | about 4 MB  | `apk`           | A small, fast toolbox: `openssh`, `curl`, `python3`, `git`, `rsync` are a few seconds away       |
   | **Ubuntu 24.04 LTS**  | about 30 MB | `apt`           | The full Debian family — anything that expects glibc, `.deb` packages, Node, Go, Rust toolchains |

3. Press **Install**. The tarball is downloaded, checked against a SHA-256 pinned in the app, unpacked (symlinks, hard links and permissions preserved) and configured. Progress is shown per stage; the download can be cancelled and resumed later.

After that **Local terminal** opens the Linux shell by default. The Android shell is still available as **Android shell** in the same menu, for `pm`, `am`, `logcat` and the like.

![Ubuntu running in the local terminal](/docs/img/android-linux-shell.png)

## What you get

- A root shell (`root@localhost`) with the distribution's `PATH`, `/etc`, `/usr`, `/var`.
- Working **DNS**: the resolver is filled in from the phone's current network at every start, so `apt update`, `pip install`, `git clone` and `ssh` work on Wi-Fi and mobile data alike.
- `/proc`, `/sys`, `/dev` (including `/dev/pts`, `/dev/null`, `/dev/urandom`) from the phone, so TUI programs, `ps`, `top` and anything that needs a pseudo-terminal behave.
- The phone's shared storage mounted at **`/mnt/sdcard`** — copy files in and out with ordinary `cp`, or `tar` a project straight into Downloads.
- Your home directory and everything you install persist in the app's data until you remove the environment.
- `uname` reports a Linux kernel the distribution is happy with; `hostname` is set so tools that insist on one stay quiet.

```sh
apt update && apt install -y python3 git openssh-client
git clone https://github.com/you/project /root/project
ssh deploy@web-01       # keys live in /root/.ssh like on any Linux box
```

## Limits worth knowing

- **proot, not a VM.** Programs run as ordinary Android processes with system calls translated. Expect roughly native speed for most tools and a noticeable slowdown for things that `fork()` thousands of times (large configure scripts, some test suites).
- **No real root.** `root` inside the environment is an illusion proot provides for package managers. You cannot load kernel modules, bind ports below 1024, use `ping` raw sockets or `mount`. `sudo` is unnecessary and not installed.
- **No systemd.** Services start the Debian/Alpine way: `service nginx start`, `/etc/init.d/…`, or just run the daemon in the foreground in another tab.
- **Alpine on x86_64 is hidden.** On Intel/AMD devices (Chromebooks, emulators) musl's `fork()` uses a system call that Android's seccomp filter blocks for every app; Ubuntu uses `clone()` and is unaffected. On arm64 phones — that is, nearly all of them — both distributions are offered.
- **Battery.** A shell that is just sitting at a prompt costs nothing, but a compile does; the session keeps running in the foreground service while you switch apps, which the [notification](/docs/terminal/mobile#notification-shade) shows.

## Removing or reinstalling

**Settings → Linux environment → Remove Linux** deletes the root filesystem, installed packages and everything in its home directory, and frees the space immediately. Installing again starts from a fresh tarball — keep your work in `/mnt/sdcard` or push it to a server if you plan to switch distributions.

> [!WARNING]
> The environment lives in app-private storage. Clearing Termoso's data in Android settings or uninstalling the app removes it together with the Local vault, with no way back. Back up the vault to your account first and copy files you care about out of the environment.

> [!NOTE]
> Nothing about this feature talks to a Termoso server. The two download URLs point at `dl-cdn.alpinelinux.org` and `cdimage.ubuntu.com`, and the checksums the app verifies against are in the [source](https://github.com/rep0rtDev/termoso/blob/main/crates/termoso-mobile/src/linux.rs).
