import { useEffect, useState } from "react";
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
import { onPrompt, onPromptClosed, promptAnswer } from "@/ipc/commands";
import type { HostKeyInfo, PromptAnswer, PromptEvent } from "@/ipc/types";
import { setActiveTab, terminalStore } from "@/terminal/store";
import { Field } from "@/components/ui";

/**
 * Renders the queue of prompts raised by Rust while connecting (host key,
 * password, passphrase, keyboard-interactive). Answers go straight back to
 * Rust; nothing typed here is retained by the UI.
 */
export function PromptHost() {
  const [queue, setQueue] = useState<PromptEvent[]>([]);

  useEffect(() => {
    const unlisten = Promise.all([
      onPrompt((p) => {
        setQueue((q) => (q.some((x) => x.id === p.id) ? q : [...q, p]));
        const tab = terminalStore.get().tabs.find((t) => t.paneIds.includes(p.session_id));
        if (tab) setActiveTab(tab.id);
      }),
      onPromptClosed((c) => setQueue((q) => q.filter((x) => x.id !== c.id))),
    ]);
    return () => {
      void unlisten.then((fns) => fns.forEach((f) => f()));
    };
  }, []);

  const current = queue[0];
  if (!current) return null;

  const answer = (a: PromptAnswer) => {
    setQueue((q) => q.filter((x) => x.id !== current.id));
    void promptAnswer(current.id, a);
  };

  return <PromptDialog key={current.id} prompt={current} onAnswer={answer} />;
}

function PromptDialog({
  prompt,
  onAnswer,
}: {
  prompt: PromptEvent;
  onAnswer: (a: PromptAnswer) => void;
}) {
  const cancel = () => onAnswer({ kind: "cancel" });
  switch (prompt.kind) {
    case "host_key":
      return <HostKeyPrompt prompt={prompt} onAnswer={onAnswer} />;
    case "certificate":
      return <CertificatePrompt prompt={prompt} onAnswer={onAnswer} />;
    case "password":
      return (
        <SecretPrompt
          title={`Password for ${prompt.username}`}
          target={prompt.target}
          label="Password"
          warning={prompt.retry ? "Authentication failed. Try again." : null}
          onAnswer={onAnswer}
          onCancel={cancel}
        />
      );
    case "passphrase":
      return (
        <SecretPrompt
          title="Key passphrase"
          target={prompt.target}
          label={`Passphrase for ${prompt.key_label}`}
          onAnswer={onAnswer}
          onCancel={cancel}
        />
      );
    case "pin":
      return (
        <SecretPrompt
          title="Please enter PIN for key"
          target={prompt.target}
          label={`PIN for ${prompt.key_label}`}
          numeric
          rememberLabel={null}
          warning={
            prompt.retry
              ? prompt.retries === null
                ? "Wrong PIN. Try again."
                : `Wrong PIN. ${prompt.retries} ${prompt.retries === 1 ? "attempt" : "attempts"} left before the key locks.`
              : null
          }
          onAnswer={onAnswer}
          onCancel={cancel}
        />
      );
    case "interactive":
      return <InteractivePrompt prompt={prompt} onAnswer={onAnswer} />;
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
        {tone === "old" ? "Previously trusted" : tone === "new" ? "Presented now" : info.key_type}
        {tone ? ` · ${info.key_type}` : ""}
      </Typography>
      {info.fingerprint}
    </Box>
  );
}

function HostKeyPrompt({
  prompt,
  onAnswer,
}: {
  prompt: Extract<PromptEvent, { kind: "host_key" }>;
  onAnswer: (a: PromptAnswer) => void;
}) {
  const v = prompt.verdict;
  const changed = v.status === "changed";
  const [verified, setVerified] = useState(false);
  const decide = (decision: "reject" | "accept_once" | "accept_and_save") =>
    onAnswer({ kind: "host_key", decision });
  return (
    <Dialog open onClose={() => decide("reject")} maxWidth="sm" fullWidth>
      <DialogTitle>{changed ? "Host key has changed" : "Unknown host"}</DialogTitle>
      <DialogContent>
        <Stack spacing={1.5}>
          {changed ? (
            <Alert severity="error" variant="outlined">
              The identity of <b>{prompt.target}</b> differs from the key saved earlier. This can
              mean the server was reinstalled — or that someone is intercepting the connection.
            </Alert>
          ) : (
            <Typography variant="body2">
              The authenticity of <b>{prompt.target}</b> cannot be established. Verify the
              fingerprint with the server owner before trusting it.
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
                    I verified the new fingerprint with the server owner
                  </Typography>
                }
              />
            </>
          )}
        </Stack>
      </DialogContent>
      <DialogActions sx={{ px: 3, pb: 2 }}>
        <Button color="inherit" onClick={() => decide("reject")}>
          Reject
        </Button>
        <Box sx={{ flex: 1 }} />
        <Button color={changed ? "error" : "primary"} onClick={() => decide("accept_once")}>
          Connect once
        </Button>
        <Button
          variant="contained"
          color={changed ? "error" : "primary"}
          disabled={changed && !verified}
          onClick={() => decide("accept_and_save")}
        >
          {changed ? "Replace & connect" : "Trust & connect"}
        </Button>
      </DialogActions>
    </Dialog>
  );
}

function CertificatePrompt({
  prompt,
  onAnswer,
}: {
  prompt: Extract<PromptEvent, { kind: "certificate" }>;
  onAnswer: (a: PromptAnswer) => void;
}) {
  const decide = (decision: "reject" | "accept_once" | "accept_and_save") =>
    onAnswer({ kind: "host_key", decision });
  return (
    <Dialog open onClose={() => decide("reject")} maxWidth="sm" fullWidth>
      <DialogTitle>Untrusted certificate</DialogTitle>
      <DialogContent>
        <Stack spacing={1.5}>
          <Typography variant="body2">
            The certificate presented by <b>{prompt.host}</b> is not signed by a trusted authority
            (self-signed or private CA). Verify the fingerprint with the server owner before
            trusting it.
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
      </DialogContent>
      <DialogActions sx={{ px: 3, pb: 2 }}>
        <Button color="inherit" onClick={() => decide("reject")}>
          Reject
        </Button>
        <Box sx={{ flex: 1 }} />
        <Button onClick={() => decide("accept_once")}>Connect once</Button>
        <Button variant="contained" onClick={() => decide("accept_and_save")}>
          Trust & connect
        </Button>
      </DialogActions>
    </Dialog>
  );
}

function SecretPrompt({
  title,
  target,
  label,
  warning,
  numeric,
  rememberLabel = "Save to this host in the vault",
  onAnswer,
  onCancel,
}: {
  title: string;
  target: string;
  label: string;
  warning?: string | null;
  numeric?: boolean;
  /** `null` hides the remember checkbox (the secret is used once, never stored). */
  rememberLabel?: string | null;
  onAnswer: (a: PromptAnswer) => void;
  onCancel: () => void;
}) {
  const [value, setValue] = useState("");
  const [remember, setRemember] = useState(false);
  const submit = () => onAnswer({ kind: "secret", value, remember });
  return (
    <Dialog open onClose={onCancel} maxWidth="xs" fullWidth>
      <DialogTitle>{title}</DialogTitle>
      <DialogContent>
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
              type="password"
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
      </DialogContent>
      <DialogActions sx={{ px: 3, pb: 2 }}>
        <Button color="inherit" onClick={onCancel}>
          Cancel
        </Button>
        <Button variant="contained" onClick={submit}>
          Continue
        </Button>
      </DialogActions>
    </Dialog>
  );
}

function InteractivePrompt({
  prompt,
  onAnswer,
}: {
  prompt: Extract<PromptEvent, { kind: "interactive" }>;
  onAnswer: (a: PromptAnswer) => void;
}) {
  const [answers, setAnswers] = useState<string[]>(() => prompt.questions.map(() => ""));
  const submit = () => onAnswer({ kind: "interactive", answers });
  const cancel = () => onAnswer({ kind: "cancel" });
  return (
    <Dialog open onClose={cancel} maxWidth="xs" fullWidth>
      <DialogTitle>{prompt.name || "Authentication"}</DialogTitle>
      <DialogContent>
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
      </DialogContent>
      <DialogActions sx={{ px: 3, pb: 2 }}>
        <Button color="inherit" onClick={cancel}>
          Cancel
        </Button>
        <Button variant="contained" onClick={submit}>
          Continue
        </Button>
      </DialogActions>
    </Dialog>
  );
}
