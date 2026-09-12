import { Box, Chip, type ChipProps } from "@mui/material";
import type { TagInfo } from "@/ipc/types";

/** Palette offered in the tag manager (Termius-like, muted on dark surfaces). */
export const TAG_COLORS = [
  "#2bb884",
  "#5aa9e6",
  "#b07cf2",
  "#f25e61",
  "#f8aa4b",
  "#e3c04b",
  "#4bc8c1",
  "#ef7fb6",
  "#8d99ae",
] as const;

/** `label → colour` lookup for card chips (host cards carry only tag labels). */
export const tagColorMap = (tags: readonly TagInfo[] | undefined): ReadonlyMap<string, string> =>
  new Map((tags ?? []).flatMap((t): [string, string][] => (t.color ? [[t.label, t.color]] : [])));

/** Small colour swatch shown in front of a coloured tag. */
export function TagDot({ color, size = 8 }: { color: string | null | undefined; size?: number }) {
  if (!color) return null;
  return (
    <Box
      component="span"
      sx={{
        width: size,
        height: size,
        borderRadius: "50%",
        bgcolor: color,
        flexShrink: 0,
        display: "inline-block",
      }}
    />
  );
}

/** Tag chip that carries the tag colour as a leading dot. */
export function TagChip({
  label,
  color,
  selected = false,
  ...rest
}: {
  label: string;
  color?: string | null;
  /** Filled accent chip (active filter / chosen tag). */
  selected?: boolean;
} & Pick<ChipProps, "onClick" | "onDelete" | "variant">) {
  return (
    <Chip
      size="small"
      label={label}
      color={selected ? "primary" : "default"}
      variant={rest.variant ?? (selected ? "filled" : "outlined")}
      icon={color && !selected ? <TagDot color={color} /> : undefined}
      onClick={rest.onClick}
      onDelete={rest.onDelete}
      sx={{ "& .MuiChip-icon": { ml: "8px", mr: "-4px" } }}
    />
  );
}
