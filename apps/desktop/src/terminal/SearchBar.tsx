import { useCallback, useEffect, useRef, useState } from "react";
import {
  Checkbox,
  FormControlLabel,
  IconButton,
  InputBase,
  Paper,
  Stack,
  Tooltip,
  Typography,
} from "@mui/material";
import KeyboardArrowUpRoundedIcon from "@mui/icons-material/KeyboardArrowUpRounded";
import KeyboardArrowDownRoundedIcon from "@mui/icons-material/KeyboardArrowDownRounded";
import CloseRoundedIcon from "@mui/icons-material/CloseRounded";
import type { ISearchOptions } from "@xterm/addon-search";
import type { Uuid } from "@/ipc/types";
import { emerald } from "@/theme/theme";
import { focusPane, getRuntime } from "./store";

interface Props {
  paneId: Uuid;
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

export function SearchBar({ paneId, onClose }: Props) {
  const [query, setQuery] = useState("");
  const [caseSensitive, setCaseSensitive] = useState(false);
  const [regex, setRegex] = useState(false);
  const [results, setResults] = useState<{ index: number; count: number } | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  const options = useCallback(
    (incremental: boolean): ISearchOptions => ({ caseSensitive, regex, incremental, decorations }),
    [caseSensitive, regex],
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
    rt.search.findNext(query, options(true));
  }, [query, options, paneId]);

  const next = () => query && getRuntime(paneId)?.search.findNext(query, options(false));
  const prev = () => query && getRuntime(paneId)?.search.findPrevious(query, options(false));
  const close = () => {
    onClose();
    focusPane(paneId);
  };

  return (
    <Paper
      elevation={4}
      sx={{
        position: "absolute",
        top: 8,
        right: 16,
        zIndex: 5,
        px: 1,
        py: 0.5,
        display: "flex",
        alignItems: "center",
        gap: 0.5,
        border: 1,
        borderColor: "divider",
      }}
    >
      <InputBase
        inputRef={inputRef}
        value={query}
        onChange={(e) => {
          setQuery(e.target.value);
          if (!e.target.value) setResults(null);
        }}
        onKeyDown={(e) => {
          if (e.key === "Enter") {
            e.preventDefault();
            if (e.shiftKey) void prev();
            else void next();
          } else if (e.key === "Escape") {
            e.preventDefault();
            close();
          }
        }}
        placeholder="Find"
        sx={{ width: 220, px: 1, fontSize: 13 }}
      />
      <Typography
        variant="caption"
        color="text.secondary"
        sx={{ minWidth: 48, textAlign: "right" }}
      >
        {query ? (results ? `${results.index}/${results.count}` : "0/0") : ""}
      </Typography>
      <Stack direction="row" sx={{ ml: 0.5 }}>
        <Tooltip title="Match case">
          <FormControlLabel
            sx={{ mr: 0 }}
            control={
              <Checkbox
                size="small"
                checked={caseSensitive}
                onChange={(e) => setCaseSensitive(e.target.checked)}
              />
            }
            label={<Typography variant="caption">Aa</Typography>}
          />
        </Tooltip>
        <Tooltip title="Regular expression">
          <FormControlLabel
            sx={{ mr: 0 }}
            control={
              <Checkbox size="small" checked={regex} onChange={(e) => setRegex(e.target.checked)} />
            }
            label={<Typography variant="caption">.*</Typography>}
          />
        </Tooltip>
      </Stack>
      <IconButton size="small" onClick={() => void prev()} aria-label="Previous match">
        <KeyboardArrowUpRoundedIcon fontSize="small" />
      </IconButton>
      <IconButton size="small" onClick={() => void next()} aria-label="Next match">
        <KeyboardArrowDownRoundedIcon fontSize="small" />
      </IconButton>
      <IconButton size="small" onClick={close} aria-label="Close search">
        <CloseRoundedIcon fontSize="small" />
      </IconButton>
    </Paper>
  );
}
