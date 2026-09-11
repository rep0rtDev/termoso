import {
  Box,
  Button,
  ButtonGroup,
  CircularProgress,
  IconButton,
  InputAdornment,
  Menu,
  MenuItem,
  ListItemIcon,
  ListItemText,
  TextField,
  Tooltip,
  Typography,
  type ButtonProps,
  type SxProps,
  type Theme,
} from "@mui/material";
import KeyboardArrowDownRoundedIcon from "@mui/icons-material/KeyboardArrowDownRounded";
import CloseRoundedIcon from "@mui/icons-material/CloseRounded";
import ArrowBackRoundedIcon from "@mui/icons-material/ArrowBackRounded";
import SearchRoundedIcon from "@mui/icons-material/SearchRounded";
import InfoOutlinedIcon from "@mui/icons-material/InfoOutlined";
import { type MouseEvent, type ReactNode, useState } from "react";
import { monoFontFamily, sizes } from "@/theme/theme";

/** Normalises an optional `sx` prop so it can be spread after base styles. */
type SxItem = Exclude<SxProps<Theme>, readonly unknown[]>;
function sxList(sx?: SxProps<Theme>): readonly SxItem[] {
  if (!sx) return [];
  return Array.isArray(sx) ? (sx as readonly SxItem[]) : [sx as SxItem];
}

/* ---------------------------------------------------------------- tiles */

export type TileTone = "accent" | "neutral" | "info" | "warning" | "danger" | "purple";

const toneColor: Record<TileTone, { fg: string; bg: string }> = {
  accent: { fg: "primary.main", bg: "rgba(43,184,132,0.18)" },
  info: { fg: "secondary.main", bg: "rgba(90,169,230,0.18)" },
  warning: { fg: "warning.main", bg: "rgba(248,170,75,0.18)" },
  danger: { fg: "error.main", bg: "rgba(242,94,97,0.18)" },
  purple: { fg: "#B07CF2", bg: "rgba(176,124,242,0.18)" },
  neutral: { fg: "text.secondary", bg: "surface.highest" },
};

/** Square icon tile used on every entity card (host, group, key, rule, package…). */
export function IconTile({
  children,
  tone = "neutral",
  size = sizes.tile,
  color,
  sx,
}: {
  children: ReactNode;
  tone?: TileTone;
  size?: number;
  /** Explicit color (e.g. an OS brand) — overrides `tone`. */
  color?: string;
  sx?: SxProps<Theme>;
}) {
  const t = toneColor[tone];
  return (
    <Box
      sx={[
        {
          width: size,
          height: size,
          borderRadius: size >= 40 ? 2 : 1.5,
          display: "grid",
          placeItems: "center",
          flexShrink: 0,
          bgcolor: color ?? t.bg,
          color: color ? "#fff" : t.fg,
          "& svg": { fontSize: Math.round(size * 0.5) },
          fontWeight: 600,
          fontSize: Math.round(size * 0.42),
        },
        ...sxList(sx),
      ]}
    >
      {children}
    </Box>
  );
}

/* ---------------------------------------------------------------- cards */

/** A list/grid entry: tile + two lines + optional trailing slot. */
export function EntityCard({
  tile,
  title,
  subtitle,
  meta,
  trailing,
  actions,
  selected,
  onClick,
  onDoubleClick,
  onContextMenu,
  dense,
  className,
  sx,
}: {
  tile: ReactNode;
  title: ReactNode;
  subtitle?: ReactNode;
  /** Third line under the subtitle (tag chips). */
  meta?: ReactNode;
  /** Always visible (status). */
  trailing?: ReactNode;
  /** Revealed on hover / selection (icon buttons). */
  actions?: ReactNode;
  selected?: boolean;
  onClick?: (e: MouseEvent<HTMLElement>) => void;
  onDoubleClick?: () => void;
  onContextMenu?: (e: MouseEvent<HTMLElement>) => void;
  dense?: boolean;
  className?: string;
  sx?: SxProps<Theme>;
}) {
  return (
    <Box
      role="button"
      tabIndex={0}
      className={className}
      onClick={onClick}
      onDoubleClick={onDoubleClick}
      onContextMenu={onContextMenu}
      onKeyDown={(e) => {
        if (e.key === "Enter" && onClick && e.target === e.currentTarget) e.currentTarget.click();
      }}
      sx={[
        {
          display: "flex",
          alignItems: "center",
          gap: 1.5,
          px: dense ? 1.25 : 1.5,
          py: dense ? 0.75 : 1.25,
          minHeight: dense ? 48 : 64,
          borderRadius: 2,
          bgcolor: "surface.high",
          cursor: "default",
          outline: "1px solid transparent",
          outlineOffset: -1,
          transition: "background-color 100ms, outline-color 100ms",
          "&:hover": { bgcolor: "surface.highest" },
          "&:focus-visible": { outlineColor: "primary.main" },
          ...(selected && {
            outlineColor: "primary.main",
            "& .entity-actions": { opacity: 1 },
          }),
          "& .entity-actions": { opacity: 0, transition: "opacity 100ms" },
          "&:hover .entity-actions, &:focus-within .entity-actions": { opacity: 1 },
        },
        ...sxList(sx),
      ]}
    >
      {tile}
      <Box sx={{ flex: 1, minWidth: 0 }}>
        <Typography variant="body1" noWrap sx={{ fontWeight: 500, lineHeight: 1.35 }}>
          {title}
        </Typography>
        {subtitle !== undefined && subtitle !== null && subtitle !== "" && (
          <Typography
            variant="caption"
            color="text.secondary"
            noWrap
            sx={{ display: "block", lineHeight: 1.35, mt: 0.25 }}
          >
            {subtitle}
          </Typography>
        )}
        {meta && (
          <Box sx={{ display: "flex", alignItems: "center", gap: 0.5, mt: 0.75, flexWrap: "wrap" }}>
            {meta}
          </Box>
        )}
      </Box>
      {trailing && (
        <Box sx={{ display: "flex", alignItems: "center", gap: 0.5, flexShrink: 0 }}>
          {trailing}
        </Box>
      )}
      {actions && (
        <Box
          className="entity-actions"
          sx={{ display: "flex", alignItems: "center", gap: 0.25, flexShrink: 0, mr: -0.5 }}
        >
          {actions}
        </Box>
      )}
    </Box>
  );
}

/** Responsive card grid. */
export function CardGrid({ children, min = 280 }: { children: ReactNode; min?: number }) {
  return (
    <Box
      sx={{
        display: "grid",
        gridTemplateColumns: `repeat(auto-fill, minmax(${min}px, 1fr))`,
        gap: 1.5,
      }}
    >
      {children}
    </Box>
  );
}

/** "Groups" / "Hosts" heading above a card grid. */
export function SectionTitle({
  children,
  trailing,
  sx,
}: {
  children: ReactNode;
  trailing?: ReactNode;
  sx?: SxProps<Theme>;
}) {
  return (
    <Box
      sx={[
        { display: "flex", alignItems: "center", mb: 1.5, mt: 0.5, minHeight: 24 },
        ...sxList(sx),
      ]}
    >
      <Typography variant="subtitle2" sx={{ flex: 1 }}>
        {children}
      </Typography>
      {trailing}
    </Box>
  );
}

/** Grouped form section in a side panel (Termius "Address" / "General" cards). */
export function SectionCard({
  title,
  action,
  children,
  sx,
}: {
  title?: ReactNode;
  action?: ReactNode;
  children: ReactNode;
  sx?: SxProps<Theme>;
}) {
  return (
    <Box
      sx={[
        {
          bgcolor: "surface.high",
          borderRadius: 2,
          p: 2,
          display: "flex",
          flexDirection: "column",
          gap: 1.5,
        },
        ...sxList(sx),
      ]}
    >
      {(title ?? action) && (
        <Box sx={{ display: "flex", alignItems: "center", minHeight: 24 }}>
          {title && (
            <Typography variant="subtitle2" sx={{ flex: 1 }}>
              {title}
            </Typography>
          )}
          {action}
        </Box>
      )}
      {children}
    </Box>
  );
}

/* ------------------------------------------------------------- toolbar */

/** Row of actions under the top bar: `+ New X ⌄ | Terminal | Serial …  [right slot]`. */
export function Toolbar({
  children,
  trailing,
  sx,
}: {
  children?: ReactNode;
  trailing?: ReactNode;
  sx?: SxProps<Theme>;
}) {
  return (
    <Box
      sx={[
        {
          display: "flex",
          alignItems: "center",
          flexWrap: "wrap",
          gap: 1,
          px: 2,
          py: 1,
          minHeight: 48,
          borderBottom: 1,
          borderColor: "border.light",
        },
        ...sxList(sx),
      ]}
    >
      {children}
      {trailing && (
        <>
          <Box sx={{ flex: 1 }} />
          <Box sx={{ display: "flex", alignItems: "center", gap: 0.5 }}>{trailing}</Box>
        </>
      )}
    </Box>
  );
}

/** Compact filter field for a toolbar's right slot. */
export function SearchField({
  value,
  onChange,
  placeholder = "Search",
  width = 240,
  autoFocus,
}: {
  value: string;
  onChange: (v: string) => void;
  placeholder?: string;
  width?: number;
  autoFocus?: boolean;
}) {
  return (
    <TextField
      value={value}
      onChange={(e) => onChange(e.target.value)}
      placeholder={placeholder}
      autoFocus={autoFocus}
      fullWidth={false}
      sx={{ width, "& .MuiOutlinedInput-root": { height: sizes.control } }}
      slotProps={{
        input: {
          startAdornment: (
            <InputAdornment position="start">
              <SearchRoundedIcon fontSize="small" />
            </InputAdornment>
          ),
        },
      }}
    />
  );
}

/** Quiet one-line notice above page content ("recording is off", "not synced"). */
export function InfoBar({
  children,
  action,
  icon,
}: {
  children: ReactNode;
  action?: ReactNode;
  icon?: ReactNode;
}) {
  return (
    <Box
      sx={{
        display: "flex",
        alignItems: "center",
        gap: 1.25,
        px: 1.5,
        py: 1,
        borderRadius: 2,
        bgcolor: "surface.high",
        color: "text.secondary",
        "& > svg": { fontSize: 18, flexShrink: 0 },
      }}
    >
      {icon ?? <InfoOutlinedIcon />}
      <Typography variant="body2" sx={{ flex: 1, minWidth: 0 }}>
        {children}
      </Typography>
      {action}
    </Box>
  );
}

/** Monospace inline text (fingerprints, addresses, commands). */
export function Mono({
  children,
  secondary,
  sx,
}: {
  children: ReactNode;
  secondary?: boolean;
  sx?: SxProps<Theme>;
}) {
  return (
    <Box
      component="span"
      sx={[
        {
          fontFamily: monoFontFamily,
          fontSize: "0.8125em",
          ...(secondary && { color: "text.secondary" }),
        },
        ...sxList(sx),
      ]}
    >
      {children}
    </Box>
  );
}

export interface MenuAction {
  label: ReactNode;
  icon?: ReactNode;
  onClick: () => void;
  disabled?: boolean;
  danger?: boolean;
  divider?: boolean;
}

/** `[ + New host ] [⌄]` — primary click + dropdown with alternatives. */
export function SplitButton({
  label,
  icon,
  onClick,
  items,
  variant = "tonal",
  color,
  disabled,
}: {
  label: ReactNode;
  icon?: ReactNode;
  onClick: () => void;
  items: MenuAction[];
  variant?: ButtonProps["variant"];
  color?: ButtonProps["color"];
  disabled?: boolean;
}) {
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);
  return (
    <>
      <ButtonGroup
        variant={variant === "tonal" ? "contained" : variant}
        color={color}
        disabled={disabled}
        disableElevation
        sx={{
          "& .MuiButtonGroup-grouped": { minWidth: 0 },
          "& .MuiButtonGroup-firstButton, & .MuiButtonGroup-middleButton": {
            borderRight: "1px solid rgba(0,0,0,0.25)",
          },
        }}
      >
        <Button variant={variant} startIcon={icon} onClick={onClick}>
          {label}
        </Button>
        <Button
          variant={variant}
          size="small"
          aria-label="More options"
          onClick={(e) => setAnchor(e.currentTarget)}
          sx={{ px: 0.5, minWidth: 28 }}
        >
          <KeyboardArrowDownRoundedIcon fontSize="small" />
        </Button>
      </ButtonGroup>
      <ActionMenu anchor={anchor} onClose={() => setAnchor(null)} items={items} />
    </>
  );
}

/** Generic anchored menu built from `MenuAction`s. */
export function ActionMenu({
  anchor,
  position,
  onClose,
  items,
}: {
  anchor: HTMLElement | null;
  /** Use for context menus (mouse coordinates). */
  position?: { left: number; top: number } | null;
  onClose: () => void;
  items: MenuAction[];
}) {
  const open = Boolean(anchor) || Boolean(position);
  return (
    <Menu
      open={open}
      anchorEl={position ? undefined : anchor}
      anchorReference={position ? "anchorPosition" : "anchorEl"}
      anchorPosition={position ?? undefined}
      onClose={onClose}
      onClick={(e) => e.stopPropagation()}
    >
      {items.map((it, i) => (
        <MenuItem
          key={i}
          disabled={it.disabled}
          divider={it.divider}
          onClick={() => {
            onClose();
            it.onClick();
          }}
          sx={it.danger ? { color: "error.main" } : undefined}
        >
          {it.icon && (
            <ListItemIcon sx={it.danger ? { color: "error.main" } : undefined}>
              {it.icon}
            </ListItemIcon>
          )}
          <ListItemText primary={it.label} />
        </MenuItem>
      ))}
    </Menu>
  );
}

/** Icon button with tooltip, sized to the shared control height. */
export function ToolIconButton({
  title,
  onClick,
  children,
  disabled,
  active,
  color,
}: {
  title: string;
  onClick?: (e: MouseEvent<HTMLButtonElement>) => void;
  children: ReactNode;
  disabled?: boolean;
  active?: boolean;
  color?: "error" | "primary";
}) {
  return (
    <Tooltip title={title}>
      <span>
        <IconButton
          onClick={onClick}
          disabled={disabled}
          aria-label={title}
          color={color}
          sx={active ? { bgcolor: "surface.highest", color: "text.primary" } : undefined}
        >
          {children}
        </IconButton>
      </span>
    </Tooltip>
  );
}

/* ---------------------------------------------------------------- panel */

/** Right-hand side panel: header (title, subtitle, actions, close) + scrollable body + footer. */
export function SidePanel({
  title,
  subtitle,
  actions,
  onClose,
  onBack,
  children,
  footer,
  width = sizes.panel,
}: {
  title: ReactNode;
  subtitle?: ReactNode;
  actions?: ReactNode;
  onClose?: () => void;
  onBack?: () => void;
  children: ReactNode;
  footer?: ReactNode;
  width?: number;
}) {
  return (
    <Box
      component="aside"
      sx={{
        width,
        flexShrink: 0,
        display: "flex",
        flexDirection: "column",
        bgcolor: "surface.base",
        borderLeft: 1,
        borderColor: "border.light",
        minHeight: 0,
      }}
    >
      <Box
        sx={{
          display: "flex",
          alignItems: "center",
          gap: 1,
          px: 2,
          minHeight: 56,
          borderBottom: 1,
          borderColor: "border.light",
        }}
      >
        {onBack && (
          <IconButton onClick={onBack} aria-label="Back" sx={{ ml: -1 }}>
            <ArrowBackRoundedIcon fontSize="small" />
          </IconButton>
        )}
        <Box sx={{ flex: 1, minWidth: 0 }}>
          <Typography variant="subtitle1" noWrap>
            {title}
          </Typography>
          {subtitle && (
            <Typography variant="caption" color="text.secondary" noWrap sx={{ display: "block" }}>
              {subtitle}
            </Typography>
          )}
        </Box>
        {actions}
        {onClose && (
          <IconButton onClick={onClose} aria-label="Close panel" sx={{ mr: -1 }}>
            <CloseRoundedIcon fontSize="small" />
          </IconButton>
        )}
      </Box>
      <Box
        sx={{
          flex: 1,
          minHeight: 0,
          overflowY: "auto",
          p: 1.5,
          display: "flex",
          flexDirection: "column",
          gap: 1.5,
        }}
      >
        {children}
      </Box>
      {footer && (
        <Box
          sx={{
            p: 1.5,
            borderTop: 1,
            borderColor: "border.light",
            display: "flex",
            gap: 1,
            "& > .MuiButton-root": { flex: 1, whiteSpace: "nowrap", minWidth: 0, px: 1 },
          }}
        >
          {footer}
        </Box>
      )}
    </Box>
  );
}

/* ------------------------------------------------------------- settings */

/** One row inside a settings card: label (+ hint) on the left, control on the right. */
export function SettingRow({
  label,
  hint,
  control,
  last,
}: {
  label: ReactNode;
  hint?: ReactNode;
  control: ReactNode;
  last?: boolean;
}) {
  return (
    <Box
      sx={{
        display: "flex",
        alignItems: "center",
        gap: 2,
        minHeight: sizes.row,
        py: 0.75,
        borderBottom: last ? 0 : 1,
        borderColor: "border.light",
      }}
    >
      <Box sx={{ flex: 1, minWidth: 0 }}>
        <Typography variant="body1">{label}</Typography>
        {hint && (
          <Typography variant="caption" color="text.secondary" sx={{ display: "block" }}>
            {hint}
          </Typography>
        )}
      </Box>
      <Box sx={{ flexShrink: 0, display: "flex", alignItems: "center", gap: 1 }}>{control}</Box>
    </Box>
  );
}

/* --------------------------------------------------------------- states */

export function Loading({ pt = 8 }: { pt?: number }) {
  return (
    <Box sx={{ display: "flex", justifyContent: "center", pt }}>
      <CircularProgress size={22} thickness={4} />
    </Box>
  );
}

/** Field label sitting above a control (no floating labels). */
export function FieldLabel({ children }: { children: ReactNode }) {
  return (
    <Typography variant="body2" color="text.secondary" sx={{ mb: 0.5, fontWeight: 500 }}>
      {children}
    </Typography>
  );
}

/** Label + control + optional hint, stacked. */
export function Field({
  label,
  hint,
  children,
  sx,
}: {
  label: ReactNode;
  hint?: ReactNode;
  children: ReactNode;
  sx?: SxProps<Theme>;
}) {
  return (
    <Box sx={[{ minWidth: 0 }, ...sxList(sx)]}>
      <FieldLabel>{label}</FieldLabel>
      {children}
      {hint && (
        <Typography variant="caption" color="text.secondary" sx={{ display: "block", mt: 0.5 }}>
          {hint}
        </Typography>
      )}
    </Box>
  );
}
