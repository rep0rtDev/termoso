import { useCallback, useEffect, useRef, useState } from "react";
import { Box, IconButton, InputBase, Stack, Tooltip, Typography } from "@mui/material";
import SearchRoundedIcon from "@mui/icons-material/SearchRounded";
import KeyboardArrowUpRoundedIcon from "@mui/icons-material/KeyboardArrowUpRounded";
import KeyboardArrowDownRoundedIcon from "@mui/icons-material/KeyboardArrowDownRounded";
import type { ISearchOptions } from "@xterm/addon-search";
import type { Uuid } from "@/ipc/types";
import { emerald, monoFontFamily } from "@/theme/theme";
import { focusPane, getRuntime } from "./store";
import { tr } from "@/i18n";

interface Props {
  paneId: Uuid;
  /** Escape in the field. */
  onClose: () => void;
}

const decorations: ISearchOptions["decorations"] = {
  matchBackground: "#F2C94C55",
  matchBorder: "#F2C94C",
  matchOverviewRuler: "#F2C94C",
  activeMatchBackground: `${emerald.main}88`,
  activeMatchBorder: emerald.main,
  activeMatchColorOverviewRuler: emerald.main,
};

/**
 * Buffer search for one pane: field with previous / next (Termius layout) plus
 * match case, whole word and regex toggles and a result counter.
 */
export function SearchBar({ paneId, onClose }: Props) {
  const [query, setQuery] = useState("");
  const [caseSensitive, setCaseSensitive] = useState(false);
  const [wholeWord, setWholeWord] = useState(false);
  const [regex, setRegex] = useState(false);
  const [results, setResults] = useState<{ index: number; count: number } | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const lastOptions = useRef<(incremental: boolean) => ISearchOptions>(null);

  const options = useCallback(
    (incremental: boolean): ISearchOptions => ({
      caseSensitive,
      wholeWord,
      regex,
      incremental,
      decorations,
    }),
    [caseSensitive, wholeWord, regex],
  );

  useEffect(() => {
    const rt = getRuntime(paneId);
    if (!rt) return;
    const d = rt.search.onDidChangeResults((r) => {
      setResults(r.resultCount > 0 ? { index: r.resultIndex + 1, count: r.resultCount } : null);
    });
    return () => d.dispose();
  }, [paneId]);

  useEffect(() => {
    inputRef.current?.focus();
    return () => {
      getRuntime(paneId)?.search.clearDecorations();
    };
  }, [paneId]);

  useEffect(() => {
    const rt = getRuntime(paneId);
    if (!rt) return;
    if (!query) {
      rt.search.clearDecorations();
      return;
    }
    // The addon only re-highlights when the term changes, so drop its cache
    // when the options (case / whole word / regex) change.
    if (lastOptions.current !== options) rt.search.clearDecorations();
    lastOptions.current = options;
    rt.search.findNext(query, options(true));
  }, [query, options, paneId]);

  const next = () => query && getRuntime(paneId)?.search.findNext(query, options(false));
  const prev = () => query && getRuntime(paneId)?.search.findPrevious(query, options(false));

  return (
    <Stack sx={{ gap: 0.75 }}>
      <Stack
        direction="row"
        sx={{
          alignItems: "center",
          gap: 0.5,
          pl: 1,
          pr: 0.25,
          height: 32,
          borderRadius: 1.5,
          bgcolor: "surface.highest",
        }}
      >
        <SearchRoundedIcon sx={{ fontSize: 18, color: "text.secondary" }} />
        <InputBase
          inputRef={inputRef}
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              e.preventDefault();
              if (e.shiftKey) void prev();
              else void next();
            } else if (e.key === "Escape") {
              e.preventDefault();
              onClose();
              focusPane(paneId);
            }
          }}
          placeholder={tr("Search")}
          sx={{ flex: 1, minWidth: 0, fontSize: 13 }}
          inputProps={{ "aria-label": tr("Search in terminal") }}
        />
        <IconButton size="small" onClick={() => void prev()} aria-label={tr("Previous match")}>
          <KeyboardArrowUpRoundedIcon sx={{ fontSize: 18 }} />
        </IconButton>
        <IconButton size="small" onClick={() => void next()} aria-label={tr("Next match")}>
          <KeyboardArrowDownRoundedIcon sx={{ fontSize: 18 }} />
        </IconButton>
      </Stack>
      <Stack direction="row" sx={{ alignItems: "center", gap: 0.25, px: 0.5 }}>
        <SearchToggle
          title={tr("Match case")}
          on={caseSensitive}
          onClick={() => setCaseSensitive((v) => !v)}
        >
          {tr("Aa")}
        </SearchToggle>
        <SearchToggle
          title={tr("Whole word")}
          on={wholeWord}
          onClick={() => setWholeWord((v) => !v)}
        >
          <Box component="span" sx={{ textDecoration: "underline" }}>
            {"ab"}
          </Box>
        </SearchToggle>
        <SearchToggle
          title={tr("Regular expression")}
          on={regex}
          onClick={() => setRegex((v) => !v)}
        >
          .*
        </SearchToggle>
        <Box sx={{ flex: 1 }} />
        <Typography variant="caption" color="text.secondary">
          {query
            ? results
              ? tr("{index} of {count}", { index: results.index, count: results.count })
              : tr("No results")
            : ""}
        </Typography>
      </Stack>
    </Stack>
  );
}

function SearchToggle({
  title,
  on,
  onClick,
  children,
}: {
  title: string;
  on: boolean;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <Tooltip title={title} enterDelay={600}>
      <IconButton
        size="small"
        aria-label={title}
        aria-pressed={on}
        onClick={onClick}
        sx={{
          width: 26,
          height: 22,
          borderRadius: 1,
          fontSize: 11,
          fontWeight: 600,
          fontFamily: monoFontFamily,
          color: on ? "primary.main" : "text.secondary",
          bgcolor: on ? "rgba(43,184,132,0.18)" : "transparent",
        }}
      >
        {children}
      </IconButton>
    </Tooltip>
  );
}
