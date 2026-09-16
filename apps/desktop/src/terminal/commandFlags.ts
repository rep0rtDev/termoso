// Frequent flags and first-level subcommands of frequently used commands, for
// the terminal autocomplete; the tables live in `crates/termoso-core/assets/autocomplete` and are
// shared with the Android client. Format: `flag description;flag description;…`.

import OPTION_ROWS from "../../../../crates/termoso-core/assets/autocomplete/options.txt?raw";
import SUB_ROWS from "../../../../crates/termoso-core/assets/autocomplete/subcommands.txt?raw";

export interface Flag {
  name: string;
  desc: string;
}

/** `command|flag description;flag description;…` per line. */
function table(rows: string): Record<string, string> {
  const out: Record<string, string> = {};
  for (const line of rows.split("\n")) {
    const sep = line.indexOf("|");
    if (sep > 0) out[line.slice(0, sep)] = line.slice(sep + 1);
  }
  return out;
}

const OPTIONS = table(OPTION_ROWS);
const SUB = table(SUB_ROWS);

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
