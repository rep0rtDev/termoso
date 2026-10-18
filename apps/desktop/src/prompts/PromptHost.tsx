import { useState, type ReactNode } from "react";
import {
  Alert,
  Box,
  Button,
  Checkbox,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  FormControlLabel,
  Stack,
  TextField,
  Typography,
} from "@mui/material";
import type { HostKeyInfo, PromptAnswer, PromptEvent } from "@/ipc/types";
import { useTerminal } from "@/terminal/store";
import { Field } from "@/components/ui";
import { answerPrompt, usePrompts } from "./store";
import { tr, trn, trx } from "@/i18n";

/**
 * Shows prompts as dialogs — except those of a terminal pane that is still on
 * its connection view, which answers them inline (see `InlinePrompt`).
 */
export function PromptHost() {
  const panes = useTerminal((s) => s.panes);
  const queue = usePrompts((s) => s.queue);
  const current =
    queue.find((p) => {
      const pane = panes[p.session_id];
      return !(pane?.status === "connecting" && !pane.reconnecting);
    }) ?? null;
  if (!current) return null;
  return (
    <PromptDialog key={current.id} prompt={current} onAnswer={(a) => answerPrompt(current.id, a)} />
  );
}

/** The same prompt rendered as a card inside a pane's connection view. */
export function InlinePrompt({ prompt }: { prompt: PromptEvent }) {
  return (
    <PromptDialog
      key={prompt.id}
      inline
      prompt={prompt}
      onAnswer={(a) => answerPrompt(prompt.id, a)}
    />
  );
}

/** Dialog chrome or (inline) a plain card: title, body, action row. */
function PromptFrame({
  inline,
  title,
  onClose,
  maxWidth,
  actions,
  children,
}: {
  inline?: boolean;
  title: ReactNode;
  onClose: () => void;
  maxWidth: "xs" | "sm";
  actions: ReactNode;
  children: ReactNode;
}) {
  if (inline) {
    return (
      <Stack
        spacing={1.5}
        sx={{
          p: 2,
          borderRadius: 2,
          bgcolor: "surface.high",
          border: 1,
          borderColor: "border.light",
        }}
      >
        <Typography variant="subtitle2" sx={{ fontWeight: 600 }}>
          {title}
        </Typography>
        {children}
        <Stack direction="row" spacing={1} sx={{ alignItems: "center", pt: 0.5 }}>
          {actions}
        </Stack>
      </Stack>
    );
  }
  return (
    <Dialog open onClose={onClose} maxWidth={maxWidth} fullWidth>
      <DialogTitle>{title}</DialogTitle>
      <DialogContent>{children}</DialogContent>
      <DialogActions sx={{ px: 3, pb: 2 }}>{actions}</DialogActions>
    </Dialog>
  );
}

function PromptDialog({
  prompt,
  inline,
  onAnswer,
}: {
  prompt: PromptEvent;
  inline?: boolean;
  onAnswer: (a: PromptAnswer) => void;
}) {
  const cancel = () => onAnswer({ kind: "cancel" });
  switch (prompt.kind) {
    case "host_key":
      return <HostKeyPrompt prompt={prompt} inline={inline} onAnswer={onAnswer} />;
    case "certificate":
      return <CertificatePrompt prompt={prompt} inline={inline} onAnswer={onAnswer} />;
    case "username":
      return (
        <SecretPrompt
          inline={inline}
          title={tr("Username for {host}", { host: prompt.host })}
          target={prompt.target}
          label={tr("Username")}
          visible
          rememberLabel={tr("Save to this host in the vault")}
          warning={prompt.retry ? tr("Enter a username to continue.") : null}
          onAnswer={onAnswer}
          onCancel={cancel}
        />
      );
    case "password":
      return (
        <SecretPrompt
          inline={inline}
          title={tr("Password for {username}", { username: prompt.username })}
          target={prompt.target}
          label={tr("Password")}
          warning={prompt.retry ? tr("Authentication failed. Try again.") : null}
          onAnswer={onAnswer}
          onCancel={cancel}
        />
      );
    case "passphrase":
      return (
        <SecretPrompt
          inline={inline}
          title={tr("Key passphrase")}
          target={prompt.target}
          label={tr("Passphrase for {key_label}", { key_label: prompt.key_label })}
          onAnswer={onAnswer}
          onCancel={cancel}
        />
      );
    case "pin":
      return (
        <SecretPrompt
          inline={inline}
          title={tr("Please enter PIN for key")}
          target={prompt.target}
          label={tr("PIN for {key_label}", { key_label: prompt.key_label })}
          numeric
          rememberLabel={null}
          warning={
            prompt.retry
              ? prompt.retries === null
                ? tr("Wrong PIN. Try again.")
                : trn(
                    prompt.retries,
                    "Wrong PIN. {count} attempt left before the key locks.",
                    "Wrong PIN. {count} attempts left before the key locks.",
                  )
              : null
          }
          onAnswer={onAnswer}
          onCancel={cancel}
        />
      );
    case "interactive":
      return <InteractivePrompt prompt={prompt} inline={inline} onAnswer={onAnswer} />;
  }
}

function KeyBlock({ info, tone }: { info: HostKeyInfo; tone?: "old" | "new" }) {
  return (
    <Box
      sx={{
        p: 1.25,
        borderRadius: 1,
        bgcolor: "background.default",
        border: 1,
        borderColor: tone === "old" ? "error.main" : tone === "new" ? "warning.main" : "divider",
        fontFamily: "monospace",
        fontSize: 12,
        userSelect: "text",
        wordBreak: "break-all",
      }}
    >
      <Typography sx={{ display: "block" }} variant="caption" color="text.secondary">
        {tone === "old"
          ? tr("Previously trusted")
          : tone === "new"
            ? tr("Presented now")
            : info.key_type}
        {tone ? ` · ${info.key_type}` : ""}
      </Typography>
      {info.fingerprint}
    </Box>
  );
}

function HostKeyPrompt({
  prompt,
  inline,
  onAnswer,
}: {
  prompt: Extract<PromptEvent, { kind: "host_key" }>;
  inline?: boolean;
  onAnswer: (a: PromptAnswer) => void;
}) {
  const v = prompt.verdict;
  const changed = v.status === "changed";
  const [verified, setVerified] = useState(false);
  const decide = (decision: "reject" | "accept_once" | "accept_and_save") =>
    onAnswer({ kind: "host_key", decision });
  return (
    <PromptFrame
      inline={inline}
      title={changed ? tr("Host key has changed") : tr("Unknown host")}
      onClose={() => decide("reject")}
      maxWidth="sm"
      actions={
        <>
          <Button color="inherit" onClick={() => decide("reject")}>
            {tr("Reject")}
          </Button>
          <Box sx={{ flex: 1 }} />
          <Button color={changed ? "error" : "primary"} onClick={() => decide("accept_once")}>
            {tr("Connect once")}
          </Button>
          <Button
            variant="contained"
            color={changed ? "error" : "primary"}
            disabled={changed && !verified}
            onClick={() => decide("accept_and_save")}
          >
            {changed ? tr("Replace & connect") : tr("Trust & connect")}
          </Button>
        </>
      }
    >
      <Stack spacing={1.5}>
        {changed ? (
          <Alert severity="error" variant="outlined">
            {trx(
              "The identity of {target} differs from the key saved earlier. This can mean the server was reinstalled — or that someone is intercepting the connection.",
              { target: <b>{prompt.target}</b> },
            )}
          </Alert>
        ) : (
          <Typography variant="body2">
            {trx(
              "The authenticity of {target} cannot be established. Verify the fingerprint with the server owner before trusting it.",
              { target: <b>{prompt.target}</b> },
            )}
          </Typography>
        )}
        {v.status === "unknown" && <KeyBlock info={v.key} />}
        {v.status === "changed" && (
          <>
            <KeyBlock info={v.old} tone="old" />
            <KeyBlock info={v.new} tone="new" />
            <FormControlLabel
              control={
                <Checkbox
                  size="small"
                  checked={verified}
                  onChange={(e) => setVerified(e.target.checked)}
                />
              }
              label={
                <Typography variant="body2">
                  {tr("I verified the new fingerprint with the server owner")}
                </Typography>
              }
            />
          </>
        )}
      </Stack>
    </PromptFrame>
  );
}

function CertificatePrompt({
  prompt,
  inline,
  onAnswer,
}: {
  prompt: Extract<PromptEvent, { kind: "certificate" }>;
  inline?: boolean;
  onAnswer: (a: PromptAnswer) => void;
}) {
  const decide = (decision: "reject" | "accept_once" | "accept_and_save") =>
    onAnswer({ kind: "host_key", decision });
  return (
    <PromptFrame
      inline={inline}
      title={tr("Untrusted certificate")}
      onClose={() => decide("reject")}
      maxWidth="sm"
      actions={
        <>
          <Button color="inherit" onClick={() => decide("reject")}>
            {tr("Reject")}
          </Button>
          <Box sx={{ flex: 1 }} />
          <Button onClick={() => decide("accept_once")}>{tr("Connect once")}</Button>
          <Button variant="contained" onClick={() => decide("accept_and_save")}>
            {tr("Trust & connect")}
          </Button>
        </>
      }
    >
      <Stack spacing={1.5}>
        <Typography variant="body2">
          {trx(
            "The certificate presented by {host} is not signed by a trusted authority (self-signed or private CA). Verify the fingerprint with the server owner before trusting it.",
            { host: <b>{prompt.host}</b> },
          )}
        </Typography>
        <KeyBlock
          info={{
            host: prompt.host,
            key_type: "SHA-256",
            fingerprint: prompt.fingerprint,
            public_key: "",
          }}
        />
      </Stack>
    </PromptFrame>
  );
}

function SecretPrompt({
  title,
  target,
  label,
  warning,
  numeric,
  visible,
  rememberLabel = "Save to this host in the vault",
  inline,
  onAnswer,
  onCancel,
}: {
  inline?: boolean;
  title: string;
  target: string;
  label: string;
  warning?: string | null;
  numeric?: boolean;
  /** Plain text field (a username, not a secret). */
  visible?: boolean;
  /** `null` hides the remember checkbox (the secret is used once, never stored). */
  rememberLabel?: string | null;
  onAnswer: (a: PromptAnswer) => void;
  onCancel: () => void;
}) {
  const [value, setValue] = useState("");
  const [remember, setRemember] = useState(false);
  const submit = () => onAnswer({ kind: "secret", value, remember });
  return (
    <PromptFrame
      inline={inline}
      title={title}
      onClose={onCancel}
      maxWidth="xs"
      actions={
        <>
          <Button color="inherit" onClick={onCancel}>
            {tr("Cancel")}
          </Button>
          <Button variant="contained" onClick={submit}>
            {tr("Continue")}
          </Button>
        </>
      }
    >
      <Stack
        component="form"
        spacing={1.5}
        onSubmit={(e) => {
          e.preventDefault();
          submit();
        }}
      >
        <Typography variant="body2" color="text.secondary">
          {target}
        </Typography>
        {warning && <Alert severity="warning">{warning}</Alert>}
        <Field label={label}>
          <TextField
            autoFocus
            type={visible ? "text" : "password"}
            value={value}
            onChange={(e) => setValue(e.target.value)}
            fullWidth
            autoComplete="off"
            inputMode={numeric ? "numeric" : undefined}
          />
        </Field>
        {rememberLabel !== null && (
          <FormControlLabel
            control={
              <Checkbox checked={remember} onChange={(e) => setRemember(e.target.checked)} />
            }
            label={rememberLabel}
          />
        )}
      </Stack>
    </PromptFrame>
  );
}

function InteractivePrompt({
  prompt,
  inline,
  onAnswer,
}: {
  prompt: Extract<PromptEvent, { kind: "interactive" }>;
  inline?: boolean;
  onAnswer: (a: PromptAnswer) => void;
}) {
  const [answers, setAnswers] = useState<string[]>(() => prompt.questions.map(() => ""));
  const submit = () => onAnswer({ kind: "interactive", answers });
  const cancel = () => onAnswer({ kind: "cancel" });
  return (
    <PromptFrame
      inline={inline}
      title={prompt.name || tr("Authentication")}
      onClose={cancel}
      maxWidth="xs"
      actions={
        <>
          <Button color="inherit" onClick={cancel}>
            {tr("Cancel")}
          </Button>
          <Button variant="contained" onClick={submit}>
            {tr("Continue")}
          </Button>
        </>
      }
    >
      <Stack
        component="form"
        spacing={1.5}
        onSubmit={(e) => {
          e.preventDefault();
          submit();
        }}
      >
        <Typography variant="body2" color="text.secondary">
          {prompt.target}
        </Typography>
        {prompt.instructions && (
          <Typography variant="body2" sx={{ whiteSpace: "pre-wrap" }}>
            {prompt.instructions}
          </Typography>
        )}
        {prompt.questions.map((q, i) => (
          <Field label={q.prompt.replace(/:\s*$/, "")}>
            <TextField
              key={i}
              autoFocus={i === 0}
              type={q.echo ? "text" : "password"}
              value={answers[i] ?? ""}
              onChange={(e) => setAnswers((a) => a.map((v, j) => (j === i ? e.target.value : v)))}
              fullWidth
              autoComplete="off"
            />
          </Field>
        ))}
      </Stack>
    </PromptFrame>
  );
}
