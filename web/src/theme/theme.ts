import { createTheme, alpha } from "@mui/material/styles";

export const emerald = {
  main: "#2BB884",
  dark: "#1F8F66",
  light: "#5FD0A4",
};

export const sky = {
  main: "#5AA9E6",
  dark: "#3D86BF",
  light: "#8CC5F0",
};

export const dark = {
  bg: "#12151F",
  surface: "#1A1E2B",
  surfaceHigh: "#232838",
  surfaceHighest: "#2D3345",
  outline: "#3A4055",
  disabled: "#5A6076",
  secondary: "#8E93A8",
  text: "#E6E8F0",
};

export const light = {
  bg: "#F5F7FA",
  surface: "#FFFFFF",
  surfaceHigh: "#EEF1F5",
  surfaceHighest: "#E3E7ED",
  outline: "#CBD2DC",
  disabled: "#9AA3B2",
  secondary: "#5F6B7C",
  text: "#171B26",
};

const fontFamily = [
  "'Inter Variable'",
  "Inter",
  "system-ui",
  "-apple-system",
  "'Segoe UI'",
  "Roboto",
  "sans-serif",
].join(",");

export const monoFontFamily = [
  "'JetBrains Mono Variable'",
  "'JetBrains Mono'",
  "ui-monospace",
  "SFMono-Regular",
  "Menlo",
  "monospace",
].join(",");

export const theme = createTheme({
  cssVariables: { colorSchemeSelector: "data-termoso-scheme" },
  colorSchemes: {
    dark: {
      palette: {
        mode: "dark",
        primary: {
          main: emerald.main,
          dark: emerald.dark,
          light: emerald.light,
          contrastText: "#0B1A14",
        },
        secondary: { main: sky.main, dark: sky.dark, light: sky.light, contrastText: "#0A1420" },
        error: { main: "#F25E61" },
        warning: { main: "#F2C94C" },
        success: { main: emerald.light },
        info: { main: sky.light },
        background: { default: dark.bg, paper: dark.surface },
        text: { primary: dark.text, secondary: dark.secondary, disabled: dark.disabled },
        divider: dark.outline,
        action: {
          hover: alpha("#FFFFFF", 0.06),
          selected: alpha(emerald.main, 0.16),
          disabledBackground: alpha("#FFFFFF", 0.08),
        },
      },
    },
    light: {
      palette: {
        mode: "light",
        primary: {
          main: emerald.dark,
          dark: "#166B4C",
          light: emerald.main,
          contrastText: "#FFFFFF",
        },
        secondary: { main: sky.dark, dark: "#2C6B9C", light: sky.main, contrastText: "#FFFFFF" },
        error: { main: "#D93F42" },
        warning: { main: "#C99A16" },
        success: { main: emerald.dark },
        info: { main: sky.dark },
        background: { default: light.bg, paper: light.surface },
        text: { primary: light.text, secondary: light.secondary, disabled: light.disabled },
        divider: light.outline,
        action: {
          hover: alpha("#000000", 0.04),
          selected: alpha(emerald.dark, 0.12),
        },
      },
    },
  },
  shape: { borderRadius: 12 },
  typography: {
    fontFamily,
    h1: { fontSize: "2rem", fontWeight: 600, letterSpacing: "-0.01em" },
    h2: { fontSize: "1.5rem", fontWeight: 600, letterSpacing: "-0.01em" },
    h3: { fontSize: "1.25rem", fontWeight: 600 },
    h4: { fontSize: "1.125rem", fontWeight: 600 },
    h5: { fontSize: "1rem", fontWeight: 600 },
    h6: { fontSize: "0.9375rem", fontWeight: 600 },
    subtitle1: { fontWeight: 500 },
    subtitle2: { fontWeight: 500, fontSize: "0.8125rem" },
    body2: { fontSize: "0.8125rem" },
    button: { textTransform: "none", fontWeight: 600 },
    overline: { letterSpacing: "0.08em", fontWeight: 600 },
  },
  components: {
    MuiCssBaseline: {
      styleOverrides: {
        body: { minHeight: "100vh" },
        code: { fontFamily: monoFontFamily },
        "::selection": { backgroundColor: alpha(emerald.main, 0.35) },
      },
    },
    MuiButton: {
      defaultProps: { disableElevation: true },
      styleOverrides: {
        root: { borderRadius: 999, paddingInline: 20, minHeight: 40 },
        sizeSmall: { minHeight: 32, paddingInline: 14 },
        sizeLarge: { minHeight: 48, paddingInline: 24 },
      },
    },
    MuiPaper: {
      defaultProps: { elevation: 0 },
      styleOverrides: {
        root: { backgroundImage: "none" },
        outlined: ({ theme: t }) => ({ borderColor: t.vars.palette.divider }),
      },
    },
    MuiCard: {
      defaultProps: { variant: "outlined" },
    },
    MuiTextField: {
      defaultProps: { variant: "outlined", size: "medium", fullWidth: true },
    },
    MuiOutlinedInput: {
      styleOverrides: { root: { borderRadius: 12 } },
    },
    MuiChip: {
      styleOverrides: { root: { fontWeight: 500 } },
    },
    MuiListItemButton: {
      styleOverrides: {
        root: ({ theme: t }) => ({
          borderRadius: 999,
          marginInline: 8,
          paddingBlock: 8,
          "&.Mui-selected": {
            backgroundColor: t.vars.palette.action.selected,
            color: t.vars.palette.primary.main,
            "& .MuiListItemIcon-root": { color: t.vars.palette.primary.main },
          },
        }),
      },
    },
    MuiListItemIcon: {
      styleOverrides: { root: { minWidth: 40 } },
    },
    MuiDialog: {
      styleOverrides: { paper: { borderRadius: 24 } },
    },
    MuiTooltip: {
      defaultProps: { arrow: true },
    },
    MuiTableCell: {
      styleOverrides: {
        head: ({ theme: t }) => ({
          fontWeight: 600,
          color: t.vars.palette.text.secondary,
          fontSize: "0.75rem",
          textTransform: "uppercase",
          letterSpacing: "0.05em",
        }),
      },
    },
    MuiAlert: {
      styleOverrides: { root: { borderRadius: 12 } },
    },
  },
});
