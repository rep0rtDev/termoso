import { IconButton, ListItemText, Menu, MenuItem, Tooltip } from "@mui/material";
import TranslateRoundedIcon from "@mui/icons-material/TranslateRounded";
import { useState } from "react";
import { LANGUAGES, LOCALE_NAMES, language, setLanguage, tr, type Language } from "@/i18n";

const label = (l: Language) => (l === "system" ? tr("System") : LOCALE_NAMES[l]);

/** UI language picker. Strings are resolved when modules load, so a change reloads the page. */
export function LanguageToggle() {
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);
  const current = language();
  return (
    <>
      <Tooltip title={tr("Language")}>
        <IconButton onClick={(e) => setAnchor(e.currentTarget)} aria-label={tr("Language")}>
          <TranslateRoundedIcon fontSize="small" />
        </IconButton>
      </Tooltip>
      <Menu open={anchor !== null} anchorEl={anchor} onClose={() => setAnchor(null)}>
        {LANGUAGES.map((l) => (
          <MenuItem
            key={l}
            selected={l === current}
            onClick={() => {
              setAnchor(null);
              if (l === current) return;
              setLanguage(l);
              window.location.reload();
            }}
          >
            <ListItemText>{label(l)}</ListItemText>
          </MenuItem>
        ))}
      </Menu>
    </>
  );
}
