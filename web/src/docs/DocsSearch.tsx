import { useEffect, useMemo, useState } from "react";
import {
  Box,
  Dialog,
  InputBase,
  List,
  ListItemButton,
  ListItemText,
  Typography,
} from "@mui/material";
import SearchRoundedIcon from "@mui/icons-material/SearchRounded";
import { useNavigate } from "react-router";
import { hrefOf, search, type SearchHit } from "./manifest";

export function useSearchShortcut(open: () => void) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        open();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open]);
}

export function DocsSearch({ open, onClose }: { open: boolean; onClose: () => void }) {
  const [query, setQuery] = useState("");
  const [cursor, setCursor] = useState(0);
  const navigate = useNavigate();
  const hits = useMemo(() => search(query), [query]);

  const go = (h: SearchHit) => {
    onClose();
    void navigate(hrefOf(h.page) + h.anchor);
  };

  return (
    <Dialog
      open={open}
      onClose={onClose}
      fullWidth
      maxWidth="sm"
      slotProps={{
        paper: { sx: { mt: { xs: 2, sm: 10 }, alignSelf: "flex-start", overflow: "hidden" } },
      }}
    >
      <Box
        sx={{
          display: "flex",
          alignItems: "center",
          gap: 1.5,
          px: 2,
          height: 56,
          borderBottom: 1,
          borderColor: "border.light",
        }}
      >
        <SearchRoundedIcon sx={{ color: "text.secondary" }} />
        <InputBase
          autoFocus
          fullWidth
          placeholder="Search the docs…"
          value={query}
          onChange={(e) => {
            setQuery(e.target.value);
            setCursor(0);
          }}
          onKeyDown={(e) => {
            if (e.key === "ArrowDown") {
              e.preventDefault();
              setCursor((c) => Math.min(c + 1, hits.length - 1));
            } else if (e.key === "ArrowUp") {
              e.preventDefault();
              setCursor((c) => Math.max(c - 1, 0));
            } else if (e.key === "Enter" && hits[cursor]) {
              go(hits[cursor]);
            }
          }}
          sx={{ fontSize: "1rem" }}
        />
        <Box
          component="kbd"
          sx={{
            px: 0.75,
            py: 0.25,
            borderRadius: 1,
            border: 1,
            borderColor: "border.basic",
            fontSize: 11,
            color: "text.secondary",
            fontFamily: "inherit",
          }}
        >
          Esc
        </Box>
      </Box>
      {query.trim().length >= 2 && hits.length === 0 && (
        <Typography variant="body2" color="text.secondary" sx={{ px: 2.5, py: 3 }}>
          Nothing matches “{query}”.
        </Typography>
      )}
      {hits.length > 0 && (
        <List dense disablePadding sx={{ maxHeight: 420, overflowY: "auto", py: 1 }}>
          {hits.map((h, i) => (
            <ListItemButton
              key={hrefOf(h.page) + h.anchor}
              selected={i === cursor}
              onMouseEnter={() => setCursor(i)}
              onClick={() => go(h)}
              sx={{ px: 2.5, alignItems: "flex-start" }}
            >
              <ListItemText
                primary={
                  <Box sx={{ display: "flex", gap: 1, alignItems: "baseline", minWidth: 0 }}>
                    <Typography variant="subtitle2" noWrap>
                      {h.heading ?? h.page.title}
                    </Typography>
                    {h.heading && (
                      <Typography variant="caption" color="text.secondary" noWrap>
                        {h.page.title}
                      </Typography>
                    )}
                  </Box>
                }
                secondary={h.snippet}
                slotProps={{
                  secondary: {
                    sx: {
                      display: "-webkit-box",
                      WebkitLineClamp: 2,
                      WebkitBoxOrient: "vertical",
                      overflow: "hidden",
                    },
                  },
                }}
              />
            </ListItemButton>
          ))}
        </List>
      )}
    </Dialog>
  );
}
