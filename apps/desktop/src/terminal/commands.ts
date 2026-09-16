// Offline command dictionary for terminal autocomplete: common Unix / Linux /
// macOS / Windows commands with one-line descriptions, read from the
// catalogue in `crates/termoso-core/assets/autocomplete` that the Android client shares
// (`name|description[|d|n]` — `d` = takes directories, `n` = no paths).
// Frequent options and subcommands live in `commandFlags.ts`. It is a hint
// list, not a manual — anything the shell knows still works as usual.

import ROWS from "../../../../crates/termoso-core/assets/autocomplete/commands.txt?raw";
import WRAPPER_ROWS from "../../../../crates/termoso-core/assets/autocomplete/wrappers.txt?raw";

export type PathKind = "any" | "dir" | "none";

export interface CommandSpec {
  name: string;
  desc: string;
  paths: PathKind;
}

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
export const WRAPPERS = new Set(WRAPPER_ROWS.split("\n").filter(Boolean));
