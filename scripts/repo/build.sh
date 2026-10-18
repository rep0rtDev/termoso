#!/usr/bin/env bash
# Rebuilds the APT + DNF repositories served from termoso/termoso-repo
# (GitHub Pages) out of the .deb/.rpm bundles of one release.
#
#   build.sh <assets dir> <repo checkout> <version> <gpg key id>
#
# The repositories are signed with the key given (expected in the current
# GNUPGHOME, no passphrase). Only the two newest versions are kept in the
# pools so the Pages repository stays small; older builds remain on the
# GitHub release page.
set -euo pipefail

assets=$1 out=$2 version=$3 key=$4
keep=2
here=$(cd "$(dirname "$0")" && pwd)

prune() { # <dir> <glob>: drop every version but the $keep newest
  local dir=$1 glob=$2
  ls "$dir"/$glob 2>/dev/null \
    | sed -E 's/.*[_-]([0-9]+\.[0-9]+\.[0-9]+[^_-]*)[_-].*/\1 &/' \
    | sort -V | awk -v k="$keep" '{ v[NR]=$1; f[NR]=$2 }
        END { n=0; for (i=NR; i>=1; i--) { if (!(v[i] in seen)) { seen[v[i]]=1; n++ } if (n>k) print f[i] } }' \
    | xargs -r rm -v --
}

### APT ###################################################################
deb=$out/deb
mkdir -p "$deb/pool/main/t/termoso" "$deb/dists/stable/main"
for f in "$assets"/*_amd64.deb "$assets"/*_arm64.deb; do
  [ -e "$f" ] || { echo "missing $f" >&2; exit 1; }
  cp -f "$f" "$deb/pool/main/t/termoso/$(basename "$f" | tr 'A-Z' 'a-z')"
done
prune "$deb/pool/main/t/termoso" '*.deb'
(
  cd "$deb"
  for arch in amd64 arm64; do
    d=dists/stable/main/binary-$arch
    mkdir -p "$d"
    apt-ftparchive --arch "$arch" packages pool > "$d/Packages"
    gzip -9 -n -k -f "$d/Packages"
  done
  cat > /tmp/apt-release.conf <<CONF
APT::FTPArchive::Release {
  Origin "Termoso";
  Label "Termoso";
  Suite "stable";
  Codename "stable";
  Architectures "amd64 arm64";
  Components "main";
  Description "Termoso — SSH, SFTP and terminal client (https://termoso.dev)";
};
CONF
  apt-ftparchive -c /tmp/apt-release.conf release dists/stable > dists/stable/Release
  rm -f dists/stable/InRelease dists/stable/Release.gpg
  gpg --batch --yes --local-user "$key" --digest-algo SHA256 --clearsign -o dists/stable/InRelease dists/stable/Release
  gpg --batch --yes --local-user "$key" --digest-algo SHA256 -abs -o dists/stable/Release.gpg dists/stable/Release
)
echo "deb [signed-by=/usr/share/keyrings/termoso.gpg] https://pkg.termoso.dev/deb stable main" > "$out/termoso.list"

### DNF ###################################################################
rpm=$out/rpm
for pair in x86_64:x86_64 aarch64:aarch64; do
  arch=${pair%%:*}
  mkdir -p "$rpm/$arch"
  src=$(ls "$assets"/*."$arch".rpm 2>/dev/null | head -1)
  [ -n "$src" ] || { echo "missing .$arch.rpm" >&2; exit 1; }
  dst="$rpm/$arch/$(basename "$src" | tr 'A-Z' 'a-z')"
  cp -f "$src" "$dst"
  rpmsign --define "__gpg /usr/bin/gpg" --define "_gpg_name $key" --define "_gpg_digest_algo sha256" --addsign "$dst" > /dev/null
  prune "$rpm/$arch" '*.rpm'
  createrepo_c --update --quiet "$rpm/$arch"
  rm -f "$rpm/$arch/repodata/repomd.xml.asc"
  gpg --batch --yes --local-user "$key" --digest-algo SHA256 -abs -o "$rpm/$arch/repodata/repomd.xml.asc" "$rpm/$arch/repodata/repomd.xml"
done
cat > "$out/termoso.repo" <<REPO
[termoso]
name=Termoso
baseurl=https://pkg.termoso.dev/rpm/\$basearch
enabled=1
gpgcheck=1
repo_gpgcheck=1
gpgkey=https://pkg.termoso.dev/termoso.asc
REPO

### keys + landing ########################################################
gpg --batch --yes --export --armor "$key" > "$out/termoso.asc"
gpg --batch --yes --export "$key" > "$out/termoso.gpg"
sed "s/@VERSION@/$version/g; s/@FINGERPRINT@/$(gpg --batch --with-colons --fingerprint "$key" | awk -F: '$1=="fpr"{print $10; exit}')/" \
  "$here/index.html" > "$out/index.html"
touch "$out/.nojekyll"
echo "$version" > "$out/VERSION"
echo "repositories rebuilt for $version:"
(cd "$out" && find deb/pool rpm -name '*.deb' -o -name '*.rpm' | sort)
