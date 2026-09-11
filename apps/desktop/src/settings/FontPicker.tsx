import { useState } from "react";
import { Box, MenuItem, TextField, Typography } from "@mui/material";
import { bundledFont, bundledFonts, terminalFontStack } from "@/terminal/fonts";
import { useTerminalTheme } from "@/terminal/useTerminalTheme";

const CUSTOM = "\u0000custom";

/** Bundled monospace faces rendered in themselves, or any installed family by name. */
export function FontPicker({
  value,
  onChange,
}: {
  value: string;
  onChange: (name: string) => void;
}) {
  const bundled = bundledFont(value);
  const [custom, setCustom] = useState(!bundled && value.trim() !== "");
  const selectValue = custom || !bundled ? CUSTOM : bundled.name;

  return (
    <Box sx={{ display: "flex", flexDirection: "column", gap: 1, alignItems: "flex-end" }}>
      <TextField
        select
        value={selectValue}
        onChange={(e) => {
          if (e.target.value === CUSTOM) {
            setCustom(true);
            return;
          }
          setCustom(false);
          onChange(e.target.value);
        }}
        sx={{ width: 280 }}
        slotProps={{ select: { MenuProps: { slotProps: { paper: { sx: { maxHeight: 360 } } } } } }}
      >
        {bundledFonts.map((f) => (
          <MenuItem key={f.name} value={f.name} sx={{ fontFamily: `'${f.family}', monospace` }}>
            {f.name}
            {f.ligatures && (
              <Typography component="span" variant="caption" color="text.secondary" sx={{ ml: 1 }}>
                ligatures
              </Typography>
            )}
          </MenuItem>
        ))}
        <MenuItem value={CUSTOM}>Installed font…</MenuItem>
      </TextField>
      {(custom || !bundled) && (
        <TextField
          value={bundled ? "" : value}
          placeholder="Family name, e.g. JetBrainsMono Nerd Font"
          onChange={(e) => onChange(e.target.value)}
          sx={{ width: 280 }}
        />
      )}
    </Box>
  );
}

/** Rendered sample with box drawing, Nerd Font glyphs and a ligature candidate. */
export function FontPreview({
  family,
  size,
  lineHeight,
}: {
  family: string;
  size: number;
  lineHeight: number;
}) {
  const theme = useTerminalTheme();
  const [, , green, yellow, blue, magenta] = theme.ansi;
  return (
    <Box
      sx={{
        bgcolor: theme.background,
        color: theme.foreground,
        borderRadius: 1.5,
        px: 1.5,
        py: 1.25,
        fontFamily: terminalFontStack(family),
        fontSize: size,
        lineHeight,
        whiteSpace: "pre",
        overflow: "hidden",
      }}
    >
      <span style={{ color: blue }}>{"\uF17C ~/termoso  "}</span>
      <span style={{ color: magenta }}>{"\uE725 main  "}</span>
      <span style={{ color: green }}>{"\uF00C\n"}</span>
      <span style={{ color: green }}>{"❯ "}</span>
      {'cargo build --release && echo "done" => 0x1F 0O0 1lI |ij\n'}
      {"The quick brown fox jumps over the lazy dog "}
      <span style={{ color: yellow }}>0123456789</span>
      {" ->> != <=>"}
    </Box>
  );
}
