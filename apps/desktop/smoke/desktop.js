// Native desktop smoke, injected into the webview of a *debug* build by
// TERMOSO_SMOKE_SCRIPT (src-tauri/src/smoke.rs). Drives the UI the way a
// first-time user would — welcome screen → hosts → local terminal → settings —
// and checks that a command typed into the terminal comes back through the
// shell integration. Every step is reported through `smoke_report`; a line
// starting with `shot <name>` asks the harness for a screenshot, the last line
// is `DONE` or `FAIL <reason>`.
(async () => {
  const invoke = (cmd, args) => window.__TAURI_INTERNALS__.invoke(cmd, args ?? {});
  const report = (line) => invoke("smoke_report", { line: String(line) }).catch(() => undefined);
  const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
  const shot = async (name) => {
    await sleep(600);
    await report(`shot ${name}`);
    await sleep(1500);
  };

  const visible = (el) => {
    const r = el.getBoundingClientRect();
    return r.width > 0 && r.height > 0;
  };
  const byText = (selector, text) =>
    [...document.querySelectorAll(selector)].find(
      (el) => visible(el) && el.textContent.trim() === text,
    );
  const waitFor = async (what, probe, ms = 20000) => {
    const end = Date.now() + ms;
    while (Date.now() < end) {
      const v = await probe();
      if (v) return v;
      await sleep(200);
    }
    throw new Error(`timeout waiting for ${what}`);
  };
  const click = (el) => {
    el.scrollIntoView({ block: "center" });
    const r = el.getBoundingClientRect();
    const opts = {
      bubbles: true,
      cancelable: true,
      clientX: r.x + r.width / 2,
      clientY: r.y + r.height / 2,
    };
    for (const type of ["pointerdown", "mousedown", "pointerup", "mouseup", "click"]) {
      el.dispatchEvent(new MouseEvent(type, opts));
    }
  };
  // xterm takes printable characters from `input` events on its hidden
  // textarea and control keys from `keydown`, so synthetic events exercise the
  // real browser input path down to the PTY. xterm calls preventDefault on
  // every key it consumed; a synthetic Enter it did not consume (WebKit may
  // ignore `keyCode` in the init dict) is sent as a CR insertText instead.
  const insert = (textarea, data) =>
    textarea.dispatchEvent(
      new InputEvent("input", { data, inputType: "insertText", bubbles: true, cancelable: true }),
    );
  const typeInto = (textarea, text) => {
    const notes = new Set();
    for (const ch of text) {
      if (ch === "\n") {
        const ev = new KeyboardEvent("keydown", {
          key: "Enter",
          code: "Enter",
          keyCode: 13,
          bubbles: true,
          cancelable: true,
        });
        textarea.dispatchEvent(ev);
        if (ev.defaultPrevented) notes.add("enter=keydown");
        else {
          notes.add("enter=insertText");
          insert(textarea, "\r");
        }
      } else {
        insert(textarea, ch);
      }
    }
    return [...notes].join(",");
  };
  const xtermGl = () => {
    for (const canvas of document.querySelectorAll(".xterm canvas")) {
      const gl = canvas.getContext("webgl2") || canvas.getContext("webgl");
      if (gl) return gl;
    }
    return null;
  };
  const rowsText = () =>
    [...document.querySelectorAll(".xterm-rows > div")].map((r) => r.textContent).join("\n");

  try {
    const info = await invoke("app_info");
    await report(`app ${info.version} platform=${info.platform} master=${info.masterKeySource}`);

    await waitFor("welcome screen", () => byText("button", "Continue offline"));
    await shot("welcome");
    click(byText("button", "Continue offline"));

    const terminalButton = await waitFor("hosts toolbar", () => byText("button", "Terminal"));
    await report("hosts page shown");
    await shot("hosts");

    click(terminalButton);
    const textarea = await waitFor("terminal", () =>
      document.querySelector(".xterm-helper-textarea"),
    );
    const sessions = await waitFor(
      "local session",
      async () => {
        const list = await invoke("sessions_list");
        return list.length ? list : null;
      },
      15000,
    );
    await report(
      `session ${sessions[0].id} protocol=${sessions[0].protocol} target=${sessions[0].target}`,
    );
    await sleep(2500); // let the shell start and the integration install
    const gl = xtermGl();
    if (gl) {
      const dbg = gl.getExtension("WEBGL_debug_renderer_info");
      const name = dbg
        ? gl.getParameter(dbg.UNMASKED_RENDERER_WEBGL)
        : gl.getParameter(gl.RENDERER);
      await report(`renderer webgl (${name}) lost=${gl.isContextLost()}`);
    } else {
      await report(`renderer dom rows=${document.querySelectorAll(".xterm-rows > div").length}`);
    }
    await shot("terminal");

    const marker = `termoso-smoke-${Date.now().toString(36)}`;
    const command = `echo ${marker}`;
    textarea.focus();
    const typed = typeInto(textarea, `${command}\n`);
    await report(`typed command through the xterm textarea (${typed})`);
    const recorded = async () =>
      (await invoke("history_commands", { limit: 50 })).some((h) => h.data.command === command);
    let via = "input events";
    try {
      await waitFor("command in history", async () => (await recorded()) || null, 12000);
    } catch {
      via = "terminal_write";
      await report("typed input not recorded, retrying through terminal_write");
      // ^U first: whatever the typed attempt left on the line must not merge.
      await invoke("terminal_write", { id: sessions[0].id, data: `\x15${command}\r` });
      await waitFor(
        "command in history (terminal_write)",
        async () => (await recorded()) || null,
        12000,
      );
    }
    await report(`shell integration recorded the command (input via ${via})`);
    await shot("terminal-output");

    // Simulate a GPU context loss (sleep / eGPU unplug on a real Mac): the
    // WebGL addon must hand over to the DOM renderer without losing the
    // buffer, which also lets us read the screen as text.
    if (gl) {
      const lose = gl.getExtension("WEBGL_lose_context");
      if (!lose) throw new Error("WEBGL_lose_context unavailable");
      lose.loseContext();
    }
    const screen = await waitFor(
      "echo output on the DOM-rendered screen",
      () => {
        const text = rowsText();
        return text.split(marker).length > 2 ? text : null;
      },
      15000,
    );
    await report(`screen has ${screen.split("\n").length} rows, echo output visible`);
    await shot("terminal-dom");

    // The sidebar only exists on the Vaults tab; the terminal tab hides it.
    click(await waitFor("vaults tab", () => byText("[role=tab]", "Vaults")));
    click(await waitFor("settings nav", () => byText("div[role=button], a, button", "Settings")));
    await waitFor("settings page", () => byText("div[role=button], a, button", "Account & sync"));
    click(await waitFor("general section", () => byText("div[role=button], a, button", "General")));
    await waitFor("general settings", () => document.body.textContent.includes("Appearance"));
    await shot("settings");

    await report("DONE");
  } catch (e) {
    await report(`FAIL ${e && e.message ? e.message : e}`);
    await shot("failure");
  }
})();
