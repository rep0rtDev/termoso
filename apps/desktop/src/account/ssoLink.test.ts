import { describe, expect, it } from "vitest";
import { isSsoLink, parseSsoLink } from "./ssoLink";

const FLOW = "Q2hlY2tfdGhpc19mbG93X2lkXzMyYnl0ZXNfXw";

describe("parseSsoLink", () => {
  it("accepts the callback the server redirects to", () => {
    expect(parseSsoLink(`termoso://sso?flow=${FLOW}`)).toBe(FLOW);
    expect(parseSsoLink(`  TERMOSO://SSO/?flow=${FLOW}\n`)).toBe(FLOW);
  });

  it("rejects other schemes, hosts and paths", () => {
    expect(parseSsoLink(`https://example.com/sso?flow=${FLOW}`)).toBeNull();
    expect(parseSsoLink(`termoso://host/${FLOW}`)).toBeNull();
    expect(parseSsoLink(`termoso://sso/callback?flow=${FLOW}`)).toBeNull();
    expect(parseSsoLink(`termoso://sso.evil?flow=${FLOW}`)).toBeNull();
    expect(parseSsoLink(`termoso://sso@evil?flow=${FLOW}`)).toBeNull();
  });

  it("rejects a missing or malformed flow id", () => {
    expect(parseSsoLink("termoso://sso")).toBeNull();
    expect(parseSsoLink("termoso://sso?")).toBeNull();
    expect(parseSsoLink("termoso://sso?flow=")).toBeNull();
    expect(parseSsoLink("termoso://sso?flow=short")).toBeNull();
    expect(parseSsoLink(`termoso://sso?flow=${FLOW}/../x`)).toBeNull();
    expect(parseSsoLink(`termoso://sso?flow=${"a".repeat(129)}`)).toBeNull();
    expect(
      parseSsoLink(`termoso://sso?flow=${encodeURIComponent("a b c d e f g h i j k l")}`),
    ).toBeNull();
  });

  it("refuses anything besides the flow id — tokens never travel in the link", () => {
    expect(parseSsoLink(`termoso://sso?flow=${FLOW}&access_token=abc`)).toBeNull();
    expect(parseSsoLink(`termoso://sso?flow=${FLOW}&flow=${FLOW}`)).toBeNull();
    expect(parseSsoLink(`termoso://sso?session=${FLOW}`)).toBeNull();
    expect(parseSsoLink(`termoso://sso?flow=${FLOW}#id_token=abc`)).toBeNull();
    expect(parseSsoLink(`termoso://sso#flow=${FLOW}`)).toBeNull();
  });
});

describe("isSsoLink", () => {
  it("recognises the callback host even when the link is malformed", () => {
    expect(isSsoLink("termoso://sso")).toBe(true);
    expect(isSsoLink("termoso://sso?flow=x")).toBe(true);
    expect(isSsoLink("termoso://sso#x")).toBe(true);
    expect(isSsoLink("termoso://ssoo")).toBe(false);
    expect(isSsoLink("termoso://host/x")).toBe(false);
    expect(isSsoLink("ssh://sso")).toBe(false);
  });
});
