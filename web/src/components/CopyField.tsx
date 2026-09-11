import { useState } from "react";
import { IconButton, InputAdornment, TextField, Tooltip } from "@mui/material";
import ContentCopyRoundedIcon from "@mui/icons-material/ContentCopyRounded";
import CheckRoundedIcon from "@mui/icons-material/CheckRounded";
import { monoFontFamily } from "@/theme/theme";

interface Props {
  label?: string;
  value: string;
  mono?: boolean;
  multiline?: boolean;
}

export function CopyField({ label, value, mono = true, multiline }: Props) {
  const [copied, setCopied] = useState(false);
  const copy = async () => {
    await navigator.clipboard.writeText(value);
    setCopied(true);
    window.setTimeout(() => setCopied(false), 1500);
  };
  return (
    <TextField
      label={label}
      value={value}
      multiline={multiline}
      slotProps={{
        input: {
          readOnly: true,
          sx: mono ? { fontFamily: monoFontFamily, fontSize: "0.875rem" } : undefined,
          endAdornment: (
            <InputAdornment position="end">
              <Tooltip title={copied ? "Copied" : "Copy"}>
                <IconButton onClick={copy} edge="end" size="small" aria-label="Copy">
                  {copied ? (
                    <CheckRoundedIcon fontSize="small" />
                  ) : (
                    <ContentCopyRoundedIcon fontSize="small" />
                  )}
                </IconButton>
              </Tooltip>
            </InputAdornment>
          ),
        },
      }}
    />
  );
}
