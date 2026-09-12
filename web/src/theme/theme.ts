import { createTheme, alpha } from "@mui/material/styles";

declare module "@mui/material/styles" {
  interface Palette {
    surface: { lowest: string; base: string; high: string; highest: string };
    border: { light: string; basic: string; strong: string };
  }
  interface PaletteOptions {
    surface?: { lowest: string; base: string; high: string; highest: string };
    border?: { light: string; basic: string; strong: string };
  }
}

declare module "@mui/material/Button" {
  interface ButtonPropsVariantOverrides {
    /** Neutral filled button; `contained` is reserved for the primary action. */
    tonal: true;
  }
}

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

/** Four stacked layers, lowest → highest. Cards never draw borders; they sit one layer up. */
export const dark = {
  lowest: "#141826",
  base: "#1C2032",
  high: "#262B3F",
  highest: "#30364C",
  strong: "#3E4459",
  disabled: "#5A5E73",
  secondary: "#8D91A5",
  text: "#F1F3F8",
};

export const light = {
  lowest: "#EEF1F5",
  base: "#F7F9FB",
  high: "#FFFFFF",
  highest: "#E7EBF1",
  strong: "#C9D0DB",
  disabled: "#9AA3B2",
  secondary: "#626D80",
  text: "#171B26",
};

const grey = "#8D91A5";
const popoverShadow = "0 8px 24px rgba(0,0,0,0.35)";

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

/** Control heights shared by buttons, inputs and list rows. */
export const sizes = {
  control: 32,
  input: 36,
  row: 44,
  sidebar: 232,
  topbar: 52,
  content: 880,
} as const;

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
          contrastText: "#08170F",
        },
        secondary: {
          main: sky.main,
          dark: sky.dark,
          light: sky.light,
          contrastText: "#0A1420",
        },
        error: { main: "#F25E61" },
        warning: { main: "#F8AA4B" },
        success: { main: emerald.main },
        info: { main: sky.main },
        background: { default: dark.lowest, paper: dark.base },
        surface: {
          lowest: dark.lowest,
          base: dark.base,
          high: dark.high,
          highest: dark.highest,
        },
        border: {
          light: alpha(grey, 0.1),
          basic: alpha(grey, 0.22),
          strong: dark.strong,
        },
        text: {
          primary: dark.text,
          secondary: dark.secondary,
          disabled: dark.disabled,
        },
        divider: alpha(grey, 0.14),
        action: {
          hover: alpha(grey, 0.1),
          selected: alpha(grey, 0.16),
          focus: alpha(emerald.main, 0.24),
          disabledBackground: alpha(grey, 0.1),
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
        secondary: {
          main: sky.dark,
          dark: "#2C6B9C",
          light: sky.main,
          contrastText: "#FFFFFF",
        },
        error: { main: "#D93F42" },
        warning: { main: "#C67D1A" },
        success: { main: emerald.dark },
        info: { main: sky.dark },
        background: { default: light.lowest, paper: light.base },
        surface: {
          lowest: light.lowest,
          base: light.base,
          high: light.high,
          highest: light.highest,
        },
        border: {
          light: alpha("#3B4557", 0.08),
          basic: alpha("#3B4557", 0.18),
          strong: light.strong,
        },
        text: {
          primary: light.text,
          secondary: light.secondary,
          disabled: light.disabled,
        },
        divider: alpha("#3B4557", 0.12),
        action: {
          hover: alpha("#3B4557", 0.06),
          selected: alpha("#3B4557", 0.1),
          focus: alpha(emerald.dark, 0.2),
        },
      },
    },
  },
  shape: { borderRadius: 8 },
  typography: {
    fontFamily,
    fontSize: 14,
    h1: { fontSize: "1.75rem", fontWeight: 600, letterSpacing: "-0.01em" },
    h2: { fontSize: "1.375rem", fontWeight: 600, letterSpacing: "-0.01em" },
    h3: { fontSize: "1.125rem", fontWeight: 600 },
    h4: { fontSize: "1rem", fontWeight: 600 },
    h5: { fontSize: "0.9375rem", fontWeight: 600 },
    h6: { fontSize: "0.875rem", fontWeight: 600 },
    subtitle1: { fontSize: "0.9375rem", fontWeight: 600, lineHeight: 1.4 },
    subtitle2: { fontSize: "0.875rem", fontWeight: 600, lineHeight: 1.4 },
    body1: { fontSize: "0.875rem", lineHeight: 1.5 },
    body2: { fontSize: "0.8125rem", lineHeight: 1.5 },
    caption: { fontSize: "0.75rem", lineHeight: 1.4 },
    button: { textTransform: "none", fontWeight: 500, fontSize: "0.8125rem" },
    overline: { letterSpacing: "0.04em", fontWeight: 600, fontSize: "0.6875rem" },
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
      defaultProps: { disableElevation: true, size: "small" },
      styleOverrides: {
        root: ({ theme: t }) => ({
          borderRadius: 6,
          minHeight: sizes.control,
          paddingInline: 12,
          gap: 6,
          whiteSpace: "nowrap",
          "& .MuiButton-startIcon": { marginRight: 0, marginLeft: -2 },
          "& .MuiButton-endIcon": { marginLeft: 0, marginRight: -4 },
          "& .MuiButton-startIcon > *:nth-of-type(1), & .MuiButton-endIcon > *:nth-of-type(1)": {
            fontSize: 18,
          },
          variants: [
            {
              props: { variant: "tonal" },
              style: {
                backgroundColor: t.vars.palette.surface.highest,
                color: t.vars.palette.text.primary,
                "&:hover": { backgroundColor: t.vars.palette.border.strong },
                "&.Mui-disabled": {
                  backgroundColor: t.vars.palette.action.disabledBackground,
                },
              },
            },
          ],
        }),
        sizeSmall: { minHeight: sizes.control, paddingInline: 12 },
        sizeMedium: { minHeight: 36, paddingInline: 14 },
        sizeLarge: { minHeight: 40, paddingInline: 16, fontSize: "0.875rem" },
        outlined: ({ theme: t }) => ({
          borderColor: t.vars.palette.border.basic,
          color: t.vars.palette.text.primary,
          "&:hover": {
            borderColor: t.vars.palette.border.strong,
            backgroundColor: t.vars.palette.action.hover,
          },
          "&.MuiButton-colorError": { color: t.vars.palette.error.main },
        }),
        text: ({ theme: t }) => ({
          color: t.vars.palette.text.primary,
          "&:hover": { backgroundColor: t.vars.palette.action.hover },
          "&.MuiButton-colorError": { color: t.vars.palette.error.main },
        }),
      },
    },
    MuiIconButton: {
      defaultProps: { size: "small" },
      styleOverrides: {
        root: ({ theme: t }) => ({
          borderRadius: 6,
          color: t.vars.palette.text.secondary,
          "&:hover": {
            backgroundColor: t.vars.palette.action.hover,
            color: t.vars.palette.text.primary,
          },
        }),
        sizeSmall: { width: sizes.control, height: sizes.control, padding: 0 },
      },
    },
    MuiSvgIcon: {
      styleOverrides: {
        fontSizeSmall: { fontSize: 18 },
        fontSizeMedium: { fontSize: 20 },
      },
    },
    MuiToggleButtonGroup: {
      styleOverrides: {
        root: ({ theme: t }) => ({
          backgroundColor: t.vars.palette.surface.highest,
          borderRadius: 6,
          padding: 2,
          gap: 2,
        }),
      },
    },
    MuiToggleButton: {
      defaultProps: { size: "small" },
      styleOverrides: {
        root: ({ theme: t }) => ({
          border: 0,
          borderRadius: "5px !important",
          minHeight: sizes.control - 4,
          padding: "0 10px",
          color: t.vars.palette.text.secondary,
          textTransform: "none",
          fontWeight: 500,
          "&.Mui-selected": {
            backgroundColor: t.vars.palette.surface.base,
            color: t.vars.palette.text.primary,
            "&:hover": { backgroundColor: t.vars.palette.surface.base },
          },
        }),
      },
    },
    MuiPaper: {
      defaultProps: { elevation: 0 },
      styleOverrides: {
        root: { backgroundImage: "none" },
        outlined: ({ theme: t }) => ({ borderColor: t.vars.palette.border.light }),
      },
    },
    MuiCard: {
      defaultProps: { variant: "elevation" },
      styleOverrides: {
        root: ({ theme: t }) => ({
          backgroundColor: t.vars.palette.surface.high,
          borderRadius: 10,
        }),
      },
    },
    MuiFormControl: {
      defaultProps: { size: "small" },
    },
    MuiTextField: {
      defaultProps: { variant: "outlined", size: "small", fullWidth: true },
    },
    // Labels sit above the field (no floating / notched outline).
    MuiInputLabel: {
      defaultProps: { shrink: true },
      styleOverrides: {
        root: ({ theme: t }) => ({
          position: "relative",
          transform: "none",
          maxWidth: "none",
          fontSize: "0.8125rem",
          fontWeight: 500,
          lineHeight: 1.3,
          marginBottom: 6,
          color: t.vars.palette.text.secondary,
          pointerEvents: "auto",
          "&.Mui-focused": { color: t.vars.palette.text.secondary },
          "&.Mui-error": { color: t.vars.palette.error.main },
          "&.Mui-disabled": { color: t.vars.palette.text.disabled },
          "& .MuiFormLabel-asterisk": { color: t.vars.palette.text.disabled },
        }),
      },
    },
    MuiFormHelperText: {
      styleOverrides: {
        root: { marginInline: 0, marginTop: 6, fontSize: "0.75rem" },
      },
    },
    MuiOutlinedInput: {
      styleOverrides: {
        root: ({ theme: t }) => ({
          borderRadius: 8,
          backgroundColor: t.vars.palette.surface.base,
          fontSize: "0.875rem",
          "& .MuiOutlinedInput-notchedOutline": {
            top: 0,
            borderColor: t.vars.palette.border.basic,
            "& legend": { display: "none" },
          },
          "&:hover .MuiOutlinedInput-notchedOutline": {
            borderColor: t.vars.palette.border.strong,
          },
          "&.Mui-focused .MuiOutlinedInput-notchedOutline": {
            borderColor: t.vars.palette.primary.main,
            borderWidth: 1,
          },
          "&.Mui-disabled .MuiOutlinedInput-notchedOutline": {
            borderColor: t.vars.palette.border.light,
          },
          "& .MuiInputAdornment-root": { color: t.vars.palette.text.secondary },
        }),
        input: { paddingBlock: 8.5, paddingInline: 12, height: "1.375em" },
        multiline: { padding: 0, "& textarea": { paddingBlock: 8.5, paddingInline: 12 } },
      },
    },
    MuiSelect: {
      defaultProps: { displayEmpty: true },
      styleOverrides: {
        icon: ({ theme: t }) => ({ color: t.vars.palette.text.secondary }),
      },
    },
    MuiMenu: {
      styleOverrides: {
        paper: ({ theme: t }) => ({
          backgroundColor: t.vars.palette.surface.high,
          borderRadius: 8,
          border: `1px solid ${t.vars.palette.border.light}`,
          boxShadow: popoverShadow,
          marginTop: 4,
          minWidth: 180,
        }),
        list: { padding: 4 },
      },
    },
    MuiMenuItem: {
      styleOverrides: {
        root: ({ theme: t }) => ({
          borderRadius: 6,
          minHeight: 34,
          fontSize: "0.875rem",
          paddingInline: 10,
          gap: 10,
          "& .MuiListItemIcon-root": { minWidth: 0, color: t.vars.palette.text.secondary },
          "&.Mui-selected": {
            backgroundColor: t.vars.palette.action.selected,
            "&:hover": { backgroundColor: t.vars.palette.action.selected },
          },
        }),
      },
    },
    MuiPopover: {
      styleOverrides: {
        paper: ({ theme: t }) => ({
          backgroundColor: t.vars.palette.surface.high,
          borderRadius: 8,
          border: `1px solid ${t.vars.palette.border.light}`,
          boxShadow: popoverShadow,
        }),
      },
    },
    MuiChip: {
      styleOverrides: {
        root: ({ theme: t }) => ({
          fontWeight: 500,
          borderRadius: 6,
          backgroundColor: t.vars.palette.surface.highest,
          "&.MuiChip-colorPrimary": {
            backgroundColor: alpha(emerald.main, 0.16),
            color: emerald.light,
            ...t.applyStyles("light", { color: emerald.dark }),
          },
          "&.MuiChip-colorSecondary": {
            backgroundColor: alpha(sky.main, 0.16),
            color: sky.light,
            ...t.applyStyles("light", { color: sky.dark }),
          },
          "&.MuiChip-colorSuccess": {
            backgroundColor: alpha(emerald.main, 0.16),
            color: emerald.light,
            ...t.applyStyles("light", { color: emerald.dark }),
          },
          "&.MuiChip-colorWarning": {
            backgroundColor: alpha("#F8AA4B", 0.16),
            color: "#F8AA4B",
            ...t.applyStyles("light", { color: "#9A5F0C" }),
          },
          "&.MuiChip-colorError": {
            backgroundColor: alpha("#F25E61", 0.16),
            color: "#F25E61",
            ...t.applyStyles("light", { color: "#B32C2F" }),
          },
        }),
        sizeSmall: { height: 22, fontSize: "0.75rem" },
        outlined: { border: 0 },
      },
    },
    MuiListItemButton: {
      styleOverrides: {
        root: ({ theme: t }) => ({
          borderRadius: 6,
          paddingBlock: 6,
          "&.Mui-selected": {
            backgroundColor: t.vars.palette.surface.highest,
            "&:hover": { backgroundColor: t.vars.palette.surface.highest },
          },
        }),
      },
    },
    MuiListItemIcon: {
      styleOverrides: {
        root: ({ theme: t }) => ({
          minWidth: 32,
          color: t.vars.palette.text.secondary,
          "& svg": { fontSize: 18 },
        }),
      },
    },
    MuiListSubheader: {
      styleOverrides: {
        root: ({ theme: t }) => ({
          backgroundColor: "transparent",
          color: t.vars.palette.text.disabled,
          fontSize: "0.6875rem",
          fontWeight: 600,
          letterSpacing: "0.04em",
          textTransform: "uppercase",
          lineHeight: "28px",
          paddingInline: 10,
        }),
      },
    },
    MuiDialog: {
      styleOverrides: {
        paper: ({ theme: t }) => ({
          borderRadius: 12,
          backgroundColor: t.vars.palette.surface.base,
          border: `1px solid ${t.vars.palette.border.light}`,
          backgroundImage: "none",
        }),
      },
    },
    MuiDialogTitle: {
      styleOverrides: {
        root: { fontSize: "1rem", fontWeight: 600, padding: "20px 24px 8px" },
      },
    },
    MuiDialogContent: {
      styleOverrides: {
        root: { padding: "8px 24px 12px" },
      },
    },
    MuiDialogContentText: {
      styleOverrides: {
        root: { fontSize: "0.875rem" },
      },
    },
    MuiDialogActions: {
      styleOverrides: { root: { padding: "12px 24px 20px", gap: 4 } },
    },
    MuiTooltip: {
      defaultProps: { arrow: false, enterDelay: 400 },
      styleOverrides: {
        tooltip: ({ theme: t }) => ({
          backgroundColor: t.vars.palette.surface.highest,
          color: t.vars.palette.text.primary,
          fontSize: "0.75rem",
          fontWeight: 500,
          borderRadius: 6,
          border: `1px solid ${t.vars.palette.border.light}`,
          padding: "5px 8px",
        }),
      },
    },
    MuiTableCell: {
      styleOverrides: {
        root: ({ theme: t }) => ({
          borderBottomColor: t.vars.palette.border.light,
          padding: "10px 16px",
          fontSize: "0.875rem",
        }),
        head: ({ theme: t }) => ({
          fontWeight: 500,
          color: t.vars.palette.text.secondary,
          fontSize: "0.75rem",
          paddingBlock: 8,
          textTransform: "none",
          letterSpacing: 0,
          whiteSpace: "nowrap",
        }),
      },
    },
    MuiTableRow: {
      styleOverrides: {
        root: ({ theme: t }) => ({
          "&:last-child td": { borderBottom: 0 },
          "&.MuiTableRow-hover:hover": { backgroundColor: t.vars.palette.action.hover },
        }),
      },
    },
    MuiTablePagination: {
      styleOverrides: {
        root: ({ theme: t }) => ({
          borderTop: `1px solid ${t.vars.palette.border.light}`,
          color: t.vars.palette.text.secondary,
          fontSize: "0.8125rem",
        }),
        toolbar: { minHeight: 44 },
        selectLabel: { fontSize: "0.8125rem" },
        displayedRows: { fontSize: "0.8125rem" },
      },
    },
    MuiAlert: {
      styleOverrides: {
        root: { borderRadius: 8, fontSize: "0.8125rem", alignItems: "center" },
        icon: { fontSize: 18, alignItems: "center" },
        standard: ({ theme: t }) => ({
          color: t.vars.palette.text.primary,
          "&.MuiAlert-colorInfo": { backgroundColor: alpha(sky.main, 0.12) },
          "&.MuiAlert-colorWarning": { backgroundColor: alpha("#F8AA4B", 0.14) },
          "&.MuiAlert-colorError": { backgroundColor: alpha("#F25E61", 0.14) },
          "&.MuiAlert-colorSuccess": { backgroundColor: alpha(emerald.main, 0.14) },
        }),
        outlined: ({ theme: t }) => ({
          color: t.vars.palette.text.primary,
          borderColor: t.vars.palette.border.basic,
          "& .MuiAlert-icon": { color: t.vars.palette.text.secondary },
        }),
      },
    },
    MuiSwitch: {
      styleOverrides: {
        root: ({ theme: t }) => ({
          width: 44,
          height: 28,
          padding: 2,
          "& .MuiSwitch-switchBase": {
            padding: 5,
            color: t.vars.palette.text.secondary,
            "&.Mui-checked": {
              transform: "translateX(16px)",
              color: "#fff",
              "& + .MuiSwitch-track": { opacity: 1, backgroundColor: emerald.main },
            },
            "&.Mui-disabled + .MuiSwitch-track": { opacity: 0.4 },
          },
          "& .MuiSwitch-thumb": { width: 18, height: 18, boxShadow: "none" },
        }),
        track: ({ theme: t }) => ({
          borderRadius: 12,
          opacity: 1,
          backgroundColor: t.vars.palette.border.strong,
        }),
      },
    },
    MuiCheckbox: {
      defaultProps: { size: "small" },
      styleOverrides: {
        root: ({ theme: t }) => ({ color: t.vars.palette.text.secondary, padding: 6 }),
      },
    },
    MuiRadio: {
      defaultProps: { size: "small" },
      styleOverrides: {
        root: ({ theme: t }) => ({ color: t.vars.palette.text.secondary, padding: 6 }),
      },
    },
    MuiFormControlLabel: {
      styleOverrides: {
        root: { marginLeft: -4, marginRight: 0 },
        label: { fontSize: "0.875rem", marginLeft: 6 },
      },
    },
    MuiTabs: {
      styleOverrides: {
        root: { minHeight: 36 },
        indicator: { height: 2, borderRadius: 1 },
      },
    },
    MuiTab: {
      styleOverrides: {
        root: {
          minHeight: 36,
          textTransform: "none",
          fontWeight: 500,
          fontSize: "0.875rem",
          paddingInline: 12,
          minWidth: 0,
        },
      },
    },
    MuiDivider: {
      styleOverrides: {
        root: ({ theme: t }) => ({ borderColor: t.vars.palette.border.light }),
      },
    },
    MuiLinearProgress: {
      styleOverrides: {
        root: ({ theme: t }) => ({
          borderRadius: 2,
          backgroundColor: t.vars.palette.surface.highest,
        }),
      },
    },
    MuiAvatar: {
      styleOverrides: {
        root: ({ theme: t }) => ({
          backgroundColor: t.vars.palette.surface.highest,
          color: t.vars.palette.text.primary,
          fontWeight: 600,
        }),
      },
    },
    MuiLink: {
      defaultProps: { underline: "hover" },
    },
    MuiSkeleton: {
      styleOverrides: {
        root: ({ theme: t }) => ({ backgroundColor: t.vars.palette.surface.high }),
      },
    },
  },
});
