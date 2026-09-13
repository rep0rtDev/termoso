import { useState } from "react";
import { Box, Button, Stack, Typography } from "@mui/material";
import ContentCopyRoundedIcon from "@mui/icons-material/ContentCopyRounded";
import DownloadRoundedIcon from "@mui/icons-material/DownloadRounded";
import { monoFontFamily } from "@/theme/theme";

interface Props {
  phrase: string;
  email?: string;
}

export function RecoveryPhraseGrid({ phrase, email }: Props) {
  const words = phrase.trim().split(/\s+/);
  const [copied, setCopied] = useState(false);

  const copy = async () => {
    await navigator.clipboard.writeText(phrase);
    setCopied(true);
    window.setTimeout(() => setCopied(false), 1500);
  };

  const download = () => {
    const body = [
      "Termoso recovery key",
      email ? `Account: ${email}` : null,
      "",
      "Keep this file offline. Anyone with these 24 words can reset your password and read your data.",
      "",
      ...words.map((w, i) => `${String(i + 1).padStart(2, " ")}. ${w}`),
      "",
    ]
      .filter((l) => l !== null)
      .join("\n");
    const blob = new Blob([body], { type: "text/plain" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = "termoso-recovery-key.txt";
    a.click();
    URL.revokeObjectURL(url);
  };

  return (
    <Stack spacing={2}>
      <Box
        sx={{
          p: 1.5,
          display: "grid",
          gridTemplateColumns: { xs: "repeat(2, 1fr)", sm: "repeat(3, 1fr)" },
          gap: 0.75,
          borderRadius: 2.5,
          bgcolor: "surface.lowest",
        }}
      >
        {words.map((w, i) => (
          <Box
            key={i}
            sx={{
              display: "flex",
              alignItems: "baseline",
              gap: 1,
              px: 1.25,
              py: 0.75,
              borderRadius: 1.5,
              bgcolor: "surface.high",
            }}
          >
            <Typography
              variant="caption"
              color="text.disabled"
              sx={{ minWidth: 16, fontFamily: monoFontFamily }}
            >
              {i + 1}
            </Typography>
            <Typography
              sx={{ fontFamily: monoFontFamily, fontSize: "0.875rem", userSelect: "all" }}
            >
              {w}
            </Typography>
          </Box>
        ))}
      </Box>
      <Stack direction="row" spacing={1} useFlexGap sx={{ flexWrap: "wrap", alignItems: "center" }}>
        <Button startIcon={<ContentCopyRoundedIcon />} onClick={copy} variant="outlined">
          {copied ? "Copied" : "Copy"}
        </Button>
        <Button startIcon={<DownloadRoundedIcon />} onClick={download} variant="outlined">
          Download .txt
        </Button>
        <Typography variant="caption" color="text.secondary" sx={{ ml: "auto" }}>
          {words.length} words
        </Typography>
      </Stack>
    </Stack>
  );
}
