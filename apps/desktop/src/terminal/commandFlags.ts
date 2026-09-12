// Frequent flags and first-level subcommands of frequently used commands, for
// the terminal autocomplete. Format: `flag description;flag description;…`.

export interface Flag {
  name: string;
  desc: string;
}

const OPTIONS: Record<string, string> = {
  ls: "-l long format;-a all incl. hidden;-h human sizes;-t sort by time;-r reverse;-S sort by size;-R recursive;-d directories themselves;-1 one per line;--color colourize",
  cp: "-r recursive;-a archive;-v verbose;-i prompt before overwrite;-n no clobber;-u update only;-p preserve attributes",
  mv: "-i prompt before overwrite;-f force;-n no clobber;-v verbose;-u update only",
  rm: "-r recursive;-f force;-i prompt each;-v verbose;-d remove empty dirs",
  mkdir: "-p make parents;-v verbose;-m set mode",
  chmod: "-R recursive;-v verbose;-c report changes;--reference copy mode from file",
  chown: "-R recursive;-v verbose;-h affect symlinks;--reference copy owner from file",
  ln: "-s symbolic link;-f force;-n no dereference;-v verbose;-r relative symlink",
  cat: "-n number lines;-A show all;-b number non-blank;-s squeeze blank;-E show line ends",
  head: "-n lines;-c bytes;-q no headers",
  tail: "-f follow;-F follow by name, retry;-n lines;-c bytes;--pid stop when PID dies",
  less: "-N line numbers;-S chop long lines;-R raw control chars;-i ignore case;+F follow;-X no init",
  grep: "-r recursive;-i ignore case;-n line numbers;-v invert match;-l files with matches;-c count;-E extended regex;-F fixed strings;-w whole words;-o only matching;-A lines after;-B lines before;-C context;--include file glob;--exclude-dir skip dirs;--color colourize",
  rg: "-i ignore case;-n line numbers;-l files with matches;-v invert;-w whole words;-F fixed strings;-t file type;-g glob;--hidden search hidden;-A after;-B before;-C context;-S smart case;--no-ignore ignore .gitignore",
  find: "-name name pattern;-iname case-insensitive name;-type f / d / l;-size size filter;-mtime modified days;-newer newer than file;-exec execute command;-delete delete matches;-maxdepth depth limit;-empty empty files;-user owner;-perm permissions",
  sed: "-i in-place;-e expression;-n quiet;-E extended regex;-s separate files",
  awk: "-F field separator;-v assign variable;-f program file",
  sort: "-n numeric;-r reverse;-u unique;-k key;-t separator;-h human numeric;-f ignore case;-V version sort;-o output file",
  uniq: "-c count;-d duplicates only;-u unique only;-i ignore case",
  wc: "-l lines;-w words;-c bytes;-m chars;-L longest line",
  cut: "-d delimiter;-f fields;-c characters;-b bytes;--complement invert",
  tr: "-d delete;-s squeeze;-c complement",
  diff: "-u unified;-r recursive;-q brief;-w ignore whitespace;-i ignore case;-y side by side;--color colourize",
  du: "-h human-readable;-s summarize;-a all files;-d max depth;-c grand total;--exclude skip pattern;-x one filesystem",
  df: "-h human-readable;-T filesystem type;-i inodes;-a all;-x exclude type",
  tar: "-x extract;-c create;-t list;-v verbose;-f archive file;-z gzip;-j bzip2;-J xz;-C change dir;--exclude skip pattern;-p preserve permissions;--strip-components strip path parts",
  zip: "-r recursive;-q quiet;-e encrypt;-9 best compression;-x exclude",
  unzip: "-l list;-d destination;-o overwrite;-q quiet;-t test",
  gzip: "-d decompress;-k keep;-9 best;-r recursive;-c stdout",
  xz: "-d decompress;-k keep;-9 best;-T threads;-z compress",
  zstd: "-d decompress;-k keep;-19 high compression;-T0 all threads;--rm remove input",
  ps: "aux all processes (BSD);-ef all processes (full);-u by user;-p by PID;-o output format;--sort sort by;-C by command;--forest tree",
  kill: "-9 SIGKILL;-15 SIGTERM;-HUP SIGHUP;-INT SIGINT;-STOP SIGSTOP;-CONT SIGCONT;-l list signals",
  pkill: "-f match full command line;-9 SIGKILL;-u user;-x exact match;-e echo killed",
  pgrep:
    "-f match full command line;-l list name;-a list full;-u user;-x exact;-n newest;-o oldest",
  top: "-u user;-p PID;-d delay;-n iterations;-b batch mode;-o sort field",
  htop: "-u user;-p PIDs;-d delay;-t tree view;-s sort column",
  free: "-h human-readable;-m megabytes;-g gigabytes;-s repeat every N sec;-t total",
  ssh: "-p port;-i identity file;-L local forward;-R remote forward;-D dynamic (SOCKS) forward;-N no command;-f background;-v verbose;-J jump host;-A agent forwarding;-X X11 forwarding;-t force tty;-o option;-C compression;-q quiet;-l login name;-F config file",
  "ssh-keygen":
    "-t type (ed25519, rsa, ecdsa);-b bits;-f output file;-C comment;-N passphrase;-p change passphrase;-l show fingerprint;-y print public key;-R remove host from known_hosts;-a KDF rounds",
  "ssh-copy-id": "-i identity file;-p port;-f force;-n dry run",
  "ssh-add":
    "-l list fingerprints;-L list public keys;-d delete key;-D delete all;-t lifetime;-x lock;-X unlock;-c confirm",
  scp: "-r recursive;-P port;-i identity file;-p preserve;-C compression;-q quiet;-v verbose;-l limit bandwidth;-J jump host;-o option",
  rsync:
    "-a archive;-v verbose;-z compress;-P progress + partial;-r recursive;-n dry run;--delete delete extraneous;--exclude skip pattern;-e remote shell;-h human-readable;--progress show progress;-u update;-l links;--bwlimit bandwidth limit;-c checksum",
  curl: "-o output file;-O remote name;-L follow redirects;-s silent;-S show errors;-I headers only;-i include headers;-X request method;-H header;-d data;--data-binary binary data;-F form;-u user:password;-k insecure;-v verbose;-w write out;--json JSON body;-x proxy;--retry retries;-A user agent;-b cookie;-c cookie jar;--compressed accept compressed;-m max time;-# progress bar",
  wget: "-O output file;-P directory prefix;-c continue;-q quiet;-r recursive;-np no parent;-nd no directories;-m mirror;--no-check-certificate skip TLS check;-b background;-N timestamping;--limit-rate bandwidth;-U user agent",
  ping: "-c count;-i interval;-W timeout;-s packet size;-4 IPv4;-6 IPv6;-q quiet;-D timestamps",
  ip: "-br brief output;-c colour;-4 IPv4;-6 IPv6;-s statistics",
  ss: "-t TCP;-u UDP;-l listening;-n numeric;-p processes;-a all;-s summary;-4 IPv4;-6 IPv6;-r resolve;-e extended",
  netstat:
    "-t TCP;-u UDP;-l listening;-n numeric;-p programs;-a all;-r routes;-i interfaces;-s statistics",
  dig: "+short short answer;+trace trace delegation;-x reverse lookup;+noall no output sections;+answer answer section;-t record type;+dnssec DNSSEC",
  nmap: "-sS SYN scan;-sT connect scan;-sU UDP scan;-sV version detection;-O OS detection;-p ports;-A aggressive;-Pn skip host discovery;-T4 timing;-oN normal output;-oX XML output;-sn ping scan;-v verbose;--script NSE scripts",
  tcpdump:
    "-i interface;-n numeric;-nn numeric ports;-w write pcap;-r read pcap;-c count;-v verbose;-X hex+ascii;-A ascii;-s snaplen",
  nc: "-l listen;-p port;-v verbose;-z scan only;-u UDP;-w timeout;-k keep listening;-N shutdown on EOF;-q quit after EOF",
  systemctl:
    "--user user manager;--now start/stop as well;-a all units;--failed failed units;-t unit type;--no-pager no pager;-l full output;--state filter state",
  journalctl:
    "-u unit;-f follow;-n lines;-b this boot;-e jump to end;-r reverse;-p priority;--since since time;--until until time;-k kernel;-x explanations;--no-pager no pager;-o output format;--disk-usage disk usage;--vacuum-time remove older;-g grep pattern",
  docker: "-H daemon socket;--context context;-D debug",
  kubectl:
    "-n namespace;-A all namespaces;-o output (yaml, json, wide);-f file;-l label selector;--context context;-w watch;--dry-run client / server;-it interactive tty;--all all resources;--force force;-c container;--kubeconfig kubeconfig;-R recursive;-k kustomize dir;--show-labels show labels;--sort-by sort",
  git: "-C run in directory;-c config value;--no-pager no pager;--version version",
  make: "-j parallel jobs;-f makefile;-C directory;-n dry run;-B always make;-k keep going;-s silent",
  apt: "-y assume yes;-q quiet;--fix-broken fix dependencies;--no-install-recommends skip recommends;-t target release;--reinstall reinstall;--purge remove config too;-s simulate",
  "apt-get":
    "-y assume yes;-q quiet;-f fix broken;--no-install-recommends skip recommends;-s simulate;--purge purge;-d download only",
  dpkg: "-i install;-r remove;-P purge;-l list;-L list files;-S search file;-s status;--configure configure;-x extract;-c list contents",
  dnf: "-y assume yes;-q quiet;--enablerepo enable repo;--disablerepo disable repo;--refresh refresh metadata;--best best versions;--allowerasing allow erasing",
  yum: "-y assume yes;-q quiet;--enablerepo enable repo;--disablerepo disable repo;--nogpgcheck skip GPG",
  pacman:
    "-S sync install;-Syu full upgrade;-Ss search;-Si package info;-R remove;-Rns remove with deps and config;-Q query installed;-Qs search installed;-Qi installed info;-Ql list files;-Qo owner of file;-Qdt orphans;-U install file;-Sc clean cache;--noconfirm no confirm;-F file database",
  apk: "--no-cache no cache;-U update index;-u upgrade;-v verbose;-q quiet;--virtual virtual package;-i interactive",
  brew: "--cask casks;--force force;--verbose verbose;--HEAD HEAD version;--dry-run dry run;--greedy upgrade auto-updating casks",
  npm: "-g global;--save-dev dev dependency;-D dev dependency;--legacy-peer-deps ignore peer deps;--force force;--production no dev deps;-w workspace;--dry-run dry run",
  pnpm: "-g global;-D dev dependency;-w workspace root;-r recursive;--filter filter packages;--frozen-lockfile frozen lockfile;--prod production",
  yarn: "-D dev dependency;-W workspace root;--frozen-lockfile frozen lockfile;--production production;--immutable immutable",
  cargo:
    "--release optimized build;-p package;--workspace all packages;--all-features all features;--features features;--no-default-features no default features;--target target triple;-j jobs;--offline offline;--locked locked;-q quiet;-v verbose;--bin binary;--example example;--lib library;--tests tests;--all-targets all targets",
  pip: "-r requirements file;-U upgrade;-e editable;--user user site;-q quiet;--no-cache-dir no cache;--index-url index",
  pip3: "-r requirements file;-U upgrade;-e editable;--user user site;-q quiet;--no-cache-dir no cache",
  python:
    "-m run module;-c command;-i interactive;-u unbuffered;-V version;-W warnings;-O optimize;-B no .pyc",
  python3:
    "-m run module;-c command;-i interactive;-u unbuffered;-V version;-W warnings;-B no .pyc",
  node: "-e eval;-p print;-v version;--inspect debugger;-r preload module;--watch watch;--env-file env file;--test run tests;--max-old-space-size heap MB",
  sudo: "-i login shell;-s shell;-u user;-E preserve env;-k invalidate timestamp;-l list privileges;-v validate;-n non-interactive;-b background;-H set HOME",
  su: "- login shell;-l login shell;-c command;-s shell;-m preserve env",
  mount:
    "-t type;-o options;-a all in fstab;-l list;-r read-only;-w read-write;--bind bind mount;-v verbose",
  umount: "-l lazy;-f force;-a all;-v verbose;-R recursive",
  dd: "if= input file;of= output file;bs= block size;count= blocks;status=progress progress;conv= conversions;skip= skip input blocks;seek= skip output blocks",
  tmux: "-s session name;-t target;-d detached;-f config file;-L socket name;-u UTF-8;-2 256 colours",
  screen:
    "-S session name;-r reattach;-ls list;-d detach;-x multi attach;-dmS detached named;-X send command;-wipe wipe dead",
  crontab: "-e edit;-l list;-r remove;-u user;-i confirm removal",
  useradd:
    "-m create home;-s shell;-G supplementary groups;-g primary group;-d home dir;-u UID;-c comment;-r system account;-e expire date",
  usermod:
    "-aG append groups;-G groups;-s shell;-d home;-l new login;-L lock;-U unlock;-e expire;-u UID;-m move home",
  userdel: "-r remove home;-f force",
  passwd: "-l lock;-u unlock;-d delete password;-e expire;-S status;-n min days;-x max days",
  chsh: "-s shell;-l list shells",
  gpg: "--gen-key generate key;--full-generate-key generate key (full);--list-keys list keys;-k list keys;--list-secret-keys secret keys;-K secret keys;--export export;-a ASCII armour;--import import;-e encrypt;-d decrypt;-r recipient;-s sign;--verify verify;-c symmetric;--edit-key edit key;--delete-key delete key;--recv-keys receive keys;--keyserver keyserver",
  ffmpeg:
    "-i input;-c:v video codec;-c:a audio codec;-c codec;-vf video filter;-af audio filter;-b:v video bitrate;-b:a audio bitrate;-r frame rate;-s size;-ss start time;-t duration;-to end time;-y overwrite;-n never overwrite;-an no audio;-vn no video;-crf quality;-preset encoding preset;-f format;-map stream map;-hide_banner hide banner;-loglevel log level",
  jq: "-r raw output;-c compact;-s slurp;-n null input;-e exit status;-S sort keys;--arg string arg;--argjson JSON arg;-C colour;-M monochrome;--tab tab indent;-j join output",
  xargs:
    "-0 null separated;-n max args;-I replace string;-P parallel;-t print commands;-p prompt;-r no run if empty;-d delimiter;-L max lines",
  strace:
    "-p attach to PID;-f follow forks;-e filter (trace=…);-o output file;-s string size;-c summary;-t timestamps;-T syscall time;-y print paths;-k stack trace",
  lsof: "-i network;-p PID;-u user;-n no DNS;-P no port names;-t PIDs only;-c command;+D directory;-a AND",
  watch:
    "-n interval;-d highlight differences;-t no title;-e exit on error;-c colour;-g exit on change;-x exec",
  date: "+%Y-%m-%d ISO date;+%s unix time;+%H:%M:%S time;+%F full date;+%T time;-u UTC;-d parse date;-s set;-R RFC 2822;-I ISO 8601;-r file mtime",
  uname:
    "-a all;-r kernel release;-m machine;-n node name;-s kernel name;-v kernel version;-o operating system;-p processor",
  history: "-c clear;-d delete entry;-w write;-r read;-a append;-n read new lines",
  echo: "-n no newline;-e interpret escapes;-E no escapes",
  shutdown: "-h halt;-r reboot;-c cancel;now immediately;+5 in 5 minutes;-P power off;-k warn only",
  iptables:
    "-L list;-A append;-I insert;-D delete;-F flush;-P policy;-t table;-n numeric;-v verbose;-p protocol;--dport destination port;--sport source port;-s source;-d destination;-j jump target;-i in interface;-o out interface;-m match;-S list rules;--line-numbers line numbers;-N new chain;-X delete chain;-Z zero counters",
  "firewall-cmd":
    "--state state;--reload reload;--list-all list all;--add-port add port;--remove-port remove port;--add-service add service;--remove-service remove service;--permanent permanent;--zone zone;--get-zones zones;--get-active-zones active zones;--list-ports ports;--list-services services;--add-rich-rule rich rule;--runtime-to-permanent save runtime",
  fdisk: "-l list partitions;-u units;-x extra;-o output columns",
  lsblk:
    "-f filesystems;-o columns;-a all;-p full paths;-d no partitions;-J JSON;-m permissions;-t topology",
  fsck: "-y assume yes;-f force;-n no changes;-t type;-A all in fstab;-C progress;-p auto repair",
  mkfs: "-t type;-L label;-F force;-V verbose;-n dry run",
  chattr:
    "+i immutable;-i remove immutable;+a append only;-a remove append;-R recursive;-V verbose",
  sysctl: "-w write;-a all;-p load file;-n values only;-e ignore unknown;--system load all config",
  modprobe:
    "-r remove;-l list;-v verbose;-n dry run;-c config;--show-depends dependencies;-a all;-f force",
  dmesg:
    "-T human timestamps;-w follow;-H human output;-l level;-k kernel;-C clear;-f facility;-x decode;-e reltime;--since since",
  rpm: "-i install;-U upgrade;-e erase;-q query;-qa all installed;-qi info;-ql list files;-qf owner of file;-V verify;-v verbose;-h hashes;--nodeps no dependency check;--force force;-K check signature;--import import key;-qp query package file",
  psql: "-U user;-h host;-p port;-d database;-c command;-f file;-l list databases;-W prompt password;-t tuples only;-A unaligned;-x expanded;-q quiet;-v variable;--csv CSV output;-1 single transaction;-E echo hidden queries",
  pg_dump:
    "-U user;-h host;-p port;-d database;-F format (c, d, t, p);-f file;-t table;-n schema;-s schema only;-a data only;-c clean;-C create;-Z compression;-j jobs;--no-owner no owner;--no-privileges no privileges;-v verbose",
  mysql:
    "-u user;-p password prompt;-h host;-P port;-D database;-e execute;-N no headers;-B batch;-s silent;-v verbose;--default-character-set charset;-S socket;--ssl-mode SSL mode",
  mysqldump:
    "-u user;-p password prompt;-h host;--all-databases all databases;--databases databases;--single-transaction consistent snapshot;--routines routines;--triggers triggers;--no-data schema only;--no-create-info data only;--quick quick;-r result file",
  "redis-cli":
    "-h host;-p port;-n database;--scan scan keys;--pattern pattern;-r repeat;-i interval;--stat stats;--bigkeys big keys;--latency latency;--pipe pipe mode;-u URI;--tls TLS",
  sqlite3:
    "-header headers;-column column mode;-csv CSV;-json JSON;-line line mode;-readonly read only;-cmd run command;-bail stop on error;-batch batch;-echo echo;-init init file;-table table mode;-box box mode;-markdown markdown",
  nginx:
    "-t test config;-T test and dump;-s signal (reload, stop, quit, reopen);-c config file;-p prefix;-g global directives;-v version;-V version and configure;-q quiet",
  certbot:
    "--nginx nginx plugin;--apache apache plugin;--standalone standalone;--webroot webroot;-w webroot path;-d domain;--dry-run dry run;--email email;--agree-tos agree TOS;-n non-interactive;--force-renewal force;--cert-name name;--preferred-challenges challenges;--manual manual",
  "docker-compose":
    "-f compose file;-d detached;-p project name;--build build first;--force-recreate recreate;-v remove volumes",
  helm: "-n namespace;-f values file;--set set value;--create-namespace create namespace;--dry-run dry run;--wait wait;-i install if missing;--atomic atomic;--version chart version",
  terraform:
    "-auto-approve auto approve;-var variable;-var-file var file;-target target;-out plan file;-refresh=false no refresh;-upgrade upgrade;-reconfigure reconfigure;-lock=false no lock",
  ansible:
    "-i inventory;-m module;-a args;-b become;-u user;-k ask pass;-K ask become pass;--become-user become user;-e extra vars;-l limit;-f forks;-v verbose;--list-hosts list hosts;-C check;-D diff;--private-key key;-c connection",
  "ansible-playbook":
    "-i inventory;-l limit;-t tags;--skip-tags skip tags;-e extra vars;-b become;-K ask become pass;-k ask pass;-u user;-C check;-D diff;-v verbose;--list-tasks list tasks;--list-hosts list hosts;--syntax-check syntax;--start-at-task start at;--step step;-f forks;--private-key key;--vault-password-file vault pass file;--ask-vault-pass ask vault pass",
  aws: "--profile profile;--region region;--output output format;--query JMESPath query;--no-cli-pager no pager;--dry-run dry run",
  az: "-g resource group;-n name;-o output;--query JMESPath;--subscription subscription;-l location",
  gcloud: "--project project;--zone zone;--region region;--format format;--filter filter;-q quiet",
  podman:
    "-it interactive tty;-d detached;--rm remove on exit;-p publish port;-v volume;--name name;-e env",
  gh: "-R repository;--web open in browser",
  code: "-r reuse window;-n new window;-g goto file:line;-d diff;-w wait;--install-extension install extension;--list-extensions list extensions;--disable-extensions disable extensions;-a add folder",
  vim: "-R read-only;-o horizontal splits;-O vertical splits;-p tabs;-d diff mode;-u vimrc;-c command;+ line number;-n no swap;-b binary;--clean no config;-S session",
  nvim: "-R read-only;-o horizontal splits;-O vertical splits;-p tabs;-d diff mode;-u init file;-c command;+ line number;--clean no config;--headless headless;-S session;--listen server",
  nano: "-l line numbers;-m mouse;-w no wrap;-B backup;-i auto indent;-T tab size;-E tabs to spaces;-c constant show cursor;-S smooth scroll;-v view mode;+ line number",
  ncdu: "-x one filesystem;-e extended info;-r read-only;-q quiet;-o export;-f import;--exclude exclude;-t threads;--color colour",
  tree: "-L depth;-a all files;-d directories only;-I ignore pattern;-P pattern;-h human sizes;-f full paths;-C colour;-s sizes;-D dates;-p permissions;--du dir sizes;-J JSON;--gitignore respect gitignore;--dirsfirst dirs first",
  fzf: "-m multi select;--preview preview command;-q query;-e exact;--height height;--reverse reverse layout;--border border;--ansi ANSI colours;-1 select only match;-0 exit on no match;--bind key bindings;--header header;--prompt prompt;-d delimiter;-n fields;--tac reverse input;-i case-insensitive",
  man: "-k search descriptions;-f whatis;-a all pages;-w where;-P pager;-l local file;-K full text search",
  timeout:
    "-s signal;-k kill after;--preserve-status preserve status;--foreground foreground;-v verbose",
  base64: "-d decode;-w wrap columns;-i ignore garbage",
  sha256sum:
    "-c check;-b binary;--quiet quiet;--status status only;--ignore-missing ignore missing;--tag BSD style",
  md5sum: "-c check;-b binary;--quiet quiet;--status status only",
  file: "-b brief;-i MIME type;-L follow symlinks;-z look inside compressed;-s read block/char devices;-k keep going",
  stat: "-c format;-f filesystem;-L follow symlinks;-t terse;--printf printf format",
  touch:
    "-a access time;-m modification time;-c no create;-d date string;-r reference file;-t timestamp",
  env: "-i empty environment;-u unset;-0 null separated;-C chdir;-S split string",
  type: "-a all locations;-t type only;-p path;-P force path search",
  which: "-a all matches;-s silent",
  seq: "-s separator;-w equal width;-f format",
  shuf: "-n count;-e args as input;-i range;-r repeat;-o output;-z null separated",
  split:
    "-l lines per file;-b bytes per file;-n number of chunks;-d numeric suffixes;-a suffix length;--additional-suffix suffix",
  column:
    "-t table;-s separator;-c width;-n no merge delimiters;-N column names;-J JSON;-o output separator",
  tee: "-a append;-i ignore interrupts;-p diagnose write errors",
  lscpu: "-e extended;-p parsable;-J JSON;-a all CPUs;-b online;-c offline",
  lspci:
    "-v verbose;-vv very verbose;-k kernel drivers;-nn IDs;-t tree;-s slot;-d device;-x hex dump",
  lsusb: "-v verbose;-t tree;-s bus:device;-d vendor:product",
  smartctl:
    "-a all info;-H health;-i identity;-t run test;-l log;-x extended;-d device type;-c capabilities;-A attributes",
  mtr: "-r report;-c count;-n no DNS;-b IPs and names;-4 IPv4;-6 IPv6;-T TCP;-u UDP;-P port;-i interval;-w wide;-z AS numbers",
  traceroute:
    "-n numeric;-I ICMP;-T TCP;-p port;-m max hops;-q queries;-w wait;-4 IPv4;-6 IPv6;-i interface",
  iperf3:
    "-s server;-c client;-p port;-t time;-u UDP;-b bandwidth;-P parallel;-R reverse;-J JSON;-i interval;-w window;-B bind;-4 IPv4;-6 IPv6;--bidir bidirectional",
  socat:
    "-d debug;-v verbose;-u unidirectional;-T timeout;TCP-LISTEN: listen TCP;TCP: connect TCP;UNIX-CONNECT: unix socket;EXEC: exec;STDIO stdio;OPENSSL: TLS;UDP: UDP;PTY pseudo tty;FILE: file",
  http: "-j JSON;-f form;-a auth;-v verbose;-h headers only;-b body only;-d download;-o output;--follow follow redirects;--verify verify TLS;-p print parts;--session session;--timeout timeout",
  rclone:
    "-P progress;-v verbose;--dry-run dry run;--transfers transfers;--exclude exclude;--include include;-n dry run;--bwlimit bandwidth",
  age: "-e encrypt;-d decrypt;-r recipient;-R recipients file;-i identity;-p passphrase;-o output;-a armour",
  sops: "-e encrypt;-d decrypt;-i in place;-r rotate;--age age recipients;--kms KMS arn;--pgp PGP fingerprint;--input-type input type;--output-type output type;--extract extract path;--set set",
  hyperfine:
    "-w warmup;-r runs;-m min runs;-M max runs;-N no shell;-p prepare;-c cleanup;--export-json JSON;--export-markdown markdown;-i ignore failure;-s style;-L parameter list;-P parameter scan",
  entr: "-c clear;-r restart;-d track dirs;-p postpone;-s shell;-z exit after;-n non-interactive",
  bat: "-n numbers;-p plain;-A show all;-l language;--paging paging;--style style;-d diff;-r line range;-H highlight line;--theme theme;--list-themes themes;-L languages;--color colour",
  eza: "-l long;-a all;-T tree;-L level;--git git status;--icons icons;-h header;-s sort;-r reverse;-d dirs;--group-directories-first dirs first;-1 one per line;-G grid;--total-size total size",
  fd: "-e extension;-t type;-H hidden;-I no ignore;-x exec;-X exec batch;-i ignore case;-s case sensitive;-g glob;-p full path;-d max depth;-E exclude;-a absolute;-l list details;-0 null separated;--changed-within changed within;-S size",
  pwsh: "-c command;-Command command;-File script file;-NoProfile no profile;-NoExit no exit;-NonInteractive non-interactive;-ExecutionPolicy execution policy;-WorkingDirectory working dir;-Version version;-Login login shell;-EncodedCommand base64 command",
  powershell:
    "-Command command;-File script file;-NoProfile no profile;-NoExit no exit;-NonInteractive non-interactive;-ExecutionPolicy execution policy;-WindowStyle window style;-EncodedCommand base64 command;-Version version",
  bash: "-c command;-l login;-i interactive;-x trace;-e exit on error;-u unset error;-n syntax check;--norc no rc;--noprofile no profile;-s stdin;-v verbose;--posix POSIX;-o option;--version version",
  zsh: "-c command;-l login;-i interactive;-x trace;-e exit on error;-u unset error;-n syntax check;-f no rc;-d no global rc;-o option;--version version;-s stdin;-v verbose",
  fish: "-c command;-l login;-i interactive;-P private;-N no config;--no-config no config;-d debug;-C init command;-v version;-p profile",
  sh: "-c command;-l login;-i interactive;-x trace;-e exit on error;-u unset error;-n syntax check;-s stdin;-v verbose",
  script:
    "-a append;-c command;-q quiet;-f flush;-t timing;-e return exit code;-T timing file;-o output limit",
  at: "-l list;-r remove;-d delete;-f file;-m mail;-q queue;-v show time;-c cat job;-t time",
  logrotate: "-d debug;-f force;-v verbose;-s state file;-m mail command",
  logger:
    "-t tag;-p priority;-s stderr too;-i PID;-f file;-n server;-P port;-d UDP;-T TCP;--journald journald;-e skip empty",
  xrandr:
    "--output output;--mode mode;--auto auto;--off off;--primary primary;--left-of left of;--right-of right of;--above above;--below below;--rotate rotate;--scale scale;--rate rate;-q query;--listmonitors monitors;--brightness brightness;--dpi DPI;--same-as mirror",
  "notify-send":
    "-u urgency;-t timeout;-i icon;-a app name;-c category;-h hint;-p print id;-r replace id;-w wait;-A action;-e transient",
  open: "-a application;-e TextEdit;-t default text editor;-R reveal in Finder;-n new instance;-g background;-b bundle id;-W wait;-f read stdin;-u URL;--args arguments",
  xclip:
    "-selection selection;-sel selection;-o output;-i input;-l loops;-t target;-r remove newline;-f filter",
  xsel: "-b clipboard;-p primary;-s secondary;-o output;-i input;-c clear;-a append;-k keep;-x exchange;-d delete",
  "yt-dlp":
    "-f format;-F list formats;-o output template;-x extract audio;--audio-format audio format;--audio-quality audio quality;-a batch file;-i ignore errors;--write-subs subtitles;--write-auto-subs auto subs;--sub-langs sub languages;--embed-subs embed subs;--embed-thumbnail embed thumbnail;--embed-metadata embed metadata;--playlist-items playlist items;--yes-playlist playlist;--no-playlist no playlist;-S sort formats;--merge-output-format merge format;--cookies cookies;--cookies-from-browser browser cookies;-U update;--list-subs list subs;--download-archive archive;-r rate limit;-q quiet;--progress progress;-P paths",
  mpv: "--no-video audio only;--fs fullscreen;--loop loop;--shuffle shuffle;--volume volume;--speed speed;--start start time;--sub-file subtitles;--ytdl-format yt-dlp format;--playlist playlist;--mute mute;--no-audio no audio;--vo video output;--ao audio output;--hwdec hw decode;--really-quiet quiet;--pause start paused;--geometry geometry;--ontop on top;--no-border no border;--keep-open keep open;--save-position-on-quit save position",
  convert:
    "-resize resize;-quality quality;-crop crop;-rotate rotate;-flip flip;-flop flop;-strip strip metadata;-background background;-gravity gravity;-extent extent;-density density;-trim trim;-format format;-colorspace colorspace;-negate negate;-blur blur;-sharpen sharpen;-append append vertically;+append append horizontally;-flatten flatten;-alpha alpha;-fuzz fuzz;-transparent transparent colour;-border border;-annotate annotate;-pointsize point size;-font font;-fill fill;-auto-orient auto orient;-thumbnail thumbnail;-delay delay;-loop loop",
  exiftool:
    "-all= strip all;-overwrite_original overwrite;-r recursive;-s short tags;-S very short;-G group;-a all duplicates;-u unknown;-j JSON;-csv CSV;-T tab;-d date format;-ext extension;-o output;-tagsFromFile copy tags;-P preserve mtime;-q quiet;-v verbose;-n numeric;-g group by;-common common tags;-time:all time tags;-AllDates all dates",
  qrencode:
    "-o output;-s size;-l level;-m margin;-t type (PNG, ANSI, UTF8, SVG);-r read file;-v version;-d DPI;-8 8-bit;-i ignore case;-k kanji;-c casesensitive;-M micro QR;--foreground foreground;--background background",
  uuidgen: "-r random;-t time;-m MD5;-s SHA1;-n namespace;-N name;-x hex;-C count",
  pwgen:
    "-s secure;-y symbols;-n numerals;-c capitals;-B no ambiguous;-1 one per line;-A no capitals;-0 no numerals;-v no vowels;-N count;-r remove chars;-C columns",
  cd: "-L logical;-P physical;- previous;~ home;.. parent",
  pwd: "-L logical;-P physical",
  clear: "-x keep scrollback;-T terminal type;-V version",
  test: "-e exists;-f regular file;-d directory;-r readable;-w writable;-x executable;-s non-empty;-L symlink;-z empty string;-n non-empty string;-eq equal;-ne not equal;-lt less than;-le less or equal;-gt greater than;-ge greater or equal;-nt newer than;-ot older than;-ef same file;-a and;-o or;! not",
  set: "-e exit on error;-u unset error;-x trace;-o pipefail pipefail;-o option;+e no exit on error;+x no trace;-v verbose;-n no exec;-f no glob;-C no clobber;-a allexport;-m monitor;-B braceexpand;-P physical;-T functrace;-E errtrace;-- end options",
  ulimit:
    "-a all;-c core size;-d data size;-f file size;-l locked memory;-m memory;-n open files;-p pipe size;-s stack size;-t CPU time;-u processes;-v virtual memory;-x file locks;-H hard;-S soft",
  read: "-a array;-d delimiter;-e readline;-i initial text;-n nchars;-N exactly nchars;-p prompt;-r raw;-s silent;-t timeout;-u fd",
  declare:
    "-a array;-A assoc array;-f functions;-F function names;-g global;-i integer;-l lowercase;-n nameref;-r readonly;-t trace;-u uppercase;-x export;-p print",
  compgen:
    "-c commands;-a aliases;-b builtins;-k keywords;-f files;-d directories;-u users;-g groups;-v variables;-e exported;-j jobs;-s services;-A action;-W wordlist;-F function;-C command;-X filter;-P prefix;-S suffix;-o option",
  trap: "-l list signals;-p print;EXIT on exit;ERR on error;INT SIGINT;TERM SIGTERM;HUP SIGHUP;DEBUG debug;RETURN return",
  tput: "clear clear;reset reset;cols columns;lines lines;colors colours;bold bold;sgr0 reset attributes;setaf foreground;setab background;smcup alt screen;rmcup main screen;civis hide cursor;cnorm show cursor;cup cursor position;el clear to EOL;ed clear to EOS;smso standout;rmso end standout;smul underline;rmul end underline;rev reverse;bel bell;longname long name",
  stty: "-a all;-g save;sane sane;raw raw;cooked cooked;echo echo;-echo no echo;icanon canonical;-icanon non-canonical;rows rows;cols columns;size size;speed speed;erase erase char;intr interrupt char;kill kill char;eof EOF char;ixon flow control;-ixon no flow control;-F device",
  "7z": "a add;x extract with paths;e extract;l list;t test;u update;d delete;b benchmark;h hash;-o output dir;-p password;-r recursive;-t type;-m method;-mx compression level;-mhe encrypt headers;-v volume size;-x exclude;-i include;-y yes",
  unrar:
    "x extract with paths;e extract;l list;t test;p print;v verbose list;-p password;-o+ overwrite;-o- no overwrite;-y yes;-r recursive;-x exclude;-inul no messages;-kb keep broken",
  cmake:
    "-S source dir;-B build dir;-G generator;-D define variable;--build build;--install install;--preset preset;--list-presets list presets;-E command mode;-P script mode;--fresh fresh configure;--config config;--target target;-j parallel;--clean-first clean first;--verbose verbose;--prefix install prefix;--version version",
  just: "--list list recipes;-l list recipes;--show show recipe;-s show recipe;--summary summary;--choose choose;--dry-run dry run;-n dry run;--evaluate evaluate;--variables variables;--fmt format;--check check;--init init;--edit edit;-e edit;--dump dump;--justfile justfile;-f justfile;--working-directory working dir;-d working dir;--set set variable;--verbose verbose;-v verbose;--quiet quiet;-q quiet",
  grpcurl:
    "list list;describe describe;-plaintext plaintext;-insecure insecure;-d data;-H header;-proto proto file;-import-path import path;-protoset protoset;-authority authority;-cacert CA cert;-cert cert;-key key;-v verbose;-format format;-emit-defaults emit defaults;-max-time max time;-connect-timeout connect timeout",
  pandoc:
    "-f from format;-t to format;-o output;-s standalone;--pdf-engine PDF engine;--toc table of contents;-N number sections;-V variable;-M metadata;--template template;-c CSS;--css CSS;-H header;--highlight-style highlight style;--filter filter;-L Lua filter;--extract-media extract media;--bibliography bibliography;--csl CSL;--citeproc citeproc;--mathjax MathJax;--katex KaTeX;--wrap wrap;--columns columns;--embed-resources embed resources;--list-input-formats input formats;--list-output-formats output formats;--verbose verbose;--quiet quiet;--version version",
  showmount: "-e exports;-a all mounts;-d directories;--no-headers no headers",
  smbclient:
    "-L list shares;-U user;-W workgroup;-N no password;-c command;-m max protocol;-p port;-I IP;-A auth file;-k kerberos;-E stderr;-d debug;-D directory;-T tar;-g grepable;-e encrypt",
  ldapsearch:
    "-x simple auth;-H URI;-D bind DN;-W prompt password;-b base DN;-s scope;-LLL LDIF without comments;-Z StartTLS;-ZZ require StartTLS;-o option;-z size limit;-l time limit;-a deref;-A attrs only;-S sort;-Y SASL mechanism;-U SASL user;-N no reverse DNS;-n dry run;-v verbose;-d debug;-f file;-h host;-p port",
  kinit:
    "-f forwardable;-r renewable;-l lifetime;-k keytab;-t keytab file;-c cache;-R renew;-V verbose;-p proxiable;-a addresses;-A no addresses;-S service;-n anonymous;-E enterprise",
  klist:
    "-e enctypes;-f flags;-a addresses;-A all caches;-l list caches;-k keytab;-s silent;-c cache;-t timestamps;-K keys;-V version",
  sensors: "-f Fahrenheit;-A no adapter;-u raw;-j JSON;-s set;-c config",
  "nvidia-smi":
    "-l loop;-q query;-i GPU id;--query-gpu query fields;--format format;-L list GPUs;-pm persistence mode;-pl power limit;-r reset",
  sestatus: "-v verbose;-b booleans",
  chcon: "-t type;-u user;-r role;-R recursive;-v verbose;--reference reference;-h affect symlinks",
  restorecon:
    "-R recursive;-v verbose;-n dry run;-F force;-i ignore missing;-p progress;-e exclude",
  semanage: "-a add;-d delete;-m modify;-l list;-t type;-p protocol",
  auditctl:
    "-l list rules;-w watch path;-p permissions;-k key;-a append rule;-A prepend rule;-d delete rule;-D delete all;-s status;-e enable;-b backlog;-f failure mode;-S syscall;-F field;-R read rules file",
  getfacl:
    "-a access ACL;-d default ACL;-c no comments;-e effective rights;-E no effective;-s skip base;-R recursive;-L logical;-P physical;-t tabular;-n numeric;-p absolute names",
  setfacl:
    "-m modify;-x remove;-b remove all;-k remove default;-d default;-R recursive;-L logical;-P physical;-n no mask;--mask recalc mask;--set set;--set-file set from file;-M modify from file;-X remove from file;--restore restore;--test test",
  losetup:
    "-a all;-d detach;-D detach all;-f find free;-j associated;-l list;-P partscan;-r read-only;-o offset;--sizelimit size limit;--show show device;-J JSON;-v verbose",
  cryptsetup: "-y verify passphrase;--type type;-c cipher;-s key size;--key-file key file",
  mdadm:
    "--create create;--assemble assemble;--detail detail;--examine examine;--manage manage;--add add;--remove remove;--fail fail;--stop stop;--run run;--scan scan;--monitor monitor;--grow grow;--zero-superblock zero superblock;-l level;-n raid devices;-x spare devices;-c chunk;-b bitmap;-v verbose",
  lvcreate:
    "-L size;-l extents;-n name;-s snapshot;-T thin;-V virtual size;-i stripes;-I stripe size;-m mirrors;--type type;-y yes",
  lvextend: "-L size;-l extents;-r resize fs;-n no fsck;-f force;-t test;-v verbose;-y yes",
  tune2fs:
    "-l list;-L label;-c max mount count;-i interval;-m reserved percent;-r reserved blocks;-o mount options;-O features;-e error behaviour;-j add journal;-U UUID;-E extended options",
  resize2fs: "-f force;-F flush;-M minimize;-P print minimum;-p progress",
  parted: "-l list;-s script;-a alignment;-m machine",
  blkid:
    "-o output format;-s tag;-t token;-p probe;-i I/O limits;-c cache file;-g garbage collect;-k list filesystems;-L label;-U UUID",
  findmnt:
    "-t type;-o output;-l list;-r raw;-J JSON;-D df style;-n no headings;-T target;-S source;-s fstab;-m mtab;-k kernel;-R submounts",
  swapon:
    "-a all;-s summary;-p priority;-d discard;-e ifexists;-o options;-L label;-U UUID;-v verbose;--show show",
  swapoff: "-a all;-v verbose;-L label;-U UUID",
  hwclock:
    "-r show;-s hctosys;-w systohc;--set set;--date date;-u UTC;-l localtime;--adjust adjust;-v verbose;-f RTC device",
  loginctl:
    "-a all;-p property;--value value only;--no-pager no pager;--no-legend no legend;-l full;-n lines;-o output;-s signal",
  hdparm: "-I identify;-t timing;-T cache timing;-i info;-y standby;-Y sleep;-B APM;-S spindown",
  ethtool:
    "-s settings;-i driver info;-S statistics;-k offload;-K set offload;-a pause;-A set pause;-c coalesce;-C set coalesce;-g ring;-G set ring;-l channels;-L set channels;-p identify;-t test;-r restart;-m module info;-P permanent address;-T timestamping",
  tshark:
    "-i interface;-r read file;-w write file;-f capture filter;-Y display filter;-T output format;-e field;-E field options;-c count;-a autostop;-b ring buffer;-D list interfaces;-V verbose;-x hex;-q quiet;-z statistics;-n no resolve;-d decode as;-o preference;-l flush;-p no promiscuous;-s snaplen;--color colour;--export-objects export objects",
  ftp: "-i no interactive;-n no auto-login;-v verbose;-p passive;-g no globbing;-d debug;-4 IPv4;-6 IPv6;-A active;-e no editing;-t trace;-P port",
  sftp: "-P port;-i identity;-b batch file;-r recursive;-p preserve;-C compression;-q quiet;-v verbose;-o option;-F config;-J jump host;-l limit;-B buffer size;-R requests;-a resume;-4 IPv4;-6 IPv6",
  telnet:
    "-l user;-a auto login;-e escape char;-8 8-bit;-E no escape;-L 8-bit output;-K no auto login;-d debug;-4 IPv4;-6 IPv6",
  mosh: "--ssh SSH command;-p port;--server server command;--predict prediction mode;-a always predict;-n never predict;--no-init no init;-4 IPv4;-6 IPv6",
  ncat: "-l listen;-p port;-v verbose;-z scan only;-u UDP;-w timeout;-k keep listening;--ssl SSL;--ssl-cert cert;--ssl-key key;--proxy proxy;--proxy-type proxy type;-e exec;-c sh exec;-m max conns;-d delay;-o output;-x hex dump;-i idle timeout;-s source;-4 IPv4;-6 IPv6;-U unix;--broker broker;--chat chat;--allow allow;--deny deny;-C CRLF;-t telnet;-n no DNS",
  aria2c:
    "-o output;-d directory;-x max connections;-s split;-j max concurrent;-c continue;-i input file;-l log;-q quiet;-V check integrity;-k min split;--max-download-limit limit;--seed-time seed time;--seed-ratio seed ratio;--enable-rpc RPC;-D daemon;--check-certificate check cert;--header header;--user-agent user agent;--all-proxy proxy;--max-tries max tries;--retry-wait retry wait;--timeout timeout;--allow-overwrite overwrite",
  iftop:
    "-i interface;-n no DNS;-N no ports;-P show ports;-B bytes;-b no bars;-F net filter;-f filter;-p promiscuous;-c config;-t text;-s seconds;-L lines;-m max bandwidth;-o sort order",
  nethogs:
    "-d delay;-v view mode;-c count;-t trace;-p promiscuous;-s sort sent;-a all devices;-C capture;-b bugs;-f filter;-P PID;-l list",
  tc: "-s statistics;-d details;-r raw;-p pretty;-b batch;-n netns;-j JSON;-c colour",
  arp: "-a all;-n numeric;-v verbose;-d delete;-s set;-i interface;-f file;-D device",
  route:
    "-n numeric;-e extended;-v verbose;-A family;-C cache;add add;del delete;-net network;-host host;gw gateway;netmask netmask;dev device;metric metric;default default",
  ifconfig:
    "-a all;-s short;-v verbose;up up;down down;netmask netmask;broadcast broadcast;mtu MTU;hw hardware address;promisc promiscuous;multicast multicast;txqueuelen TX queue length",
  nslookup:
    "-type= record type;-query= record type;-port= port;-timeout= timeout;-retry= retries;-debug debug;-vc TCP;-domain= domain",
  host: "-a all;-t type;-v verbose;-4 IPv4;-6 IPv6;-C SOA check;-d debug;-l list zone;-r no recursion;-T TCP;-U UDP;-W wait;-R retries;-c class;-p port",
  whois:
    "-h host;-p port;-H hide legal;-r no recursion;-a all databases;-i inverse;-T type;-K key;-x exact;-b brief;-B no filter;-G no grouping;-d reverse delegation;-q query;-v verbose template",
  ip6tables:
    "-L list;-A append;-I insert;-D delete;-F flush;-P policy;-t table;-n numeric;-v verbose;-p protocol;--dport destination port;--sport source port;-s source;-d destination;-j jump target;-i in interface;-o out interface;-m match;-S list rules;--line-numbers line numbers",
  openvpn:
    "--config config;--client client;--dev device;--proto protocol;--remote remote;--port port;--ca CA;--cert cert;--key key;--auth-user-pass user/pass;--verb verbosity;--log log;--daemon daemon;--route route;--redirect-gateway redirect gateway;--keepalive keepalive;--status status;--genkey genkey;--version version",
  sshuttle:
    "-r remote;-e ssh command;-l listen;-x exclude;-N auto nets;-H auto hosts;-D daemon;-v verbose;--dns DNS;--method method;--python python;--pidfile pid file;--disable-ipv6 no IPv6",
  autossh: "-M monitor port;-f background;-V version",
  gunzip: "-c stdout;-f force;-k keep;-l list;-q quiet;-r recursive;-t test;-v verbose",
  bzip2:
    "-d decompress;-z compress;-k keep;-f force;-t test;-c stdout;-q quiet;-v verbose;-s small;-1 fast;-9 best",
  unxz: "-k keep;-f force;-t test;-c stdout;-q quiet;-v verbose;-T threads;-l list",
  pigz: "-d decompress;-k keep;-p processes;-1 fast;-9 best;-b block size;-c stdout;-f force;-r recursive;-t test;-l list;-K zip;-z zlib;-i independent;-q quiet;-v verbose;-R rsyncable",
  zgrep:
    "-i ignore case;-n line numbers;-v invert;-l files with matches;-c count;-E extended regex;-F fixed strings;-w whole words;-o only matching;-A after;-B before;-C context;-r recursive;-h no filename;-H filename;-q quiet;--color colour",
  cpio: "-o create;-i extract;-p pass through;-t list;-v verbose;-d make dirs;-m preserve mtime;-u unconditional;-H format;-F file;-I input file;-O output file;-B block size;-c ASCII;-A append;-L follow symlinks;-R owner;-E pattern file;-0 null;-D directory;--quiet quiet",
  aptitude:
    "-y yes;-s simulate;-d download only;-f fix broken;-v verbose;-q quiet;-P prompt;-V show versions;-D show deps;-Z show sizes;-W show why;-r recommends;-R no recommends;-t target release;-o option;-F format;-w width;--purge-unused purge unused",
  yay: "-S install;-Syu full upgrade;-Ss search;-Si info;-R remove;-Rns remove with deps;-Q query;-Qs search installed;-Qi installed info;-Qu updates;-Sc clean cache;-Scc clean all cache;-Sy sync;-G get PKGBUILD;-P print;-Y yay;-U install file;--noconfirm no confirm;--needed needed;--devel devel;--aur AUR only;--repo repo only;--gendb generate db;--stats stats;--news news;--sudoloop sudo loop;--rebuild rebuild;--redownload redownload;--cleanafter clean after;--removemake remove make",
  paru: "-S install;-Syu full upgrade;-Ss search;-Si info;-R remove;-Rns remove with deps;-Q query;-Qs search installed;-Qi installed info;-Qu updates;-Sc clean cache;-Scc clean all cache;-Sy sync;-G get PKGBUILD;-P print;-L local repo;-B build;-C chroot;-U install file;--noconfirm no confirm;--needed needed;--devel devel;--aur AUR only;--repo repo only;--review review;--noreview no review;--gendb generate db;--stats stats;--news news;--skipreview skip review;--rebuild rebuild;--redownload redownload;--cleanafter clean after;--removemake remove make",
  makepkg:
    "-s sync deps;-i install;-c clean;-C clean build;-f force;-r remove deps;-e no extract;-o no build;-d no deps;-g gen integrity;-p PKGBUILD;-A ignore arch;-L log;-R repackage;-S source;--noconfirm no confirm;--needed needed;--asdeps as deps;--nocheck no check;--skipinteg skip integrity;--skippgpcheck skip PGP;--printsrcinfo print SRCINFO;--packagelist package list;--holdver hold version",
  emerge:
    "-a ask;-p pretend;-v verbose;-u update;-D deep;-N newuse;-U changed-use;-1 oneshot;-C unmerge;-c depclean;-s search;-S search desc;-f fetch only;-k use pkg;-K use pkg only;-b build pkg;-e emptytree;-n noreplace;-o onlydeps;-O nodeps;-q quiet;-t tree;-j jobs;-l load average;--sync sync;--info info;--resume resume;--skipfirst skip first;--keep-going keep going;--autounmask autounmask;--autounmask-write autounmask write;--backtrack backtrack;--exclude exclude",
  "nix-shell":
    "-p packages;--packages packages;--run run;--command command;--pure pure;-i interpreter;--arg arg;--argstr arg string;-A attribute;-I include;-E expression;--keep keep;-j jobs;--cores cores;--option option;--show-trace show trace;-v verbose",
  pkg: "-y yes;-f force;-q quiet;-v verbose;-n dry run;-r repository;-g glob;-x regex;-C case sensitive;-i case insensitive;-d dependencies;-R reverse dependencies;-a all;-o origin;-e exact;-A automatic;-l local;-j jail;-c chroot;-N activated",
  "xbps-install":
    "-S sync;-u update;-y yes;-f force;-n dry run;-v verbose;-d debug;-A automatic;-D download only;-i ignore config;-I ignore file conflicts;-M memory sync;-R repository;-r rootdir;-c cachedir;-C config;-U unpack only;-V version;-h help",
  opkg: "install install;remove remove;update update;upgrade upgrade;list list;list-installed list installed;list-upgradable list upgradable;info info;status status;files files;search search;find find;download download;configure configure;print-architecture print arch;depends depends;whatdepends whatdepends;whatprovides whatprovides;whatconflicts whatconflicts;whatreplaces whatreplaces;flag flag;compare-versions compare versions;-d dest;-o offline root;-f config;-t tmp dir;-l lists dir;--force-depends force depends;--force-reinstall force reinstall;--force-overwrite force overwrite;--force-downgrade force downgrade;--force-space force space;--force-maintainer force maintainer;--noaction no action;--download-only download only;--nodeps no deps;--autoremove autoremove;--verbosity verbosity;-V verbosity;-v version;-h help",
  winget:
    "--id id;-e exact;--all all;-s source;--silent silent;--accept-package-agreements accept agreements;--accept-source-agreements accept source;-v version;--scope scope;-l location;-i interactive;-h silent;--force force;-n name;--moniker moniker;--tag tag;--cmd command;--upgrade-available upgrade available;--include-unknown include unknown;--pinned pinned;--exact exact;--count count;--header header;--verbose-logs verbose logs;--disable-interactivity disable interactivity;--wait wait;--logs logs;--open-logs open logs;--info info;--help help",
  choco:
    "-y yes;--version version;--pre prerelease;-f force;all all;-s source;-x force dependencies;-r limit output;--params params;--ia install args;-o override args;-n not silent;--ignore-checksums ignore checksums;--allow-empty-checksums allow empty checksums;--exact exact;--id-only id only;--by-id-only by id only;--local-only local only;--include-programs include programs;--page page;--page-size page size;--order-by-popularity order by popularity;--approved-only approved only;--download-cache-only download cache only;--not-broken not broken;--detail detail;--prerelease prerelease;-d debug;-v verbose;--trace trace;--nocolor no colour;--accept-license accept license;--confirm confirm;--limit-output limit output;--timeout timeout;--cache-location cache location;--allow-unofficial allow unofficial;--fail-on-standard-error fail on stderr;--use-system-powershell use system PowerShell;--no-progress no progress;--proxy proxy;--proxy-user proxy user;--proxy-password proxy password;--proxy-bypass-list proxy bypass list;--proxy-bypass-on-local proxy bypass on local;--log-file log file;--skip-compatibility-checks skip compatibility;--ignore-detected-reboot ignore detected reboot;--exit-when-reboot-detected exit when reboot;-h help",
  scoop:
    "-g global;-k no cache;-u no update scoop;-s skip hash check;-a arch;-i independent;-p purge;-f force;-q quiet;-v verbose;-h help",
};

const SUB: Record<string, string> = {
  git: "status working tree status;add stage changes;commit record changes;push update remote;pull fetch and merge;fetch download objects;checkout switch branches / restore;switch switch branches;branch list / create branches;merge join histories;rebase reapply commits;log commit history;diff show changes;stash shelve changes;reset reset HEAD;restore restore files;clone clone a repository;init create a repository;remote manage remotes;tag tags;show show objects;cherry-pick apply commits;revert revert commits;blame line-by-line authors;bisect binary search bugs;clean remove untracked files;config configuration;worktree worktrees;submodule submodules;reflog reference logs;describe describe commit;shortlog summarize log;grep search tree;mv move files;rm remove files;apply apply patch;format-patch prepare patches;am apply mail patches;archive create archive;gc garbage collect;fsck check objects;ls-files list files;ls-remote list remote refs;rev-parse parse revisions;merge-base common ancestor;notes notes;range-diff compare ranges;sparse-checkout sparse checkout;maintenance maintenance;help help",
  docker:
    "run create and run a container;ps list containers;images list images;pull download an image;push upload an image;build build an image;exec run in a container;logs container logs;stop stop containers;start start containers;restart restart containers;rm remove containers;rmi remove images;kill kill containers;inspect low-level info;compose Docker Compose;volume volumes;network networks;system system info / prune;container containers;image images;stats resource usage;top processes;cp copy files;attach attach;commit commit changes;tag tag image;login registry login;logout registry logout;save save images;load load images;export export container;import import image;history image history;port port mappings;rename rename container;pause pause;unpause unpause;wait wait;diff filesystem changes;events events;info system info;version version;context contexts;buildx extended build;manifest manifests;swarm swarm;service services;stack stacks;node nodes;secret secrets;config configs;plugin plugins;search search hub;update update containers;init init project",
  "docker-compose":
    "up create and start;down stop and remove;ps list;logs logs;build build;pull pull;restart restart;stop stop;start start;exec exec;run run;config validate;top processes;images images;kill kill;pause pause;unpause unpause;rm remove;create create;port port;events events;version version;push push",
  podman:
    "run run;ps list;images images;pull pull;push push;build build;exec exec;logs logs;stop stop;start start;rm remove;rmi remove image;pod pods;volume volumes;network networks;inspect inspect;system system;generate generate;play play kube;machine machines;compose compose;kill kill;restart restart;top processes;stats stats;cp copy;attach attach;commit commit;tag tag;login login;logout logout;save save;load load;export export;import import;history history;port port;rename rename;pause pause;unpause unpause;wait wait;diff diff;events events;info info;version version;search search;secret secrets;container containers;image images;manifest manifests;healthcheck healthcheck;auto-update auto update;kube kube;untag untag;mount mount;unmount unmount;init init;create create",
  kubectl:
    "get display resources;describe resource details;apply apply configuration;delete delete resources;logs container logs;exec execute in container;create create resource;edit edit resource;port-forward forward ports;rollout rollouts;scale scale;config kubeconfig;top resource usage;run run image;expose expose;set set features;explain resource docs;label labels;annotate annotations;patch patch;replace replace;cp copy files;attach attach;auth authorization;debug debug;events events;cluster-info cluster info;api-resources API resources;api-versions API versions;version version;drain drain node;cordon cordon node;uncordon uncordon node;taint taints;diff diff;kustomize kustomize;wait wait;proxy proxy;certificate certificates;completion completion;plugin plugins;autoscale autoscale",
  helm: "install install;upgrade upgrade;uninstall uninstall;list list;ls list;repo repositories;search search;template render;rollback rollback;history history;status status;show show;pull pull;dependency dependencies;lint lint;package package;create create chart;get get;test test;env environment;plugin plugins;registry registry;push push;verify verify;version version;completion completion",
  systemctl:
    "status unit status;start start units;stop stop units;restart restart units;reload reload units;enable enable units;disable disable units;is-active is active;is-enabled is enabled;is-failed is failed;list-units list units;list-unit-files list unit files;list-timers list timers;list-sockets list sockets;list-dependencies dependencies;daemon-reload reload manager;daemon-reexec reexecute manager;mask mask units;unmask unmask units;edit edit unit;cat show unit file;show show properties;set-property set property;reset-failed reset failed;kill kill unit;try-restart try restart;reload-or-restart reload or restart;isolate isolate target;set-default default target;get-default default target;poweroff power off;reboot reboot;suspend suspend;hibernate hibernate;rescue rescue mode;emergency emergency mode;halt halt;link link unit;revert revert unit;preset preset;list-jobs list jobs;cancel cancel jobs;show-environment environment;set-environment set environment;unset-environment unset environment;log-level log level;switch-root switch root;help help",
  apt: "install install packages;remove remove packages;purge remove with config;update refresh package lists;upgrade upgrade packages;full-upgrade upgrade removing if needed;dist-upgrade smart upgrade;autoremove remove unused;search search packages;show package details;list list packages;policy policy;depends dependencies;rdepends reverse dependencies;clean clean cache;autoclean clean old cache;edit-sources edit sources;source download source;build-dep build dependencies;download download package;changelog changelog;satisfy satisfy dependencies;reinstall reinstall",
  "apt-get":
    "install install;remove remove;purge purge;update update;upgrade upgrade;dist-upgrade dist-upgrade;autoremove autoremove;clean clean;autoclean autoclean;source source;build-dep build-dep;download download;check check;changelog changelog",
  "apt-cache":
    "search search;show show;policy policy;depends depends;rdepends rdepends;madison versions;showpkg show package;stats stats;pkgnames package names;dump dump;unmet unmet;showsrc show source",
  dnf: "install install;remove remove;update update;upgrade upgrade;search search;info info;list list;check-update check updates;autoremove autoremove;clean clean;makecache make cache;repolist repos;repoquery repo query;provides provides;history history;group groups;module modules;config-manager config manager;distro-sync distro sync;downgrade downgrade;reinstall reinstall;swap swap;mark mark;shell shell;builddep build deps;download download;copr COPR;system-upgrade system upgrade;needs-restarting needs restarting;check check;help help",
  yum: "install install;remove remove;update update;upgrade upgrade;search search;info info;list list;check-update check updates;autoremove autoremove;clean clean;makecache make cache;repolist repos;provides provides;whatprovides what provides;history history;groupinstall group install;grouplist group list;groupremove group remove;downgrade downgrade;reinstall reinstall;localinstall local install;deplist deplist;shell shell;version version;help help;erase erase;distro-sync distro sync;swap swap;check check;updateinfo update info;repoinfo repo info",
  zypper:
    "install install;in install;remove remove;rm remove;update update;up update;dup distribution upgrade;refresh refresh repos;ref refresh;search search;se search;info info;repos repos;lr repos;addrepo add repo;ar add repo;removerepo remove repo;rr remove repo;patch patches;patches list patches;list-updates list updates;lu list updates;clean clean;verify verify;source-install source install;packages packages;what-provides what provides;locks locks;addlock add lock;removelock remove lock;modifyrepo modify repo;mr modify repo;services services;products products;patterns patterns;shell shell;help help",
  apk: "add add packages;del delete packages;update update index;upgrade upgrade;search search;info info;list list;fix repair;cache cache;version versions;index index;fetch fetch;audit audit;verify verify;dot dot graph;policy policy;stats stats;manifest manifest",
  snap: "install install;remove remove;list list;find search;refresh update;info info;revert revert;disable disable;enable enable;connections connections;services services;logs logs;start start;stop stop;restart restart;set set;get get;unset unset;alias alias;unalias unalias;aliases aliases;changes changes;tasks tasks;abort abort;watch watch;download download;known known;ack ack;login login;logout logout;whoami whoami;version version;help help",
  flatpak:
    "install install;uninstall uninstall;update update;list list;search search;run run;info info;remote-add add remote;remotes remotes;remote-delete remove remote;override override permissions;repair repair;history history;kill kill;ps processes;permissions permissions;permission-reset reset permissions;config config;mask mask;pin pin;enter enter;documents documents;help help",
  brew: "install install formula / cask;uninstall uninstall;remove uninstall;update fetch newest Homebrew;upgrade upgrade packages;search search;info info;list list installed;outdated outdated;cleanup remove old versions;doctor check for problems;tap add a tap;untap remove a tap;services services;link link;unlink unlink;pin pin;unpin unpin;deps dependencies;uses dependents;leaves leaves;autoremove autoremove;bundle bundle;home homepage;log log;edit edit;create create formula;audit audit;test test;reinstall reinstall;fetch fetch;config config;commands commands;analytics analytics;shellenv shell env;help help",
  npm: "install install packages;i install;uninstall remove packages;run run script;start run start script;test run tests;init create package.json;update update packages;outdated check outdated;publish publish;ci clean install;ls list installed;list list installed;link symlink package;audit security audit;cache cache;config config;exec run binary;view view registry info;info view registry info;search search registry;version bump version;login login;logout logout;whoami whoami;pack create tarball;prune remove extraneous;dedupe deduplicate;doctor check environment;explain explain dependency;fund funding;help help;rebuild rebuild;root root folder;set set config;get get config;token tokens;owner owners;deprecate deprecate;dist-tag dist tags;access access;org org;team team;profile profile;star star;ping ping;why explain (alias)",
  pnpm: "install install;i install;add add packages;remove remove packages;rm remove packages;update update;up update;run run script;start start;test test;exec exec;dlx run package;create create;init init;list list;ls list;outdated outdated;why why;prune prune;store store;link link;unlink unlink;import import;rebuild rebuild;publish publish;pack pack;audit audit;licenses licenses;env env;setup setup;config config;patch patch;patch-commit patch commit;dedupe dedupe;fetch fetch;deploy deploy;root root;bin bin;help help",
  yarn: "install install;add add packages;remove remove packages;up upgrade (berry);upgrade upgrade;run run script;start start;test test;init init;dlx run package;exec exec;workspace workspace;workspaces workspaces;info info;why why;list list;outdated outdated;audit audit;cache cache;config config;link link;unlink unlink;pack pack;publish publish;version version;set set (berry);plugin plugins (berry);dedupe dedupe;patch patch;rebuild rebuild;bin bin;global global (classic);create create;import import;licenses licenses;login login;logout logout;help help",
  cargo:
    "build compile;b compile;run run binary;r run binary;test run tests;t run tests;check check without codegen;c check;clippy lint;fmt format;doc build docs;d build docs;new new package;init init package;add add dependency;remove remove dependency;rm remove dependency;update update dependencies;install install binary;uninstall uninstall binary;publish publish crate;login registry login;logout registry logout;search search crates;tree dependency tree;bench benchmarks;clean clean target;fetch fetch dependencies;generate-lockfile generate lockfile;metadata metadata;vendor vendor dependencies;package package crate;version version;help help;owner crate owners;yank yank version;report reports;locate-project locate manifest;rustc compile with rustc args;rustdoc doc with rustdoc args;pkgid package id;fix fix;audit audit (plugin);outdated outdated (plugin);watch watch (plugin);expand expand macros (plugin);nextest nextest (plugin);deny deny (plugin);machete unused deps (plugin);udeps unused deps (plugin);bloat bloat (plugin);flamegraph flamegraph (plugin);msrv MSRV (plugin);release release (plugin);make make (plugin);edit edit deps (plugin);binstall binstall (plugin);hack hack (plugin);semver-checks semver checks (plugin);llvm-cov coverage (plugin);tarpaulin coverage (plugin);sqlx sqlx (plugin);tauri Tauri (plugin);dist dist (plugin);zigbuild zigbuild (plugin);insta insta (plugin);mutants mutants (plugin);fuzz fuzz (plugin);miri miri (plugin);about about (plugin);vet vet (plugin)",
  rustup:
    "update update toolchains;default default toolchain;toolchain toolchains;target targets;component components;show show;override directory overrides;run run with toolchain;self rustup itself;doc open docs;which which binary;check check updates;install install toolchain;uninstall uninstall toolchain;set set settings;completions completions;help help",
  go: "build compile;run compile and run;test test;get add dependencies;mod modules;fmt format;vet vet;install install;env environment;version version;clean clean;doc docs;generate generate;work workspaces;tool tools;list list packages;bug bug report;fix fix;telemetry telemetry;help help",
  pip: "install install packages;uninstall uninstall;list list installed;freeze requirements format;show package info;download download;check verify deps;search search;wheel build wheels;hash hash;config config;debug debug;cache cache;inspect inspect;index index;help help",
  pip3: "install install packages;uninstall uninstall;list list installed;freeze requirements format;show package info;download download;check verify deps;wheel build wheels;hash hash;config config;debug debug;cache cache;inspect inspect;index index;help help",
  pipx: "install install;uninstall uninstall;upgrade upgrade;upgrade-all upgrade all;list list;run run;inject inject;uninject uninject;reinstall reinstall;reinstall-all reinstall all;ensurepath ensure path;environment environment;completions completions;runpip run pip;help help",
  uv: "pip pip interface;venv create venv;run run command;sync sync;lock lock;add add dependency;remove remove dependency;init init project;tool tools;tree dependency tree;python Python versions;build build;publish publish;cache cache;self self;version version;export export;help help;format format",
  poetry:
    "install install;add add;remove remove;update update;lock lock;run run;shell shell;build build;publish publish;init init;new new;show show;env environments;config config;check check;search search;export export;version version;self self;source sources;cache cache;about about;list list;help help",
  conda:
    "install install;remove remove;uninstall uninstall;update update;upgrade upgrade;create create env;env environments;activate activate;deactivate deactivate;list list;search search;info info;config config;clean clean;init init;run run;package package;build build;index index;compare compare;doctor doctor;notices notices;rename rename;export export;help help",
  gem: "install install;uninstall uninstall;list list;update update;search search;build build;push push;env environment;cleanup cleanup;outdated outdated;info info;fetch fetch;contents contents;dependency dependency;pristine pristine;which which;yank yank;owner owner;signin signin;signout signout;sources sources;specification specification;stale stale;unpack unpack;check check;cert cert;help help;lock lock;open open;server server;exec exec;rebuild rebuild",
  bundle:
    "install install;exec exec;update update;add add;remove remove;outdated outdated;init init;lock lock;config config;list list;show show;info info;check check;clean clean;open open;package package;cache cache;platform platform;gem gem;binstubs binstubs;console console;viz viz;pristine pristine;doctor doctor;env env;licenses licenses;version version;help help",
  composer:
    "install install;update update;require require;remove remove;dump-autoload autoload;create-project create project;show show;outdated outdated;validate validate;self-update self update;init init;search search;why why;why-not why not;diagnose diagnose;config config;run-script run script;exec exec;global global;licenses licenses;archive archive;audit audit;bump bump;check-platform-reqs check platform;clear-cache clear cache;depends depends;prohibits prohibits;fund fund;status status;suggests suggests;browse browse;home home;reinstall reinstall;help help",
  mvn: "clean clean;compile compile;test test;package package;install install;deploy deploy;verify verify;validate validate;site site;dependency:tree dependency tree;dependency:analyze dependency analyze;versions:display-dependency-updates dependency updates;help:effective-pom effective POM;archetype:generate generate archetype;spring-boot:run Spring Boot run;exec:java exec java;wrapper:wrapper wrapper",
  gradle:
    "build build;clean clean;test test;run run;assemble assemble;tasks tasks;dependencies dependencies;wrapper wrapper;init init;check check;jar jar;publish publish;bootRun Spring Boot run;help help;projects projects;properties properties;buildEnvironment build environment;dependencyInsight dependency insight;javadoc javadoc;compileJava compile;processResources resources;classes classes;installDist install dist;distZip dist zip;distTar dist tar",
  dotnet:
    "build build;run run;test test;publish publish;restore restore;new new project;add add reference/package;remove remove;clean clean;pack pack;tool tools;ef Entity Framework;watch watch;sln solution;nuget NuGet;list list;format format;dev-certs dev certs;user-secrets user secrets;workload workloads;sdk SDK;msbuild MSBuild;vstest VSTest;store store;help help;--info info;--version version;--list-sdks list SDKs;--list-runtimes list runtimes",
  gh: "pr pull requests;issue issues;repo repositories;run workflow runs;workflow workflows;release releases;auth auth;api API;gist gists;browse browse;codespace codespaces;secret secrets;config config;extension extensions;label labels;search search;status status;ssh-key SSH keys;gpg-key GPG keys;alias aliases;cache caches;org organizations;project projects;ruleset rulesets;variable variables;attestation attestations;completion completion;help help",
  glab: "mr merge requests;issue issues;repo repositories;ci pipelines;release releases;auth auth;api API;snippet snippets;label labels;variable variables;schedule schedules;incident incidents;user users;ssh-key SSH keys;config config;alias aliases;check-update check update;completion completion;version version;help help;cluster clusters;deploy-key deploy keys;job jobs;stack stacks;token tokens;changelog changelog;iteration iterations;milestone milestones;securefile secure files",
  tmux: "new new session;new-session new session;attach attach;attach-session attach;a attach;ls list sessions;list-sessions list sessions;kill-session kill session;kill-server kill server;detach detach;split-window split;rename-session rename;rename-window rename window;source-file source config;send-keys send keys;list-windows windows;list-panes panes;new-window new window;select-window select window;select-pane select pane;kill-window kill window;kill-pane kill pane;resize-pane resize pane;swap-pane swap pane;swap-window swap window;move-window move window;set set option;set-option set option;show show options;show-options show options;bind bind key;bind-key bind key;unbind unbind key;list-keys list keys;list-commands list commands;info info;capture-pane capture pane;save-buffer save buffer;paste-buffer paste buffer;list-buffers buffers;copy-mode copy mode;display display message;display-message display message;display-panes display panes;switch-client switch client;list-clients clients;refresh-client refresh;has-session has session;run run shell;if if shell;choose-tree choose tree;choose-session choose session;choose-window choose window;command-prompt command prompt;pipe-pane pipe pane;respawn-pane respawn pane;break-pane break pane;join-pane join pane;last-pane last pane;last-window last window;next-window next window;previous-window previous window;next-layout next layout;select-layout select layout;rotate-window rotate window;find-window find window;clear-history clear history;start start server",
  zellij:
    "attach attach;a attach;list-sessions sessions;ls sessions;kill-session kill;k kill;kill-all-sessions kill all;ka kill all;delete-session delete;d delete;delete-all-sessions delete all;da delete all;setup setup;options options;run run;r run;edit edit;e edit;action action;ac action;plugin plugin;p plugin;pipe pipe;convert-config convert config;convert-layout convert layout;convert-theme convert theme;list-aliases list aliases;help help",
  nmcli:
    "device devices;dev devices;connection connections;con connections;c connections;radio radio switches;r radio;general general status;g general;networking networking;n networking;monitor monitor;m monitor;agent agent;a agent;help help",
  hostnamectl:
    "status status;hostname hostname;set-hostname set hostname;icon-name icon;set-icon-name set icon;chassis chassis;set-chassis set chassis;deployment deployment;set-deployment set deployment;location location;set-location set location;help help",
  timedatectl:
    "status status;show show;set-time set time;set-timezone set timezone;list-timezones timezones;set-local-rtc local RTC;set-ntp enable NTP;timesync-status NTP status;show-timesync show timesync;ntp-servers NTP servers;revert revert;help help",
  localectl:
    "status status;set-locale set locale;list-locales locales;set-keymap keymap;list-keymaps keymaps;set-x11-keymap X11 keymap;list-x11-keymap-models X11 models;list-x11-keymap-layouts X11 layouts;list-x11-keymap-variants X11 variants;list-x11-keymap-options X11 options;help help",
  ufw: "allow allow rule;deny deny rule;reject reject rule;limit rate limit;delete delete rule;insert insert rule;prepend prepend rule;route route rules;status status;enable enable;disable disable;reload reload;reset reset;default default policy;logging logging;app application profiles;show show reports;version version;help help",
  nft: "list list;add add;create create;delete delete;flush flush;insert insert;replace replace;rename rename;describe describe;monitor monitor;import import;export export;destroy destroy;get get;reset reset;ruleset ruleset;table table;chain chain;rule rule;set set;map map;element element;flowtable flowtable;counter counter;quota quota;limit limit;ct ct;help help",
  "wg-quick": "up bring up;down take down;save save;strip strip",
  wg: "show show;showconf show config;set set;setconf set config;addconf add config;syncconf sync config;genkey generate private key;genpsk generate PSK;pubkey public key;help help",
  openssl:
    "req certificate request;x509 certificate;genrsa RSA key;genpkey private key;rsa RSA key tool;ec EC key tool;ecparam EC parameters;s_client TLS client;s_server TLS server;enc encrypt;dgst digest;rand random bytes;pkcs12 PKCS#12;pkcs8 PKCS#8;pkey key tool;verify verify certificate;version version;passwd hash password;speed benchmark;ciphers list ciphers;crl CRL;ca CA;dhparam DH parameters;prime prime;asn1parse ASN.1 parse;base64 base64;sha256 SHA-256;md5 MD5;list list;help help;storeutl store util;ocsp OCSP;ts timestamp;cms CMS;smime S/MIME;pkcs7 PKCS7;pkeyutl key util;dsa DSA;dsaparam DSA params;gendsa DSA key",
  certbot:
    "certonly obtain only;renew renew;run obtain and install;install install;certificates list;delete delete;revoke revoke;reconfigure reconfigure;update_account update account;register register;unregister unregister;show_account show account;enhance enhance;rollback rollback;plugins plugins;help help",
  pass: "show show;insert insert;generate generate;edit edit;rm remove;ls list;find find;grep grep;mv move;cp copy;init init;git git;help help;version version;otp OTP (ext);import import (ext)",
  virsh:
    "list list domains;start start;shutdown shutdown;destroy force off;reboot reboot;reset reset;suspend suspend;resume resume;console console;define define;undefine undefine;edit edit XML;dumpxml dump XML;dominfo info;domstate state;domstats stats;domifaddr IP addresses;domiflist interfaces;domblklist disks;net-list networks;net-start start network;net-destroy stop network;pool-list pools;pool-start start pool;vol-list volumes;vol-create-as create volume;vol-delete delete volume;snapshot-create-as snapshot;snapshot-list snapshots;snapshot-revert revert;snapshot-delete delete snapshot;autostart autostart;setmem set memory;setvcpus set vCPUs;attach-disk attach disk;detach-disk detach disk;attach-interface attach interface;detach-interface detach interface;migrate migrate;save save;restore restore;managedsave managed save;nodeinfo node info;capabilities capabilities;version version;uri URI;connect connect;help help",
  lxc: "list list;launch launch;init init;start start;stop stop;restart restart;pause pause;delete delete;exec exec;shell shell;info info;config config;image images;network networks;storage storage;profile profiles;snapshot snapshot;restore restore;copy copy;move move;file files;publish publish;remote remotes;console console;rename rename;operation operations;alias aliases;cluster cluster;project projects;query query;version version;warning warnings;monitor monitor;export export;import import;help help",
  incus:
    "list list;launch launch;init init;start start;stop stop;restart restart;pause pause;resume resume;delete delete;exec exec;shell shell;info info;config config;image images;network networks;storage storage;profile profiles;snapshot snapshots;copy copy;move move;file files;publish publish;remote remotes;console console;rename rename;operation operations;alias aliases;cluster cluster;project projects;query query;version version;warning warnings;monitor monitor;export export;import import;rebuild rebuild;webui web UI;help help",
  multipass:
    "launch launch;list list;ls list;shell shell;exec exec;start start;stop stop;restart restart;suspend suspend;delete delete;purge purge;info info;find find;mount mount;umount unmount;transfer transfer;set set;get get;alias alias;aliases aliases;unalias unalias;networks networks;snapshot snapshot;restore restore;recover recover;clone clone;authenticate authenticate;version version;help help",
  wsl: "--list list;--install install;--shutdown shutdown;--terminate terminate;--set-default set default;--set-version set version;--export export;--import import;--unregister unregister;--update update;--status status;--mount mount;--unmount unmount;--exec exec;--distribution distribution;--user user;--cd directory;--help help;--version version;--manage manage;--system system",
  winget:
    "install install;uninstall uninstall;upgrade upgrade;list list;search search;show show;source sources;export export;import import;settings settings;features features;hash hash;validate validate;pin pin;configure configure;download download;repair repair;help help",
  choco:
    "install install;uninstall uninstall;upgrade upgrade;list list;search search;info info;outdated outdated;pin pin;source sources;feature features;config config;pack pack;push push;new new;apikey API key;export export;cache cache;template templates;rule rules;help help",
  scoop:
    "install install;uninstall uninstall;update update;list list;search search;info info;bucket buckets;status status;cleanup cleanup;cache cache;hold hold;unhold unhold;reset reset;which which;checkup checkup;export export;import import;config config;prefix prefix;home home;depends depends;download download;shim shims;virustotal VirusTotal;alias alias;create create;cat cat;help help",
  nix: "build build;run run;shell shell;develop develop;flake flakes;profile profiles;search search;store store;repl REPL;eval eval;log log;why-depends why depends;path-info path info;copy copy;registry registry;upgrade-nix upgrade nix;config config;hash hash;key keys;nar NAR;print-dev-env print dev env;bundle bundle;fmt format;edit edit;derivation derivation;realisation realisation;daemon daemon;doctor doctor;help help",
  asdf: "plugin plugins;install install;uninstall uninstall;list list;latest latest;current current;set set;global global;local local;shell shell;where where;which which;exec exec;env env;reshim reshim;shim-versions shim versions;update update;info info;version version;help help",
  mise: "install install;i install;uninstall uninstall;use use;u use;ls list;list list;ls-remote remote versions;latest latest;current current;exec exec;x exec;run run;r run;tasks tasks;env env;e env;set set;unset unset;trust trust;settings settings;config config;plugins plugins;p plugins;upgrade upgrade;up upgrade;outdated outdated;prune prune;reshim reshim;where where;which which;activate activate;completion completion;doctor doctor;dr doctor;self-update self update;version version;v version;watch watch;w watch;shell shell;sh shell;link link;ln link;unuse unuse;alias alias;a alias;bin-paths bin paths;cache cache;direnv direnv;generate generate;g generate;implode implode;registry registry;sync sync;tool tool;fmt format;lock lock;backends backends;b backends;help help",
  nvm: "install install;uninstall uninstall;use use;ls list;list list;ls-remote remote versions;current current;alias alias;unalias unalias;which which;exec exec;run run;version version;version-remote remote version;deactivate deactivate;cache cache;set-colors set colours;unload unload;install-latest-npm install latest npm;reinstall-packages reinstall packages;help help",
  pyenv:
    "install install;uninstall uninstall;versions versions;version version;global global;local local;shell shell;which which;whence whence;rehash rehash;update update;doctor doctor;init init;commands commands;completions completions;exec exec;prefix prefix;root root;shims shims;version-file version file;version-name version name;version-origin version origin;virtualenv virtualenv;virtualenvs virtualenvs;activate activate;deactivate deactivate;latest latest;help help",
  rbenv:
    "install install;uninstall uninstall;versions versions;version version;global global;local local;shell shell;which which;whence whence;rehash rehash;init init;commands commands;completions completions;exec exec;prefix prefix;root root;shims shims;version-file version file;version-name version name;version-origin version origin;help help",
  terraform:
    "init init;plan plan;apply apply;destroy destroy;validate validate;fmt format;output output;state state;import import;workspace workspace;show show;refresh refresh;taint taint;untaint untaint;providers providers;graph graph;console console;login login;logout logout;get get modules;force-unlock force unlock;version version;test test;metadata metadata;modules modules;help help",
  tofu: "init init;plan plan;apply apply;destroy destroy;validate validate;fmt format;output output;state state;import import;workspace workspace;show show;refresh refresh;taint taint;untaint untaint;providers providers;graph graph;console console;login login;logout logout;get get modules;force-unlock force unlock;version version;test test;metadata metadata;help help",
  pulumi:
    "up deploy;preview preview;destroy destroy;stack stacks;config config;new new project;login login;logout logout;whoami whoami;refresh refresh;import import;state state;plugin plugins;policy policy;org org;env environments;about about;cancel cancel;console console;convert convert;install install;logs logs;package packages;schema schema;version version;watch watch;help help",
  vagrant:
    "up start;halt stop;destroy destroy;ssh ssh;status status;reload reload;provision provision;suspend suspend;resume resume;init init;box boxes;global-status global status;snapshot snapshots;package package;plugin plugins;port ports;powershell powershell;rdp RDP;ssh-config ssh config;validate validate;version version;cloud cloud;autocomplete autocomplete;login login;serve serve;winrm WinRM;winrm-config WinRM config;help help",
  packer:
    "init init;build build;validate validate;fmt format;inspect inspect;console console;fix fix;plugins plugins;hcl2_upgrade HCL2 upgrade;version version;help help",
  "ansible-vault":
    "create create;decrypt decrypt;edit edit;view view;encrypt encrypt;encrypt_string encrypt string;rekey rekey",
  aws: "s3 S3;ec2 EC2;iam IAM;lambda Lambda;ecs ECS;eks EKS;rds RDS;sts STS;ssm SSM;cloudformation CloudFormation;logs CloudWatch Logs;cloudwatch CloudWatch;route53 Route 53;ecr ECR;dynamodb DynamoDB;sqs SQS;sns SNS;kms KMS;secretsmanager Secrets Manager;configure configure;sso SSO;s3api S3 API;elbv2 ELBv2;autoscaling Auto Scaling;cloudfront CloudFront;apigateway API Gateway;events EventBridge;stepfunctions Step Functions;organizations Organizations;acm ACM;ses SES;sesv2 SESv2;cognito-idp Cognito;codebuild CodeBuild;codepipeline CodePipeline;codecommit CodeCommit;glue Glue;athena Athena;redshift Redshift;elasticache ElastiCache;efs EFS;backup Backup;batch Batch;help help",
  az: "login login;logout logout;account account;group resource groups;vm virtual machines;aks AKS;acr container registry;storage storage;network network;webapp web apps;functionapp function apps;keyvault key vault;ad Active Directory;role roles;sql SQL;cosmosdb Cosmos DB;monitor monitor;deployment deployments;resource resources;container containers;containerapp container apps;identity identities;policy policy;extension extensions;config config;configure configure;feedback feedback;find find;interactive interactive;rest REST;upgrade upgrade;version version;bicep Bicep;devops DevOps;pipelines pipelines;repos repos;boards boards;artifacts artifacts;help help",
  gcloud:
    "auth auth;config config;compute Compute Engine;container GKE;projects projects;iam IAM;run Cloud Run;functions Cloud Functions;storage Cloud Storage;sql Cloud SQL;logging logging;services services;components components;pubsub Pub/Sub;secrets Secret Manager;kms KMS;dns Cloud DNS;builds Cloud Build;artifacts Artifact Registry;scheduler Cloud Scheduler;tasks Cloud Tasks;redis Memorystore;spanner Spanner;firestore Firestore;bigtable Bigtable;dataflow Dataflow;dataproc Dataproc;ai AI;alpha alpha;beta beta;info info;init init;version version;topic topics;help help",
  doctl:
    "auth auth;account account;compute compute;kubernetes Kubernetes;k8s Kubernetes;databases databases;db databases;apps apps;registry registry;projects projects;monitoring monitoring;vpcs VPCs;serverless serverless;balance balance;billing-history billing;invoice invoices;1-click 1-click apps;version version;help help",
  flyctl:
    "launch launch;deploy deploy;status status;logs logs;apps apps;machine machines;m machines;scale scale;secrets secrets;ssh SSH;proxy proxy;postgres Postgres;pg Postgres;redis Redis;volumes volumes;vol volumes;certs certificates;ips IPs;regions regions;config config;auth auth;open open;releases releases;dashboard dashboard;doctor doctor;version version;wireguard WireGuard;orgs organizations;platform platform;tokens tokens;console console;image image;checks checks;help help",
  heroku:
    "create create;apps apps;logs logs;ps processes;config config;run run;restart restart;releases releases;addons add-ons;domains domains;certs certificates;pg Postgres;redis Redis;pipelines pipelines;git git;login login;logout logout;auth auth;access access;buildpacks buildpacks;dyno dynos;features features;labs labs;maintenance maintenance;members members;notifications notifications;orgs organizations;plugins plugins;regions regions;reviewapps review apps;sessions sessions;spaces spaces;status status;teams teams;update update;webhooks webhooks;autocomplete autocomplete;help help",
  vercel:
    "deploy deploy;dev dev;build build;env env;link link;pull pull;ls list;list list;rm remove;remove remove;logs logs;inspect inspect;domains domains;dns DNS;certs certificates;alias alias;secrets secrets;projects projects;project project;teams teams;switch switch;login login;logout logout;whoami whoami;init init;git git;promote promote;rollback rollback;redeploy redeploy;bisect bisect;integration integrations;blob blob;telemetry telemetry;help help",
  wrangler:
    "dev dev;deploy deploy;publish publish;init init;login login;logout logout;whoami whoami;kv KV;r2 R2;d1 D1;queues queues;secret secrets;tail tail;pages Pages;generate generate;delete delete;deployments deployments;rollback rollback;versions versions;triggers triggers;types types;docs docs;dispatch-namespace dispatch namespaces;mtls-certificate mTLS certificates;hyperdrive Hyperdrive;vectorize Vectorize;ai AI;workflows Workflows;pipelines Pipelines;cert certificates;check check;help help",
  vault:
    "login login;status status;read read;write write;delete delete;list list;kv KV;secrets secrets engines;auth auth methods;policy policies;token tokens;lease leases;operator operator;audit audit;agent agent;server server;namespace namespaces;plugin plugins;path-help path help;print print;ssh SSH;transit transit;pki PKI;debug debug;monitor monitor;version version;help help",
  consul:
    "agent agent;members members;catalog catalog;kv KV;services services;connect Connect;intention intentions;acl ACL;operator operator;snapshot snapshots;config config;debug debug;event events;exec exec;force-leave force leave;info info;join join;keygen keygen;keyring keyring;leave leave;lock lock;login login;logout logout;maint maintenance;monitor monitor;peering peering;reload reload;rtt RTT;tls TLS;validate validate;version version;watch watch;help help",
  nomad:
    "job jobs;run run job;stop stop job;status status;alloc allocations;node nodes;server servers;agent agent;deployment deployments;eval evaluations;namespace namespaces;var variables;acl ACL;operator operator;plan plan;validate validate;logs logs;exec exec;fs filesystem;monitor monitor;system system;quota quotas;sentinel Sentinel;setup setup;service services;volume volumes;plugin plugins;scaling scaling;recommendation recommendations;license license;tls TLS;ui UI;version version;help help",
  etcdctl:
    "get get;put put;del delete;txn transaction;watch watch;lease leases;member members;endpoint endpoints;snapshot snapshots;user users;role roles;auth auth;alarm alarms;defrag defrag;compaction compaction;move-leader move leader;check check;elect elect;lock lock;make-mirror make mirror;version version;help help",
  pm2: "start start;stop stop;restart restart;reload reload;delete delete;del delete;list list;ls list;l list;status status;logs logs;monit monitor;save save;dump save;resurrect resurrect;startup startup;unstartup unstartup;describe describe;desc describe;show describe;info describe;flush flush logs;reloadLogs reload logs;ecosystem ecosystem file;init ecosystem file;scale scale;reset reset;kill kill daemon;update update;ping ping;env env;jlist JSON list;prettylist pretty list;deploy deploy;plus PM2 Plus;link link;unlink unlink;install install module;uninstall uninstall module;set set;get get;unset unset;conf conf;report report;serve serve;attach attach;sendSignal send signal;trigger trigger;inspect inspect;id id;pid pid;create create;examples examples;help help",
  supervisorctl:
    "status status;start start;stop stop;restart restart;reread reread;update update;reload reload;tail tail;pid pid;shutdown shutdown;signal signal;clear clear logs;add add;remove remove;avail available;fg foreground;maintail main log;open open;version version;help help",
  caddy:
    "run run;start start;stop stop;reload reload;validate validate;fmt format;adapt adapt;file-server file server;reverse-proxy reverse proxy;version version;list-modules list modules;environ environment;build-info build info;hash-password hash password;respond respond;storage storage;trust trust;untrust untrust;upgrade upgrade;add-package add package;remove-package remove package;manpage manpage;completion completion;help help",
  apachectl:
    "start start;stop stop;restart restart;graceful graceful;graceful-stop graceful stop;configtest test config;status status;fullstatus full status;help help",
  cryptsetup:
    "luksFormat format LUKS;luksOpen open;open open;luksClose close;close close;luksAddKey add key;luksRemoveKey remove key;luksChangeKey change key;luksKillSlot kill slot;luksDump dump header;luksUUID UUID;luksHeaderBackup header backup;luksHeaderRestore header restore;luksSuspend suspend;luksResume resume;luksErase erase;status status;resize resize;refresh refresh;reencrypt reencrypt;isLuks is LUKS;repair repair;benchmark benchmark;config config;token tokens;convert convert;help help",
  zfs: "list list;create create;destroy destroy;snapshot snapshot;rollback rollback;clone clone;promote promote;rename rename;get get;set set;inherit inherit;mount mount;unmount unmount;umount unmount;send send;receive receive;recv receive;allow allow;unallow unallow;hold hold;release release;holds holds;diff diff;upgrade upgrade;userspace userspace;groupspace groupspace;share share;unshare unshare;bookmark bookmark;load-key load key;unload-key unload key;change-key change key;program program;wait wait;version version;help help",
  zpool:
    "list list;status status;create create;destroy destroy;import import;export export;add add;remove remove;attach attach;detach detach;replace replace;online online;offline offline;clear clear;scrub scrub;trim trim;iostat iostat;history history;get get;set set;upgrade upgrade;events events;labelclear label clear;reguid reguid;reopen reopen;split split;initialize initialize;resilver resilver;checkpoint checkpoint;sync sync;wait wait;version version;help help",
  btrfs:
    "subvolume subvolumes;filesystem filesystem;fi filesystem;device devices;scrub scrub;balance balance;check check;rescue rescue;restore restore;send send;receive receive;property properties;qgroup qgroups;quota quota;inspect-internal inspect;replace replace;version version;help help",
  parted:
    "print print;mklabel make label;mkpart make partition;rm remove;resizepart resize;set set flag;name name;unit unit;align-check align check;rescue rescue;select select;toggle toggle;version version;quit quit;help help",
  rclone:
    "copy copy;sync sync;move move;ls list;lsd list dirs;lsl list long;lsf list formatted;lsjson list JSON;mount mount;config config;check check;size size;delete delete;purge purge;mkdir mkdir;rmdir rmdir;rmdirs rmdirs;cat cat;copyto copy to;moveto move to;copyurl copy URL;serve serve;listremotes remotes;about about;cleanup cleanup;dedupe dedupe;md5sum MD5;sha1sum SHA1;hashsum hashsum;touch touch;tree tree;version version;rc remote control;rcd rc daemon;selfupdate self update;bisync bisync;backend backend;link link;ncdu ncdu;obscure obscure;test test;help help",
  http: "GET GET;POST POST;PUT PUT;DELETE DELETE;PATCH PATCH;HEAD HEAD;OPTIONS OPTIONS",
  ip: "addr addresses;address addresses;a addresses;link interfaces;l interfaces;route routing table;r routing table;ro routing table;neigh ARP / neighbours;neighbour neighbours;n neighbours;netns network namespaces;rule policy routing;tunnel tunnels;tuntap tun/tap;maddr multicast;maddress multicast;mroute multicast routes;monitor monitor;xfrm IPsec;vrf VRF;sr segment routing;tcp_metrics TCP metrics;token tokens;macsec MACsec;mptcp MPTCP;stats statistics;help help",
  "fail2ban-client":
    "status status;reload reload;start start;stop stop;set set;get get;ping ping;banned banned;unban unban;restart restart;version version;help help;flushlogs flush logs;add add jail;echo echo",
  semanage:
    "port ports;fcontext file contexts;boolean booleans;login logins;user users;permissive permissive domains;module modules;interface interfaces;node nodes;dontaudit dontaudit;ibpkey IB pkeys;ibendport IB endports;import import;export export",
  "systemd-analyze":
    "time boot time;blame blame;critical-chain critical chain;plot plot SVG;dot dot graph;dump dump;unit-files unit files;unit-paths unit paths;exit-status exit status;capability capabilities;condition condition;syscall-filter syscall filter;filesystems filesystems;calendar calendar;timestamp timestamp;timespan timespan;cat-config cat config;compare-versions compare versions;verify verify;security security;inspect-elf inspect ELF;malloc malloc;fdstore fdstore;image-policy image policy;pcrs PCRs;srk SRK;architectures architectures;help help",
  loginctl:
    "list-sessions sessions;session-status session status;show-session show session;activate activate;lock-session lock;unlock-session unlock;lock-sessions lock all;unlock-sessions unlock all;terminate-session terminate;kill-session kill;list-users users;user-status user status;show-user show user;enable-linger enable linger;disable-linger disable linger;terminate-user terminate user;kill-user kill user;list-seats seats;seat-status seat status;show-seat show seat;attach attach;flush-devices flush devices;terminate-seat terminate seat;help help",
  udevadm:
    "info info;trigger trigger;settle settle;control control;monitor monitor;test test;test-builtin test builtin;verify verify;wait wait;lock lock;help help",
  resolvectl:
    "query query;status status;statistics statistics;reset-statistics reset statistics;flush-caches flush caches;reset-server-features reset server features;dns DNS servers;domain domains;default-route default route;llmnr LLMNR;mdns mDNS;dnssec DNSSEC;dnsovertls DNS over TLS;nta NTA;revert revert;monitor monitor;show-cache show cache;show-server-state server state;log-level log level;service service;openpgp OpenPGP;tlsa TLSA;help help",
  busctl:
    "list list;status status;monitor monitor;capture capture;tree tree;introspect introspect;call call;emit emit;get-property get property;set-property set property;wait wait;help help",
  svn: "checkout checkout;co checkout;update update;up update;commit commit;ci commit;add add;delete delete;del delete;rm delete;status status;st status;diff diff;di diff;log log;info info;revert revert;merge merge;switch switch;sw switch;copy copy;cp copy;move move;mv move;mkdir mkdir;cat cat;list list;ls list;blame blame;praise blame;annotate blame;cleanup cleanup;export export;import import;lock lock;unlock unlock;propget propget;pg propget;propset propset;ps propset;proplist proplist;pl proplist;propdel propdel;pd propdel;propedit propedit;pe propedit;resolve resolve;resolved resolved;patch patch;relocate relocate;upgrade upgrade;changelist changelist;cl changelist;mergeinfo mergeinfo;auth auth;help help",
  hg: "add add;addremove addremove;annotate annotate;blame blame;archive archive;backout backout;bisect bisect;bookmarks bookmarks;bookmark bookmark;branch branch;branches branches;bundle bundle;cat cat;clone clone;commit commit;ci commit;config config;copy copy;cp copy;diff diff;export export;files files;forget forget;graft graft;grep grep;heads heads;help help;identify identify;id identify;import import;incoming incoming;in incoming;init init;log log;history log;manifest manifest;merge merge;outgoing outgoing;out outgoing;paths paths;phase phase;pull pull;push push;recover recover;remove remove;rm remove;rename rename;mv rename;resolve resolve;revert revert;root root;serve serve;shelve shelve;unshelve unshelve;status status;st status;summary summary;sum summary;tag tag;tags tags;tip tip;unbundle unbundle;update update;up update;checkout update;co update;verify verify;version version;purge purge;rebase rebase;strip strip;histedit histedit;absorb absorb;amend amend;uncommit uncommit;split split;fold fold;show show",
  yq: "eval evaluate;e evaluate;eval-all evaluate all;ea evaluate all;-i in place;-o output format;-p input format;-P pretty print;-r raw output;-j JSON output;-I indent;-n null input;-e exit status;-s split;-M no colour;-C colour;--front-matter front matter;-v verbose;-V version;--help help",
  direnv:
    "allow allow;permit allow;grant allow;block block;deny block;revoke block;disallow block;reload reload;edit edit;exec exec;export export;hook hook;prune prune;status status;stdlib stdlib;fetchurl fetch URL;version version;help help",
  minikube:
    "start start;stop stop;delete delete;status status;dashboard dashboard;service service;tunnel tunnel;ssh SSH;ip IP;kubectl kubectl;addons addons;config config;profile profiles;node nodes;image images;mount mount;logs logs;update-check update check;update-context update context;version version;cache cache;cp copy;docker-env docker env;podman-env podman env;pause pause;unpause unpause;completion completion;options options;help help",
  kind: "create create;delete delete;get get;load load;export export;build build;version version;completion completion;help help",
  k3s: "server server;agent agent;kubectl kubectl;crictl crictl;ctr ctr;etcd-snapshot etcd snapshot;secrets-encrypt secrets encrypt;certificate certificates;token tokens;completion completion;check-config check config;help help",
  kustomize:
    "build build;create create;edit edit;cfg config;fn functions;localize localize;version version;completion completion;help help",
  k9s: "info info;version version;help help",
  skopeo:
    "copy copy;inspect inspect;delete delete;list-tags list tags;login login;logout logout;sync sync;manifest-digest manifest digest;standalone-sign sign;standalone-verify verify;generate-sigstore-key generate sigstore key;help help",
  buildah:
    "from from;run run;copy copy;add add;config config;commit commit;bud build;build build;images images;containers containers;rm remove;rmi remove image;push push;pull pull;tag tag;inspect inspect;mount mount;umount unmount;unmount unmount;login login;logout logout;manifest manifests;info info;version version;prune prune;rename rename;source source;unshare unshare;help help",
  nerdctl:
    "run run;ps list;images images;pull pull;push push;build build;exec exec;logs logs;stop stop;start start;restart restart;rm remove;rmi remove image;kill kill;inspect inspect;compose compose;volume volumes;network networks;system system;container containers;image images;stats stats;top processes;cp copy;attach attach;commit commit;tag tag;login login;logout logout;save save;load load;export export;import import;history history;port port;rename rename;pause pause;unpause unpause;wait wait;diff diff;events events;info info;version version;namespace namespaces;apparmor AppArmor;builder builder;ipfs IPFS;internal internal;create create;update update;help help",
  crictl:
    "ps list containers;pods list pods;images images;img images;image images;pull pull;rmi remove image;inspect inspect;inspectp inspect pod;inspecti inspect image;logs logs;exec exec;attach attach;start start;stop stop;rm remove;stopp stop pod;rmp remove pod;runp run pod;run run;create create;port-forward port forward;stats stats;statsp pod stats;info info;version version;config config;update update;imagefsinfo image fs info;events events;runtime-config runtime config;checkpoint checkpoint;completion completion;help help",
  ctr: "containers containers;c containers;container containers;content content;events events;event events;images images;image images;i images;leases leases;namespaces namespaces;namespace namespaces;ns namespaces;pprof pprof;run run;snapshots snapshots;snapshot snapshots;tasks tasks;t tasks;task tasks;install install;oci OCI;sandboxes sandboxes;sandbox sandboxes;sb sandboxes;info info;deprecations deprecations;plugins plugins;plugin plugins;version version;help help",
  influx:
    "query query;write write;bucket buckets;org organizations;user users;auth auth;config configs;setup setup;task tasks;telegrafs Telegraf;dashboards dashboards;delete delete;export export;apply apply;stacks stacks;template templates;backup backup;restore restore;ping ping;secret secrets;v1 v1 compat;remote remotes;replication replications;scripts scripts;server-config server config;completion completion;version version;help help",
  traefik: "healthcheck healthcheck;version version;help help",
  hugo: "server server;serve server;new new;build build;mod modules;config config;convert convert;deploy deploy;env env;gen generate;import import;list list;version version;completion completion;help help",
  mkdocs:
    "serve serve;build build;new new;gh-deploy GitHub Pages deploy;get-deps get deps;help help",
  sqlx: "database database;db database;migrate migrations;mig migrations;prepare prepare;completions completions;help help",
  prisma:
    "init init;generate generate;db database;migrate migrations;studio studio;validate validate;format format;version version;debug debug;platform platform;help help",
  alembic:
    "init init;revision revision;upgrade upgrade;downgrade downgrade;current current;history history;heads heads;branches branches;show show;merge merge;stamp stamp;edit edit;check check;ensure_version ensure version;list_templates list templates",
  bw: "login login;logout logout;lock lock;unlock unlock;sync sync;generate generate;encode encode;config config;update update;completion completion;status status;serve serve;list list;get get;create create;edit edit;delete delete;restore restore;move move;confirm confirm;import import;export export;share share;send Send;receive receive;device-approval device approval;help help",
  op: "signin sign in;signout sign out;account accounts;item items;vault vaults;user users;group groups;document documents;read read;inject inject;run run;plugin plugins;connect Connect;events-api Events API;service-account service accounts;whoami whoami;update update;completion completion;help help",
  fly: "launch launch;deploy deploy;status status;logs logs;apps apps;machine machines;m machines;scale scale;secrets secrets;ssh SSH;proxy proxy;postgres Postgres;pg Postgres;redis Redis;volumes volumes;vol volumes;certs certificates;ips IPs;regions regions;config config;auth auth;open open;releases releases;dashboard dashboard;doctor doctor;version version;wireguard WireGuard;orgs organizations;platform platform;tokens tokens;console console;image image;checks checks;help help",
  netlify:
    "deploy deploy;dev dev;init init;link link;unlink unlink;login login;logout logout;status status;sites sites;env env;functions functions;open open;build build;serve serve;watch watch;addons add-ons;api API;blobs blobs;completion completion;integration integrations;logs logs;recipes recipes;switch switch;help help",
  gsutil:
    "ls list;cp copy;mv move;rm remove;rsync sync;cat cat;du disk usage;mb make bucket;rb remove bucket;acl ACL;iam IAM;cors CORS;lifecycle lifecycle;logging logging;notification notifications;versioning versioning;web website;hash hash;stat stat;setmeta set metadata;signurl signed URL;compose compose;defacl default ACL;defstorageclass default storage class;kms KMS;label labels;requesterpays requester pays;retention retention;ubla uniform bucket-level access;pap public access prevention;autoclass autoclass;rpo RPO;test test;version version;help help",
  s3cmd:
    "ls list;la list all;put put;get get;del delete;rm delete;mb make bucket;rb remove bucket;sync sync;cp copy;mv move;modify modify;info info;du disk usage;setacl set ACL;setpolicy set policy;delpolicy delete policy;setcors set CORS;delcors delete CORS;payer requester pays;multipart multipart;abortmp abort multipart;listmp list multipart;accesslog access log;sign sign;signurl signed URL;fixbucket fix bucket;ws-create create website;ws-delete delete website;ws-info website info;expire expire;setlifecycle set lifecycle;getlifecycle get lifecycle;dellifecycle delete lifecycle;help help",
  ipmitool:
    "power power;chassis chassis;sensor sensors;sdr SDR;sel event log;lan LAN;user users;fru FRU;mc management controller;sol serial over LAN;bmc BMC;channel channels;session sessions;event events;raw raw;pef PEF;delloem Dell OEM;shell shell;exec exec;set set;hpm HPM;dcmi DCMI;nm node manager;echo echo;help help",
  "nvidia-smi":
    "dmon device monitor;pmon process monitor;topo topology;nvlink NVLink;clocks clocks;vgpu vGPU;mig MIG;encodersessions encoder sessions;fbcsessions FBC sessions;drain drain;boost-slider boost slider;power-hint power hint;base-clocks base clocks;conf-compute confidential compute;gpu-reset GPU reset",
  "qemu-img":
    "create create;convert convert;info info;resize resize;snapshot snapshot;check check;commit commit;rebase rebase;compare compare;amend amend;bench bench;bitmap bitmap;dd dd;map map;measure measure;help help",
  asciinema: "rec record;play play;upload upload;auth auth;cat cat;help help",
  fc: "-l list;-n no numbers;-r reverse;-e editor;-s substitute",
  tig: "status status view;log log view;blame blame;grep grep;refs refs;stash stash;show show;--all all refs;-C directory",
  aptitude:
    "install install;remove remove;purge purge;update update;upgrade upgrade;safe-upgrade safe upgrade;full-upgrade full upgrade;dist-upgrade dist upgrade;search search;show show;why why;why-not why not;hold hold;unhold unhold;markauto mark auto;unmarkauto unmark auto;forbid-version forbid version;clean clean;autoclean autoclean;forget-new forget new;changelog changelog;download download;source source;build-dep build dep;reinstall reinstall;keep keep;keep-all keep all;versions versions",
  "nix-env":
    "-i install;--install install;-u upgrade;--upgrade upgrade;-e uninstall;--uninstall uninstall;-q query;--query query;-qa query available;-qaP query available with paths;--set set;--set-flag set flag;--switch-profile switch profile;-S switch profile;--list-generations list generations;--delete-generations delete generations;--switch-generation switch generation;-G switch generation;--rollback rollback;-p profile;--profile profile;-f file;--file file;-A attribute;--attr attribute;-b prebuilt only;--dry-run dry run;--preserve-installed preserve installed;-P preserve installed;--remove-all remove all;-r remove all;--from-expression from expression;-E from expression;--from-profile from profile;--always always;-s status;--status status;-c compare versions;--compare-versions compare versions;--description description;--drv-path drv path;--out-path out path;--meta meta;--json JSON;--xml XML;--available available;--installed installed;--system system;--help help;--version version",
  pkg: "install install;delete delete;remove delete;update update;upgrade upgrade;search search;info info;query query;which which;check check;clean clean;autoremove autoremove;lock lock;unlock unlock;audit audit;stats stats;version version;fetch fetch;add add;create create;repo repo;register register;set set;shell shell;shlib shlib;ssh ssh;updating updating;annotate annotate;alias alias;backup backup;bootstrap bootstrap;config config;convert convert;help help;plugins plugins;rquery rquery;triggers triggers",
  opkg: "install install;remove remove;update update;upgrade upgrade;list list;list-installed list installed;list-upgradable list upgradable;info info;status status;files files;search search;find find;download download;configure configure;print-architecture print arch;depends depends;whatdepends whatdepends;whatprovides whatprovides;whatconflicts whatconflicts;whatreplaces whatreplaces;flag flag;compare-versions compare versions",
};

function parseFlags(spec: string | undefined): Flag[] {
  if (!spec) return [];
  const out: Flag[] = [];
  for (const item of spec.split(";")) {
    const sp = item.indexOf(" ");
    if (sp === -1) {
      if (item) out.push({ name: item, desc: "" });
    } else {
      out.push({ name: item.slice(0, sp), desc: item.slice(sp + 1) });
    }
  }
  return out;
}

const optionCache = new Map<string, Flag[]>();
const subCache = new Map<string, Flag[]>();

export function optionsFor(command: string): Flag[] {
  let v = optionCache.get(command);
  if (!v) {
    v = parseFlags(OPTIONS[command]);
    optionCache.set(command, v);
  }
  return v;
}

export function subcommandsFor(command: string): Flag[] {
  let v = subCache.get(command);
  if (!v) {
    v = parseFlags(SUB[command]);
    subCache.set(command, v);
  }
  return v;
}
