// Offline command dictionary for terminal autocomplete: common Unix / Linux /
// macOS / Windows commands with one-line descriptions. Frequent options and
// subcommands live in `commandFlags.ts`. It is a hint list, not a manual —
// anything the shell knows still works as usual.

export type PathKind = "any" | "dir" | "none";

export interface CommandSpec {
  name: string;
  desc: string;
  paths: PathKind;
}

// `name|description[|d|n]` — `d` = takes directories, `n` = takes no paths.
const ROWS = `
ls|list directory contents
cd|change directory|d
pwd|print working directory|n
mkdir|make directories|d
rmdir|remove empty directories|d
rm|remove files or directories
cp|copy files and directories
mv|move (rename) files
ln|make links between files
touch|create files / update timestamps
cat|concatenate and print files
tac|print files in reverse
less|page through a file
more|page through a file
head|first part of files
tail|last part of files
tree|directory tree|d
file|determine file type
stat|file status
du|disk usage of files
df|file system disk space|n
find|search for files
locate|find files by name|n
fd|fast find alternative
which|locate a command|n
whereis|locate binary, source, manual|n
type|describe a command|n
realpath|resolved absolute path
readlink|resolve symbolic links
basename|strip directory and suffix
dirname|strip last path component
mktemp|temporary file or directory|n
shred|overwrite a file securely
truncate|shrink or extend a file
split|split a file into pieces
install|copy files and set attributes
rename|rename multiple files
chmod|change file mode bits
chown|change file owner and group
chgrp|change group ownership
chattr|change file attributes
lsattr|list file attributes
umask|file mode creation mask|n
getfacl|get file ACLs
setfacl|set file ACLs
sync|flush file system buffers|n
rsync|fast remote and local file copying
scp|secure copy between hosts
sftp|secure file transfer|n
ncdu|NCurses disk usage|d
ranger|console file manager|d
mc|Midnight Commander|d
nnn|terminal file manager|d
exa|modern ls
eza|modern ls
lsd|next-gen ls
bat|cat with syntax highlighting
dust|du alternative
duf|disk usage utility|n
z|jump to a directory (zoxide)|d
zoxide|smarter cd|n
grep|print lines matching a pattern
egrep|grep -E
fgrep|grep -F
rg|ripgrep recursive search
ag|the silver searcher
ack|grep-like source search
sed|stream editor
awk|pattern scanning and processing
gawk|GNU awk
cut|remove sections from lines
paste|merge lines of files
join|join lines on a common field
sort|sort lines
uniq|report or omit repeated lines
wc|count lines, words, bytes
tr|translate or delete characters
rev|reverse lines
fold|wrap lines
fmt|simple text formatter
nl|number lines
column|columnate lists
expand|tabs to spaces
unexpand|spaces to tabs
comm|compare sorted files
diff|compare files line by line
diff3|compare three files
sdiff|side-by-side diff
cmp|compare files byte by byte
patch|apply a diff
colordiff|diff with colour
delta|syntax-highlighting pager for diffs
strings|printable strings in files
od|dump files in octal / hex
xxd|hex dump
hexdump|hex dump
iconv|convert text encoding
dos2unix|convert DOS line endings
unix2dos|convert to DOS line endings
jq|JSON processor
yq|YAML / JSON processor
xmllint|parse and validate XML
xargs|build command lines from stdin
tee|write stdin to stdout and files
printf|format and print data|n
echo|display a line of text|n
seq|print a sequence of numbers|n
shuf|random permutations
yes|output a string repeatedly|n
expr|evaluate expressions|n
bc|arbitrary precision calculator|n
numfmt|human-readable numbers|n
fzf|fuzzy finder
envsubst|substitute environment variables
pandoc|document converter
vi|text editor
vim|Vi IMproved
nvim|Neovim
nano|simple text editor
emacs|the extensible editor
micro|modern terminal editor
hx|Helix editor
code|Visual Studio Code
subl|Sublime Text
ed|line editor
glow|render markdown
ps|process snapshot|n
top|display processes|n
htop|interactive process viewer|n
btop|resource monitor|n
atop|advanced system monitor|n
glances|system monitoring|n
kill|send a signal to a process|n
killall|kill processes by name|n
pkill|signal processes by pattern|n
pgrep|find processes by pattern|n
pidof|PID of a program|n
nice|run with modified priority|n
renice|alter priority|n
nohup|run immune to hangups|n
bg|resume job in background|n
fg|bring job to foreground|n
jobs|list active jobs|n
wait|wait for jobs|n
disown|remove job from shell|n
timeout|run with a time limit|n
watch|run a program periodically|n
time|time a command|n
sleep|delay|n
strace|trace system calls|n
ltrace|trace library calls|n
lsof|list open files|n
fuser|processes using files|n
uptime|system uptime|n
free|memory usage|n
vmstat|virtual memory statistics|n
iostat|CPU and I/O statistics|n
mpstat|processor statistics|n
sar|system activity reporter|n
dstat|resource statistics|n
iotop|I/O by process|n
pidstat|per-process statistics|n
uname|system information|n
hostname|system host name|n
hostnamectl|control hostname|n
lscpu|CPU architecture|n
lsblk|block devices|n
lspci|PCI devices|n
lsusb|USB devices|n
lsmod|loaded kernel modules|n
modprobe|add / remove kernel modules|n
rmmod|remove a kernel module|n
dmesg|kernel ring buffer|n
dmidecode|DMI table decoder|n
inxi|system information|n
neofetch|system info with logo|n
fastfetch|system info with logo|n
arch|machine architecture|n
nproc|number of processing units|n
date|print or set the date|n
cal|calendar|n
timedatectl|system time and date|n
hwclock|hardware clock|n
env|run in a modified environment|n
printenv|print environment|n
export|set environment variable|n
unset|unset variables|n
set|set shell options|n
alias|define aliases|n
unalias|remove aliases|n
source|execute a file in the current shell
exec|replace the shell with a command|n
eval|evaluate arguments as a command|n
exit|exit the shell|n
logout|exit a login shell|n
clear|clear the screen|n
reset|reset the terminal|n
tput|terminal capabilities|n
stty|terminal line settings|n
tty|terminal name|n
script|record a terminal session
history|command history|n
man|manual pages|n
info|info documents|n
help|shell builtin help|n
whatis|one-line manual descriptions|n
apropos|search manual names|n
tldr|simplified man pages|n
shutdown|halt, power off or reboot|n
reboot|reboot the system|n
poweroff|power off|n
halt|halt the system|n
systemctl|control systemd|n
journalctl|systemd journal|n
service|System V init scripts|n
loginctl|systemd login manager|n
systemd-analyze|analyze boot|n
crontab|maintain crontab files
at|schedule a command|n
sysctl|kernel parameters|n
ulimit|user limits|n
swapon|enable swap
swapoff|disable swap
mount|mount a filesystem
umount|unmount a filesystem
findmnt|find a filesystem|n
blkid|block device attributes|n
fdisk|partition table
parted|partition manipulation
mkfs|build a filesystem
fsck|check a filesystem
tune2fs|adjust ext filesystem
resize2fs|resize ext filesystem
lvs|LVM logical volumes|n
vgs|LVM volume groups|n
pvs|LVM physical volumes|n
lvcreate|create a logical volume|n
lvextend|extend a logical volume
zfs|ZFS filesystem|n
zpool|ZFS pool|n
btrfs|btrfs tool
dd|convert and copy a file|n
hdparm|disk parameters
smartctl|SMART disk health
udevadm|udev management|n
losetup|loop devices
cryptsetup|LUKS encryption
mdadm|software RAID
sudo|run as another user|n
su|switch user|n
doas|run as another user|n
whoami|effective user|n
who|who is logged on|n
w|who is logged on and what they do|n
id|user and group ids|n
groups|print groups|n
last|last logged in users|n
lastlog|recent logins|n
users|logged-in users|n
passwd|change password|n
chpasswd|batch password update|n
useradd|create a user|n
adduser|add a user (interactive)|n
usermod|modify a user|n
userdel|delete a user|n
deluser|remove a user|n
groupadd|create a group|n
groupmod|modify a group|n
groupdel|delete a group|n
gpasswd|administer groups|n
newgrp|log in to a new group|n
visudo|edit sudoers|n
chsh|change login shell|n
chfn|change user information|n
getent|entries from databases|n
ssh|OpenSSH client|n
ssh-keygen|SSH key generation|n
ssh-copy-id|install a public key on a server|n
ssh-add|add keys to the agent|n
ssh-agent|authentication agent|n
sshd|OpenSSH daemon|n
mosh|mobile shell|n
telnet|TELNET client|n
nc|netcat|n
ncat|Nmap netcat|n
socat|multipurpose relay|n
curl|transfer a URL|n
wget|network downloader|n
aria2c|download utility|n
http|HTTPie client|n
ping|ICMP echo|n
ping6|IPv6 ping|n
traceroute|route packets take|n
tracepath|trace path|n
mtr|network diagnostic|n
ip|routing, devices, addresses|n
ifconfig|network interface|n
iwconfig|wireless interface|n
iw|wireless devices|n
nmcli|NetworkManager CLI|n
nmtui|NetworkManager TUI|n
route|IP routing table|n
arp|ARP cache|n
netstat|network connections|n
ss|socket statistics|n
dig|DNS lookup|n
nslookup|query DNS|n
host|DNS lookup|n
whois|WHOIS client|n
resolvectl|resolve names|n
ethtool|network driver settings|n
tcpdump|dump network traffic|n
tshark|Wireshark CLI|n
nmap|network scanner|n
iptables|IPv4 packet filter|n
ip6tables|IPv6 packet filter|n
nft|nftables|n
ufw|uncomplicated firewall|n
firewall-cmd|firewalld CLI|n
fail2ban-client|fail2ban control|n
wg|WireGuard|n
wg-quick|WireGuard quick setup|n
openvpn|OpenVPN
iperf3|bandwidth test|n
speedtest|speedtest.net CLI|n
iftop|bandwidth usage|n
nethogs|net top by process|n
nload|network load|n
vnstat|traffic monitor|n
tc|traffic control|n
ftp|file transfer program|n
lftp|sophisticated file transfer|n
rclone|sync to cloud storage
smbclient|SMB / CIFS client|n
showmount|show NFS mounts|n
openssl|OpenSSL toolkit|n
certbot|Let's Encrypt client|n
mkcert|local certificates|n
gpg|GnuPG encryption|n
age|simple encryption
sshuttle|VPN over SSH|n
autossh|auto-restart SSH|n
tar|archive utility
gzip|compress files
gunzip|expand gzip files
zcat|cat gzip files
bzip2|bzip2 compressor
bunzip2|expand bzip2 files
xz|LZMA compression
unxz|expand xz files
zstd|Zstandard compression
zip|package and compress
unzip|extract zip archives
7z|7-Zip archiver
unrar|extract RAR archives
cpio|copy to / from archives
pigz|parallel gzip
zgrep|grep compressed files
apt|APT package manager|n
apt-get|APT package handling|n
apt-cache|query APT cache|n
dpkg|Debian package manager
aptitude|APT frontend|n
snap|snap packages|n
flatpak|Flatpak applications|n
yum|YUM package manager|n
dnf|DNF package manager|n
rpm|RPM package manager
zypper|openSUSE package manager|n
pacman|Arch package manager
yay|AUR helper|n
paru|AUR helper|n
makepkg|build Arch packages|n
apk|Alpine package keeper|n
emerge|Portage package manager|n
nix|Nix package manager|n
nix-shell|Nix shell|n
brew|Homebrew|n
pkg|FreeBSD packages|n
xbps-install|Void packages|n
opkg|OpenWrt packages|n
winget|Windows Package Manager|n
choco|Chocolatey|n
scoop|Scoop|n
pip|Python package installer|n
pip3|Python 3 package installer|n
pipx|isolated Python apps|n
uv|fast Python package manager|n
poetry|Python dependency management|n
conda|Conda package manager|n
npm|Node package manager|n
npx|run npm package binaries|n
yarn|Yarn package manager|n
pnpm|fast Node package manager|n
bun|Bun runtime|n
deno|Deno runtime|n
nvm|Node version manager|n
fnm|fast Node manager|n
cargo|Rust package manager|n
rustup|Rust toolchain installer|n
rustc|Rust compiler
gem|RubyGems|n
bundle|Bundler|n
composer|PHP dependency manager|n
mvn|Apache Maven|n
gradle|Gradle build tool|n
go|Go tool|n
dotnet|.NET CLI|n
mix|Elixir build tool|n
asdf|runtime version manager|n
mise|dev tools, env vars, tasks|n
pyenv|Python version manager|n
rbenv|Ruby version manager|n
git|distributed version control|n
gh|GitHub CLI|n
glab|GitLab CLI|n
tig|text-mode interface for git|n
lazygit|TUI for git|n
svn|Subversion|n
hg|Mercurial|n
make|GNU make|n
cmake|cross-platform make
ninja|small build system|n
meson|build system|n
gcc|GNU C compiler
g++|GNU C++ compiler
cc|C compiler
clang|Clang C compiler
clang++|Clang C++ compiler
ld|the GNU linker
nm|symbols from object files
objdump|object file information
readelf|ELF information
ldd|shared library dependencies
strip|discard symbols
gdb|GNU debugger
lldb|LLVM debugger
valgrind|memory debugging
perf|performance analysis|n
python|Python interpreter
python3|Python 3 interpreter
ipython|interactive Python|n
node|Node.js runtime
ruby|Ruby interpreter
irb|interactive Ruby|n
perl|Perl interpreter
php|PHP interpreter
lua|Lua interpreter
java|Java launcher
javac|Java compiler
kotlin|Kotlin
scala|Scala
sbt|Scala build tool|n
swift|Swift
zig|Zig toolchain|n
ghc|Glasgow Haskell Compiler
elixir|Elixir
erl|Erlang|n
julia|Julia
R|R interpreter
tsc|TypeScript compiler
tsx|TypeScript execute
eslint|JavaScript linter
prettier|code formatter
vite|frontend tooling|n
webpack|module bundler|n
esbuild|JavaScript bundler
jest|JavaScript testing|n
vitest|Vite-native testing|n
pytest|Python testing|n
tox|Python test automation|n
black|Python formatter
ruff|Python linter
mypy|Python type checker
shellcheck|shell script analysis
shfmt|shell formatter
docker|Docker containers|n
docker-compose|Compose multi-container apps|n
podman|pod manager|n
buildah|build OCI images|n
skopeo|container images|n
nerdctl|containerd CLI|n
crictl|CRI CLI|n
kubectl|Kubernetes CLI|n
k9s|Kubernetes TUI|n
helm|Kubernetes package manager|n
minikube|local Kubernetes|n
kind|Kubernetes in Docker|n
k3s|lightweight Kubernetes|n
kustomize|Kubernetes configuration|n
stern|multi-pod log tailing|n
terraform|infrastructure as code|n
tofu|OpenTofu|n
pulumi|infrastructure as code|n
ansible|IT automation|n
ansible-playbook|run Ansible playbooks
ansible-vault|encrypt Ansible files
vagrant|VM environments|n
packer|build machine images|n
aws|AWS CLI|n
az|Azure CLI|n
gcloud|Google Cloud CLI|n
gsutil|Google Cloud Storage|n
doctl|DigitalOcean CLI|n
flyctl|Fly.io CLI|n
heroku|Heroku CLI|n
vercel|Vercel CLI|n
wrangler|Cloudflare Workers CLI|n
s3cmd|S3 client|n
vault|HashiCorp Vault|n
consul|HashiCorp Consul|n
nomad|HashiCorp Nomad|n
etcdctl|etcd client|n
psql|PostgreSQL client|n
pg_dump|PostgreSQL backup|n
pg_restore|PostgreSQL restore
createdb|create a PostgreSQL database|n
dropdb|drop a PostgreSQL database|n
mysql|MySQL client|n
mysqldump|MySQL backup|n
mariadb|MariaDB client|n
sqlite3|SQLite shell
redis-cli|Redis client|n
mongosh|MongoDB shell|n
clickhouse-client|ClickHouse client|n
nginx|HTTP and reverse proxy server|n
apachectl|Apache HTTP server control|n
caddy|web server|n
haproxy|load balancer|n
pm2|Node process manager|n
supervisorctl|supervisor control|n
tmux|terminal multiplexer|n
screen|terminal multiplexer|n
zellij|terminal workspace|n
byobu|tmux / screen wrapper|n
direnv|per-directory environments|n
just|command runner|n
task|Taskfile runner|n
entr|run commands when files change
watchexec|run on file change|n
hyperfine|benchmarking|n
ab|Apache benchmark|n
wrk|HTTP benchmarking|n
ngrok|secure tunnels|n
cloudflared|Cloudflare tunnel|n
hugo|static site generator|n
protoc|Protocol Buffers compiler
grpcurl|gRPC curl|n
websocat|WebSocket client|n
sqlx|SQLx CLI|n
prisma|Prisma ORM|n
alembic|SQLAlchemy migrations|n
bash|GNU Bourne-Again SHell
sh|POSIX shell
zsh|Z shell
fish|friendly interactive shell
dash|Debian Almquist shell
ksh|KornShell
tcsh|TENEX C shell
nu|Nushell
pwsh|PowerShell
powershell|Windows PowerShell
test|evaluate a conditional expression|n
true|do nothing, successfully|n
false|do nothing, unsuccessfully|n
read|read a line from stdin|n
shift|shift positional parameters|n
getopts|parse option arguments|n
trap|trap signals|n
return|return from a function|n
break|exit a loop|n
continue|continue a loop|n
let|arithmetic evaluation|n
declare|declare variables|n
local|local variables|n
readonly|read-only variables|n
command|run bypassing functions|n
builtin|run a shell builtin|n
hash|remember command locations|n
dirs|directory stack|n
pushd|push directory|d
popd|pop directory|n
compgen|generate completions|n
complete|specify completions|n
bind|readline bindings|n
shopt|shell options|n
mapfile|read lines into an array|n
xdg-open|open a file or URL
open|open a file (macOS)
pbcopy|copy to clipboard (macOS)|n
pbpaste|paste from clipboard (macOS)|n
xclip|X clipboard|n
xsel|X selection|n
wl-copy|Wayland clipboard copy|n
wl-paste|Wayland clipboard paste|n
notify-send|desktop notification|n
figlet|large ASCII text|n
cowsay|talking cow|n
fortune|random adage|n
lolcat|rainbow output|n
cmatrix|matrix effect|n
asciinema|record terminal sessions|n
ffmpeg|audio / video converter
ffprobe|media information
convert|ImageMagick convert
magick|ImageMagick
identify|image information
exiftool|read / write metadata
yt-dlp|download videos|n
mpv|media player
sox|sound processing
qrencode|QR code generator|n
base64|base64 encode / decode
base32|base32 encode / decode
md5sum|MD5 checksums
sha1sum|SHA-1 checksums
sha256sum|SHA-256 checksums
sha512sum|SHA-512 checksums
b2sum|BLAKE2 checksums
cksum|CRC checksums
uuidgen|generate a UUID|n
pwgen|generate passwords|n
pass|password store|n
sops|secrets editor
units|unit conversion|n
factor|factor numbers|n
wall|write to all users|n
write|write to another user|n
mail|send and receive mail|n
mutt|mail client|n
ldapsearch|LDAP search|n
realm|join AD / IPA realm|n
kinit|obtain Kerberos ticket|n
klist|list Kerberos tickets|n
logger|write to syslog|n
logrotate|rotate log files
auditctl|audit control|n
ausearch|search audit logs|n
aa-status|AppArmor status|n
getenforce|SELinux mode|n
setenforce|set SELinux mode|n
sestatus|SELinux status|n
chcon|change SELinux context
restorecon|restore SELinux context
semanage|SELinux policy management|n
lynis|security auditing|n
rkhunter|rootkit hunter|n
clamscan|ClamAV scanner
virsh|libvirt management|n
virt-install|create VMs|n
qemu-img|QEMU disk images
lxc|LXD / LXC containers|n
incus|Incus containers & VMs|n
multipass|Ubuntu VMs|n
wsl|Windows Subsystem for Linux|n
ipmitool|IPMI management|n
sensors|hardware sensors|n
nvidia-smi|NVIDIA GPU status|n
powertop|power usage|n
acpi|battery and ACPI info|n
xrandr|screen configuration|n
localectl|locale settings|n
locale|locale information|n
dpkg-reconfigure|reconfigure a package|n
update-alternatives|manage default symlinks|n
ldconfig|configure dynamic linker|n
getconf|configuration values|n
chroot|run with a different root|d
unshare|run in new namespaces|n
nsenter|enter namespaces|n
setsid|run in a new session|n
taskset|CPU affinity|n
ionice|I/O scheduling class|n
systemd-run|run as a transient unit|n
busctl|D-Bus introspection|n
flock|manage file locks
inotifywait|wait for file changes
pv|pipe viewer|n
parallel|GNU parallel|n
sponge|soak up stdin, write to a file
ts|timestamp input|n
vidir|edit directory in editor|d
chronic|run quietly unless it fails|n
tzselect|select a timezone|n
`;

function parse(): CommandSpec[] {
  const out: CommandSpec[] = [];
  for (const line of ROWS.split("\n")) {
    if (!line) continue;
    const [name = "", desc = "", kind] = line.split("|");
    if (!name) continue;
    out.push({
      name,
      desc,
      paths: kind === "d" ? "dir" : kind === "n" ? "none" : "any",
    });
  }
  return out;
}

export const COMMANDS: readonly CommandSpec[] = parse();

const BY_NAME = new Map(COMMANDS.map((c) => [c.name, c]));

export const commandSpec = (name: string) => BY_NAME.get(name);

/** Commands that take another command as their argument (`sudo ls`, `nohup x`). */
export const WRAPPERS = new Set([
  "sudo",
  "doas",
  "nohup",
  "time",
  "timeout",
  "nice",
  "ionice",
  "watch",
  "exec",
  "command",
  "builtin",
  "env",
  "xargs",
  "strace",
  "ltrace",
  "chronic",
  "setsid",
  "unshare",
  "nsenter",
  "taskset",
  "chroot",
  "hyperfine",
]);
