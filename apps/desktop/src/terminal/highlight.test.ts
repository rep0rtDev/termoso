import { describe, expect, it } from "vitest";
import { KeywordHighlighter } from "./highlight";

const E = "\x1b";
const enc = new TextEncoder();

describe("KeywordHighlighter", () => {
  it("colours built-in keywords and addresses, restoring the default foreground", () => {
    const h = new KeywordHighlighter();
    expect(h.push("ERROR warning ok INFO debug 192.168.1.10 aa:bb:cc:dd:ee:ff\r\n")).toBe(
      `${E}[31mERROR${E}[39m ${E}[33mwarning${E}[39m ${E}[32mok${E}[39m ${E}[34mINFO${E}[39m ` +
        `${E}[35mdebug${E}[39m ${E}[95m192.168.1.10${E}[39m ${E}[95maa:bb:cc:dd:ee:ff${E}[39m\r\n`,
    );
  });

  it("matches whole words only", () => {
    const h = new KeywordHighlighter();
    expect(h.push("okay_token errorful 1.2.3.4.5")).toBe(
      `okay_token errorful ${E}[95m1.2.3.4${E}[39m.5`,
    );
  });

  it("restores the colour the program had set and leaves sequences untouched", () => {
    const h = new KeywordHighlighter();
    const out = h.push(`${E}[38;5;208mfile error here${E}[0m plain error`);
    expect(out).toBe(
      `${E}[38;5;208mfile ${E}[31merror${E}[38;5;208m here${E}[0m plain ${E}[31merror${E}[39m`,
    );
  });

  it("skips OSC payloads and joins sequences split across chunks", () => {
    const h = new KeywordHighlighter();
    const first = h.push(`${E}]0;error in title`);
    expect(first).toBe("");
    expect(h.push(`\x07ok`)).toBe(`${E}]0;error in title\x07${E}[32mok${E}[39m`);
  });

  it("decodes multi-byte UTF-8 split across chunks", () => {
    const h = new KeywordHighlighter();
    const bytes = enc.encode("ошибка error ✓");
    const cut = 3;
    const out = h.push(bytes.slice(0, cut)) + h.push(bytes.slice(cut));
    expect(out).toBe(`ошибка ${E}[31merror${E}[39m ✓`);
  });

  it("passes text through unchanged when nothing matches", () => {
    const h = new KeywordHighlighter();
    const text = `${E}[2J${E}[H$ ls -la\r\ntotal 12\r\n`;
    expect(h.push(text)).toBe(text);
  });
});
