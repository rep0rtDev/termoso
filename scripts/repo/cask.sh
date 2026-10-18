#!/usr/bin/env bash
# Renders the Homebrew cask for termoso/homebrew-termoso from a release's
# SHA256SUMS.txt:  cask.sh <SHA256SUMS.txt> <version> > Casks/termoso.rb
set -euo pipefail
sums=$1 version=$2
sha() { awk -v f="$1" '$2 == f || $2 == "*" f {print $1}' "$sums" | head -1; }
arm=$(sha "Termoso_${version}_aarch64.dmg"); x64=$(sha "Termoso_${version}_x64.dmg")
[ -n "$arm" ] && [ -n "$x64" ] || { echo "dmg checksums missing in $sums" >&2; exit 1; }
cat <<RUBY
cask "termoso" do
  arch arm: "aarch64", intel: "x64"

  version "$version"
  sha256 arm:   "$arm",
         intel: "$x64"

  url "https://github.com/termoso/termoso/releases/download/v#{version}/Termoso_#{version}_#{arch}.dmg",
      verified: "github.com/termoso/termoso/"
  name "Termoso"
  desc "SSH, SFTP and terminal client with end-to-end encrypted sync"
  homepage "https://termoso.dev/"

  livecheck do
    url :url
    strategy :github_latest
  end

  auto_updates true
  depends_on macos: ">= :big_sur"

  app "Termoso.app"

  zap trash: [
    "~/Library/Application Support/com.termoso.desktop",
    "~/Library/Caches/com.termoso.desktop",
    "~/Library/Preferences/com.termoso.desktop.plist",
    "~/Library/Saved Application State/com.termoso.desktop.savedState",
    "~/Library/WebKit/com.termoso.desktop",
  ]
end
RUBY
