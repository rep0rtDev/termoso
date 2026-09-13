# SFTP throughput: Termoso vs Termius vs OpenSSH

Single-file transfers of the same 512 MiB file, same sshd, same port, same
emulated network path, measured with an external filesystem timer. Numbers
below are what was measured on one box on 2026-09-13; they are not a general
claim and will differ on other hardware, kernels and servers.

## Environment

| | |
|---|---|
| Host | Ubuntu 22.04, kernel 5.15, 8 vCPU (Intel Xeon Platinum 8559C), Docker 27.4 |
| Server | OpenSSH 10.3p1 sshd in a Docker container (`pqssh`), `Subsystem sftp internal-sftp`, host port `2299` → container `22` |
| File | `bench.bin`, 536 870 912 bytes, SHA-256 `f20966a9533b4514ec04eff84f395ab74bed67ba82d5c034ff227bc4b395928c` |
| Termoso | desktop `0.1.1`, release build of this branch (`cargo build --release -p termoso-desktop`), Local vault, password auth |
| Termius | Linux desktop `10.0.0` (`Termius.deb`), password auth, same host entry `127.0.0.1:2299` |
| OpenSSH client | `OpenSSH_8.9p1` `sftp` (reference only, not a GUI client) |
| Network | LAN = Docker bridge on loopback (no added delay). WAN = `tc netem delay 40ms` on both the container `eth0` egress and the host-side veth egress → ≈ 80 ms RTT, no loss, no bandwidth cap |

Both GUI clients were driven from their own SFTP UI (drag-and-drop of the file
between panes). Each run was a single file with nothing else transferring;
the desktop transfer queue's 3-way parallelism was **not** exercised.

## Method

* The timer polls the destination file size every 100 ms and reports
  first-byte → full-size wall time; throughput = 512 MiB / that time.
  The rate shown inside each client's UI is a different quantity (their own
  moving average) and is listed separately where noted.
* After every run the destination file was hashed and compared with the
  source; every run below matched.
* Every run was preceded by deleting the destination file.

### Scripts

`netem.sh` — WAN emulation (RTT = 2 × delay):

```bash
#!/usr/bin/env bash
# netem.sh on <one_way_delay_ms> | off
set -eu
PID=$(docker inspect -f '{{.State.Pid}}' pqssh)
IFIDX=$(sudo nsenter -t "$PID" -n ip -o link show eth0 | sed -n 's/^[0-9]*: eth0@if\([0-9]*\):.*/\1/p')
VETH=$(ip -o link | awk -F': ' -v i="$IFIDX" '$1==i {print $2}' | cut -d@ -f1)
case "$VETH" in veth*) ;; *) echo "refusing to touch '$VETH'" >&2; exit 1;; esac
case "$1" in
  on)  sudo nsenter -t "$PID" -n tc qdisc replace dev eth0 root netem delay "${2}ms" limit 100000
       sudo tc qdisc replace dev "$VETH" root netem delay "${2}ms" limit 100000 ;;
  off) sudo nsenter -t "$PID" -n tc qdisc del dev eth0 root 2>/dev/null || true
       sudo tc qdisc del dev "$VETH" root 2>/dev/null || true ;;
esac
```

`timer.sh` — external timer (`local` polls a host path, `remote` polls inside
the container; a directory argument sums all regular files inside it, which
covers clients that write to a temporary name and rename on completion):

```bash
#!/usr/bin/env bash
# timer.sh <local|remote> <path> <expected_bytes> <label>
set -u
where=$1; path=$2; expected=$3; label=$4
size() {
  if [ "$where" = remote ]; then
    docker exec pqssh sh -c "if [ -d '$path' ]; then find '$path' -type f -exec stat -c %s {} + 2>/dev/null | awk '{s+=\$1} END {print s+0}'; else stat -c %s '$path' 2>/dev/null || echo 0; fi"
  else
    if [ -d "$path" ]; then find "$path" -type f -exec stat -c %s {} + 2>/dev/null | awk '{s+=$1} END {print s+0}'; else stat -c %s "$path" 2>/dev/null || echo 0; fi
  fi
}
start=""
while :; do
  s=$(size); now=$(date +%s.%N)
  [ -z "$start" ] && [ "$s" -gt 0 ] && start=$now
  [ "$s" -ge "$expected" ] && { end=$now; break; }
  sleep 0.1
done
xfer=$(echo "$end - $start" | bc -l)
printf '%s\tbytes=%s\txfer_s=%.2f\tMiB_s=%.1f\n' "$label" "$expected" "$xfer" "$(echo "$expected / 1048576 / $xfer" | bc -l)"
```

OpenSSH reference: `sftp -P 2299 demo@127.0.0.1:/home/demo/bench/bench.bin <dst>`
and `echo "put <src> <dst>" | sftp -P 2299 demo@127.0.0.1`, each with the timer
armed on the destination.

## Results (external timer, MiB/s; higher is better)

### ≈ 80 ms RTT (netem 40 ms each way)

| Client | Download | Upload |
|---|---|---|
| OpenSSH `sftp` (reference, 2 runs) | 24.1 / 24.1 | 23.6 / 23.7 |
| Termius 10.0.0 | 17.8 (28.71 s) | 21.1 (24.26 s) |
| Termoso — before tuning (16 × 32 KiB writes, 2 MiB SSH window) | 12.4 (41.31 s) | 5.2 (97.83 s) |
| Termoso — 64 in flight × 256 KiB, 16 MiB window | 30.7 (16.70 s) | 23.3 (21.93 s) |
| **Termoso — this branch** (32 in flight × 256 KiB, 8 MiB window) | **31.8 (16.08 s)** | **23.3 (22.00 s)** |

### LAN (loopback bridge, no added delay)

| Client | Download | Upload |
|---|---|---|
| OpenSSH `sftp` (reference, 2 runs) | 225.4 / 155.5 | 188.2 / 301.3 |
| Termius 10.0.0 | 91.0 (5.63 s; UI showed ≈ 78 MB/s) | 156.6 (3.27 s) |
| Termoso — before tuning | 439.1 (1.17 s) | 347.9 (1.47 s) |
| Termoso — 64 × 256 KiB, 16 MiB window (2 runs) | 273.5 / 276.8 | 212.1 / 229.7 |
| **Termoso — this branch** (32 × 256 KiB, 8 MiB window) | **253.8 (2.02 s)** | **206.4 (2.48 s)** |

All 24 destination files hashed to the source SHA-256. No transfer failed or
had to be retried.

## What changed in Termoso and why

Before tuning, `russh-sftp` defaults were used: writes were 32 KiB packets
with 16 in flight (512 KiB outstanding → ≈ 6 MiB/s at 80 ms RTT, which is
what was measured), and the `russh` per-channel receive window was 2 MiB,
which caps server→client bytes in flight. `Sftp::open` now configures the
session with 256 KiB read/write packets and 32 requests in flight per file
(8 MiB outstanding), and the SSH channel window is 8 MiB
(`crates/termoso-core/src/sftp.rs`, `crates/termoso-core/src/ssh/mod.rs`).
`russh-sftp` honours the server's `limits@openssh.com` reply, so packet
sizes shrink automatically on servers with smaller limits.

## Caveats

* LAN throughput on this box dropped from the pre-tuning run (≈ 440/350 →
  ≈ 250–280/205–230 MiB/s) while WAN throughput rose 2.5–4.5×. The LAN drop
  is consistent across 4 runs and is not explained yet; 32 vs 64 in flight
  made no difference, so it is likely the larger write packets or the
  larger window. It was accepted because real links have latency.
* Loopback LAN numbers are noisy (OpenSSH itself ranged 155–301 MiB/s
  between two runs); treat differences under ~30 % as noise there.
* Termius' UI shows a moving-average rate that is lower than the external
  timer (e.g. 78 MB/s shown vs 91 MiB/s measured on LAN download).
* Single file, single stream. Termoso's transfer queue runs up to 3 jobs in
  parallel; that was not measured and would change multi-file results.
* Emulated delay only — no packet loss, jitter or bandwidth shaping.
* Termius was measured with its default settings; it exposes no SFTP
  concurrency or window knobs that we know of.
