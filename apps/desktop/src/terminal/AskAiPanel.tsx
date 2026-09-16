import { useEffect, useRef, useState } from "react";
import { Alert, Box, Button, Chip, Stack, TextField, Tooltip, Typography } from "@mui/material";
import AutoAwesomeOutlinedIcon from "@mui/icons-material/AutoAwesomeOutlined";
import ContentCopyRoundedIcon from "@mui/icons-material/ContentCopyRounded";
import KeyboardReturnRoundedIcon from "@mui/icons-material/KeyboardReturnRounded";
import LockOutlinedIcon from "@mui/icons-material/LockOutlined";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import type { AiCommandResponse, AiStatus } from "@/ipc/types";
import { errorMessage, isDesktopError } from "@/ipc/types";
import {
  MAX_PROMPT_CHARS,
  aiErrorText,
  contextLabel,
  providerLabel,
  remainingToday,
} from "./askai";
import * as ipc from "@/ipc/commands";
import { keys, useAccount, useAiStatus } from "@/ipc/hooks";
import { goToSettings } from "@/app/navigation";
import { EmptyState } from "@/components/EmptyState";
import { useSnackbar } from "@/components/Snackbar";
import { Loading, Mono } from "@/components/ui";
import { monoFontFamily } from "@/theme/theme";
import { copyText, focusPane, pasteText, type Pane } from "./store";

export function AskAiPanel({ pane }: { pane: Pane }) {
  const account = useAccount();
  const signedIn = !!account.data?.account;
  const status = useAiStatus(signedIn);

  if (account.isPending || (signedIn && status.isPending)) return <Loading pt={4} />;
  if (!signedIn) {
    return (
      <EmptyState
        compact
        icon={<AutoAwesomeOutlinedIcon />}
        title="Sign in to ask AI"
        description="Suggestions come from your Termoso account's server, so it needs an account. Nothing else changes: no telemetry, off until you turn it on."
        action={
          <Button variant="tonal" onClick={() => goToSettings("account")}>
            Open account
          </Button>
        }
      />
    );
  }
  if (status.isError) {
    return (
      <Box sx={{ p: 1.5 }}>
        <Alert severity="error">{aiErrorText(status.error).text}</Alert>
      </Box>
    );
  }
  const s = status.data;
  if (!s) return null;
  if (!s.available) {
    return (
      <EmptyState
        compact
        icon={<AutoAwesomeOutlinedIcon />}
        title="No AI provider on this server"
        description="Self-hosted servers enable suggestions with TERMOSO_AI__API_KEY: your own Chutes key or any OpenAI-compatible endpoint."
      />
    );
  }
  if (!s.enabled) return <OptIn status={s} />;
  return <Ask pane={pane} status={s} />;
}

/** Explicit opt-in with the full disclosure; the server refuses until then. */
function OptIn({ status }: { status: AiStatus }) {
  const qc = useQueryClient();
  const snackbar = useSnackbar();
  const enable = useMutation({
    mutationFn: () => ipc.aiSetEnabled(true),
    onSuccess: (st) => qc.setQueryData(keys.ai, st),
    onError: (e) => snackbar.error(errorMessage(e)),
  });
  return (
    <Stack sx={{ p: 1.5, gap: 1.25 }}>
      <Typography variant="subtitle2">Ask AI for a command</Typography>
      <Typography variant="body2" color="text.secondary">
        Describe what you want in plain words and get one shell command back, with a short
        explanation. It is inserted into the terminal for you to review — never run for you.
      </Typography>
      <Disclosure status={status} />
      <Button
        variant="contained"
        disabled={enable.isPending}
        onClick={() => enable.mutate()}
        sx={{ alignSelf: "flex-start" }}
      >
        Turn on for my account
      </Button>
      <Typography variant="caption" color="text.secondary">
        Off by default. Turn it off any time in Settings → Account.
      </Typography>
    </Stack>
  );
}

export function Disclosure({ status }: { status: AiStatus }) {
  return (
    <Alert
      severity="info"
      icon={<LockOutlinedIcon fontSize="inherit" />}
      sx={{ "& .MuiAlert-message": { width: "100%" } }}
    >
      <Typography variant="body2" sx={{ mb: 0.5 }}>
        What leaves this computer: your request text plus two labels — the OS family and the shell
        (like <Mono>linux · bash</Mono>). Nothing from the terminal: no output, no history, no host
        name or address, no credentials, no vault contents.
      </Typography>
      <Typography variant="body2" color="text.secondary">
        Provider: {providerLabel(status)}.{" "}
        {status.confidential
          ? "Runs in confidential compute (TEE): the operator cannot read your request, though the model itself does. This is not end-to-end encryption."
          : "The provider sees your request in plain text."}{" "}
        {status.daily_quota} requests per day.
      </Typography>
    </Alert>
  );
}

function Ask({ pane, status }: { pane: Pane; status: AiStatus }) {
  const qc = useQueryClient();
  const snackbar = useSnackbar();
  const [prompt, setPrompt] = useState("");
  const [answer, setAnswer] = useState<AiCommandResponse | null>(null);
  const inputRef = useRef<HTMLTextAreaElement | null>(null);
  const connected = pane.status === "connected";

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  const ask = useMutation({
    mutationFn: (text: string) => ipc.aiAsk(text, pane.id),
    onSuccess: (r) => {
      setAnswer(r);
      qc.setQueryData<AiStatus>(keys.ai, (old) =>
        old ? { ...old, used_today: old.daily_quota - r.remaining_today } : old,
      );
    },
    onError: (e) => {
      if (isDesktopError(e) && e.kind === "ai_not_enabled") {
        qc.setQueryData<AiStatus>(keys.ai, (old) => (old ? { ...old, enabled: false } : old));
      }
    },
  });

  const submit = () => {
    const text = prompt.trim();
    if (!text || ask.isPending) return;
    setAnswer(null);
    ask.mutate(text);
  };

  const insert = () => {
    if (!answer?.command || !connected) return;
    pasteText(pane.id, answer.command);
    focusPane(pane.id);
  };

  const remaining = remainingToday(status);
  const err = ask.error ? aiErrorText(ask.error) : null;

  return (
    <Stack sx={{ p: 1.5, gap: 1.25 }}>
      <Stack direction="row" sx={{ alignItems: "center", gap: 1 }}>
        <Typography variant="subtitle2" sx={{ flex: 1 }}>
          Ask AI
        </Typography>
        <Tooltip
          title={
            status.confidential
              ? "Confidential compute (TEE): the operator cannot read requests; the model does. Not end-to-end encryption."
              : "The provider sees requests in plain text."
          }
        >
          <Chip
            size="small"
            variant="outlined"
            icon={status.confidential ? <LockOutlinedIcon /> : undefined}
            label={providerLabel(status)}
            sx={{ maxWidth: 180 }}
          />
        </Tooltip>
      </Stack>
      <TextField
        inputRef={inputRef}
        multiline
        minRows={2}
        maxRows={5}
        size="small"
        placeholder="e.g. find files over 100 MB modified this week"
        value={prompt}
        onChange={(e) => setPrompt(e.target.value.slice(0, MAX_PROMPT_CHARS))}
        onKeyDown={(e) => {
          if (e.key === "Enter" && !e.shiftKey) {
            e.preventDefault();
            submit();
          }
        }}
        slotProps={{ htmlInput: { maxLength: MAX_PROMPT_CHARS, "aria-label": "Request" } }}
      />
      <Stack direction="row" sx={{ alignItems: "center", gap: 1 }}>
        <Typography variant="caption" color="text.secondary" sx={{ flex: 1 }} noWrap>
          Sends: request · {contextLabel(pane)}
        </Typography>
        <Button
          variant="contained"
          size="small"
          disabled={!prompt.trim() || ask.isPending}
          onClick={submit}
        >
          {ask.isPending ? "Asking…" : "Suggest"}
        </Button>
      </Stack>

      {err && (
        <Alert
          severity={err.retry ? "warning" : "info"}
          action={
            err.retry ? (
              <Button color="inherit" size="small" onClick={submit}>
                Retry
              </Button>
            ) : undefined
          }
        >
          {err.text}
        </Alert>
      )}

      {answer && (
        <Stack sx={{ gap: 1 }}>
          {answer.command ? (
            <Box
              sx={{
                p: 1.25,
                borderRadius: 1.5,
                bgcolor: "surface.lowest",
                fontFamily: monoFontFamily,
                fontSize: 13,
                whiteSpace: "pre-wrap",
                wordBreak: "break-all",
                userSelect: "text",
              }}
              data-testid="ai-command"
            >
              {answer.command}
            </Box>
          ) : (
            <Alert severity="info">No command for that request.</Alert>
          )}
          {answer.explanation && (
            <Typography variant="body2" color="text.secondary">
              {answer.explanation}
            </Typography>
          )}
          {answer.command && (
            <Stack direction="row" sx={{ gap: 1 }}>
              <Tooltip
                title={
                  connected
                    ? "Types the command at the prompt. You press Enter."
                    : "Connect the session to insert"
                }
              >
                <span>
                  <Button
                    variant="tonal"
                    size="small"
                    startIcon={<KeyboardReturnRoundedIcon />}
                    disabled={!connected}
                    onClick={insert}
                  >
                    Insert
                  </Button>
                </span>
              </Tooltip>
              <Button
                size="small"
                color="inherit"
                startIcon={<ContentCopyRoundedIcon />}
                onClick={() => {
                  void copyText(answer.command);
                  snackbar.notify("Command copied");
                }}
              >
                Copy
              </Button>
            </Stack>
          )}
        </Stack>
      )}

      <Typography variant="caption" color="text.secondary">
        Review before you run it: the model can be wrong. Nothing is executed for you. {remaining}{" "}
        of {status.daily_quota} left today.
      </Typography>
    </Stack>
  );
}
