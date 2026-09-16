#!/usr/bin/env python3
"""Render the AltStore/SideStore source for one release.

    render.py --template source.template.json --repo owner/name --tag v0.2.0 \
              --version 0.2.0 --ipa termoso-0.2.0-ios.ipa --out termoso-altstore.json

Fills version, download URL, size and sha256 of the .ipa. No credentials are
involved: the feed only points at the unsigned .ipa on GitHub Releases, signing
happens on the user's device.
"""

import argparse
import datetime as dt
import hashlib
import json
import os
import plistlib
import sys
import zipfile


def ipa_build_version(path: str) -> str:
    with zipfile.ZipFile(path) as z:
        for name in z.namelist():
            parts = name.split("/")
            if len(parts) == 3 and parts[0] == "Payload" and parts[1].endswith(".app") and parts[2] == "Info.plist":
                info = plistlib.loads(z.read(name))
                return str(info.get("CFBundleVersion", "1"))
    return "1"


def fill(value, subs):
    if isinstance(value, str):
        for k, v in subs.items():
            value = value.replace("{" + k + "}", str(v))
        return value
    if isinstance(value, list):
        return [fill(v, subs) for v in value]
    if isinstance(value, dict):
        return {k: fill(v, subs) for k, v in value.items()}
    return value


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--template", required=True)
    ap.add_argument("--repo", required=True, help="owner/name")
    ap.add_argument("--tag", required=True)
    ap.add_argument("--version", required=True)
    ap.add_argument("--ipa", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--date", default=dt.datetime.now(dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"))
    args = ap.parse_args()

    with open(args.ipa, "rb") as f:
        digest = hashlib.sha256(f.read()).hexdigest()

    subs = {
        "repo": args.repo,
        "tag": args.tag,
        "version": args.version,
        "build": ipa_build_version(args.ipa),
        "date": args.date,
        "ipa_name": os.path.basename(args.ipa),
        "size": os.path.getsize(args.ipa),
        "sha256": digest,
    }
    with open(args.template, encoding="utf-8") as f:
        source = fill(json.load(f), subs)

    for app in source["apps"]:
        for version in app["versions"]:
            version["size"] = int(version["size"])

    leftovers = [s for s in json.dumps(source).split("{")[1:] if s.split("}")[0] in subs]
    if leftovers:
        print(f"unfilled placeholders: {leftovers}", file=sys.stderr)
        return 1

    with open(args.out, "w", encoding="utf-8") as f:
        json.dump(source, f, indent=2, ensure_ascii=False)
        f.write("\n")
    print(f"{args.out}: {subs['ipa_name']} {subs['size']} bytes sha256={digest[:16]}…")
    return 0


if __name__ == "__main__":
    sys.exit(main())
