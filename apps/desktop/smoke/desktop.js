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
  // xterm listens for keydown on its hidden textarea and turns the key into
  // terminal input itself, so synthetic events exercise the real input path.
  const typeInto = (textarea, text) => {
    for (const ch of text) {
      const enter = ch === "\n";
      textarea.dispatchEvent(
        new KeyboardEvent("keydown", {
          key: enter ? "Enter" : ch,
          code: enter ? "Enter" : `Key${ch.toUpperCase()}`,
          keyCode: enter ? 13 : ch.charCodeAt(0),
          bubbles: true,
          cancelable: true,
        }),
      );
    }
  };

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
    const probeCanvas = document.createElement("canvas");
    const webgl = ["webgl2", "webgl"].find((k) => probeCanvas.getContext(k)) ?? "none";
    const canvases = [...document.querySelectorAll(".xterm canvas")].map((c) => {
      const r = c.getBoundingClientRect();
      return `${Math.round(r.width)}x${Math.round(r.height)}`;
    });
    const rows = document.querySelectorAll(".xterm-rows > div").length;
    await report(`renderer webgl=${webgl} canvases=[${canvases}] domRows=${rows}`);
    await shot("terminal");

    const marker = `termoso-smoke-${Date.now().toString(36)}`;
    const command = `echo ${marker}`;
    textarea.focus();
    typeInto(textarea, `${command}\n`);
    const recorded = async () =>
      (await invoke("history_commands", { limit: 50 })).some((h) => h.data.command === command);
    let via = "keydown";
    try {
      await waitFor("command in history", async () => (await recorded()) || null, 12000);
    } catch {
      via = "terminal_write";
      await report("keydown input not recorded, retrying through terminal_write");
      await invoke("terminal_write", { id: sessions[0].id, data: `${command}\r` });
      await waitFor(
        "command in history (terminal_write)",
        async () => (await recorded()) || null,
        12000,
      );
    }
    await report(`shell integration recorded the command (input via ${via})`);
    await shot("terminal-output");

    // The sidebar only exists on the Vaults tab; the terminal tab hides it.
    click(await waitFor("vaults tab", () => byText("[role=tab]", "Vaults")));
    click(await waitFor("settings nav", () => byText("div[role=button], a, button", "Settings")));
    await waitFor("settings page", () => document.body.textContent.includes("Appearance"));
    await shot("settings");

    await report("DONE");
  } catch (e) {
    await report(`FAIL ${e && e.message ? e.message : e}`);
    await shot("failure");
  }
})();
