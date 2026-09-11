import { useState } from "react";
import { Box, ButtonBase, ToggleButton, ToggleButtonGroup, Typography } from "@mui/material";
import { useColorScheme } from "@mui/material";
import CheckRoundedIcon from "@mui/icons-material/CheckRounded";
import { SectionCard } from "@/components/ui";
import {
  AUTO_THEME,
  resolveTerminalTheme,
  terminalThemes,
  type TerminalTheme,
} from "@/terminal/themes";

type Filter = "all" | "dark" | "light";

/** Grid of colour scheme cards; `auto` follows the app theme with the Termoso pair. */
export function ThemeGallery({
  value,
  onChange,
}: {
  value: string;
  onChange: (id: string) => void;
}) {
  const [filter, setFilter] = useState<Filter>("all");
  const { mode, systemMode } = useColorScheme();
  const scheme = (mode === "system" ? systemMode : mode) === "light" ? "light" : "dark";
  const auto = resolveTerminalTheme(AUTO_THEME, scheme);
  const shown = terminalThemes.filter(
    (t) => filter === "all" || (filter === "dark" ? t.dark : !t.dark),
  );

  return (
    <SectionCard
      title="Colour scheme"
      action={
        <ToggleButtonGroup
          exclusive
          size="small"
          value={filter}
          onChange={(_, v: Filter | null) => v && setFilter(v)}
        >
          <ToggleButton value="all">All</ToggleButton>
          <ToggleButton value="dark">Dark</ToggleButton>
          <ToggleButton value="light">Light</ToggleButton>
        </ToggleButtonGroup>
      }
    >
      <Box
        sx={{
          display: "grid",
          gridTemplateColumns: "repeat(auto-fill, minmax(150px, 1fr))",
          gap: 1.25,
        }}
      >
        {filter === "all" && (
          <ThemeCard
            theme={auto}
            label="Auto"
            hint="Follows app theme"
            selected={value === AUTO_THEME}
            onClick={() => onChange(AUTO_THEME)}
          />
        )}
        {shown.map((t) => (
          <ThemeCard
            key={t.id}
            theme={t}
            label={t.name}
            selected={value === t.id}
            onClick={() => onChange(t.id)}
          />
        ))}
      </Box>
    </SectionCard>
  );
}

const SAMPLE: { color: number; text: string }[][] = [
  [
    { color: 2, text: "user" },
    { color: 7, text: "@" },
    { color: 4, text: "host" },
    { color: 7, text: " ~ " },
    { color: 5, text: "$ " },
    { color: 7, text: "ls -la" },
  ],
  [
    { color: 3, text: "drwxr-x " },
    { color: 6, text: "src " },
    { color: 1, text: "main.rs " },
    { color: 7, text: "README" },
  ],
];

export function ThemeCard({
  theme,
  label,
  hint,
  selected,
  compact = false,
  onClick,
}: {
  theme: TerminalTheme;
  label: string;
  hint?: string;
  selected: boolean;
  /** Shorter preview for narrow columns (terminal side panel). */
  compact?: boolean;
  onClick: () => void;
}) {
  return (
    <ButtonBase
      onClick={onClick}
      aria-pressed={selected}
      sx={{
        display: "flex",
        flexDirection: "column",
        alignItems: "stretch",
        textAlign: "left",
        borderRadius: 1.5,
        overflow: "hidden",
        bgcolor: "surface.highest",
        outline: selected ? 2 : 0,
        outlineColor: "primary.main",
        outlineOffset: 0,
        transition: "outline-color 120ms",
        "&:hover": { outline: 2, outlineColor: selected ? "primary.main" : "border.strong" },
      }}
    >
      <Box
        sx={{
          height: compact ? 46 : 64,
          px: 1.25,
          py: compact ? 0.75 : 1,
          bgcolor: theme.background,
          color: theme.foreground,
          fontFamily: "monospace",
          fontSize: 10.5,
          lineHeight: 1.45,
          whiteSpace: "pre",
          overflow: "hidden",
          position: "relative",
        }}
      >
        {(compact ? SAMPLE.slice(0, 1) : SAMPLE).map((line, i) => (
          <Box key={i} component="div">
            {line.map((seg, j) => (
              <Box key={j} component="span" sx={{ color: theme.ansi[seg.color] }}>
                {seg.text}
              </Box>
            ))}
          </Box>
        ))}
        <Box sx={{ display: "flex", gap: 0.5, mt: 0.5 }}>
          {theme.ansi.slice(0, 8).map((c, i) => (
            <Box key={i} sx={{ width: 8, height: 8, borderRadius: "2px", bgcolor: c }} />
          ))}
        </Box>
        {selected && (
          <Box
            sx={{
              position: "absolute",
              top: 6,
              right: 6,
              width: 18,
              height: 18,
              borderRadius: "50%",
              bgcolor: "primary.main",
              color: "primary.contrastText",
              display: "grid",
              placeItems: "center",
            }}
          >
            <CheckRoundedIcon sx={{ fontSize: 13 }} />
          </Box>
        )}
      </Box>
      <Box sx={{ px: 1.25, py: compact ? 0.5 : 0.75, minWidth: 0 }}>
        <Typography variant="body2" noWrap sx={{ fontWeight: 500 }}>
          {label}
        </Typography>
        {!compact && (
          <Typography variant="caption" color="text.secondary" noWrap sx={{ display: "block" }}>
            {hint ?? (theme.dark ? "Dark" : "Light")}
          </Typography>
        )}
      </Box>
    </ButtonBase>
  );
}
