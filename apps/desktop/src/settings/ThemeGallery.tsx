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
  if (compact) {
    return (
      <ButtonBase
        onClick={onClick}
        aria-pressed={selected}
        sx={{
          display: "flex",
          alignItems: "center",
          gap: 1.25,
          textAlign: "left",
          borderRadius: 1.5,
          px: 0.75,
          py: 0.5,
          bgcolor: selected ? "surface.highest" : "transparent",
          "&:hover": { bgcolor: "surface.highest" },
        }}
      >
        <Box
          sx={{
            width: 64,
            height: 40,
            flexShrink: 0,
            borderRadius: 1,
            bgcolor: theme.background,
            display: "flex",
            flexDirection: "column",
            justifyContent: "center",
            gap: "5px",
            px: 1,
          }}
        >
          {[
            { color: theme.ansi[2], w: "100%" },
            { color: theme.ansi[4], w: "70%" },
            { color: theme.foreground, w: "45%" },
          ].map((bar, i) => (
            <Box
              key={i}
              sx={{ height: 4, width: bar.w, borderRadius: 1, bgcolor: bar.color, opacity: 0.9 }}
            />
          ))}
        </Box>
        <Box sx={{ minWidth: 0, flex: 1 }}>
          <Typography
            variant="body2"
            noWrap
            sx={{ fontWeight: 500, color: selected ? "primary.main" : "text.primary" }}
          >
            {label}
          </Typography>
          <Typography variant="caption" color="text.secondary" noWrap sx={{ display: "block" }}>
            {hint ?? (theme.dark ? "Dark" : "Light")}
          </Typography>
        </Box>
        {selected && <CheckRoundedIcon sx={{ fontSize: 18, color: "primary.main", mr: 0.5 }} />}
      </ButtonBase>
    );
  }

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
        bgcolor: selected ? "surface.strong" : "surface.highest",
        transition: "background-color 120ms",
        "&:hover": { bgcolor: "surface.strong" },
      }}
    >
      <Box
        sx={{
          height: 64,
          px: 1.25,
          py: 1,
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
        {SAMPLE.map((line, i) => (
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
      <Box sx={{ px: 1.25, py: 0.75, minWidth: 0 }}>
        <Typography
          variant="body2"
          noWrap
          sx={{ fontWeight: 500, color: selected ? "primary.main" : "text.primary" }}
        >
          {label}
        </Typography>
        <Typography variant="caption" color="text.secondary" noWrap sx={{ display: "block" }}>
          {hint ?? (theme.dark ? "Dark" : "Light")}
        </Typography>
      </Box>
    </ButtonBase>
  );
}
