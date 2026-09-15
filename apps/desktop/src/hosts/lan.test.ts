import { describe, expect, it } from "vitest";
import type { HostCard, LocalDevice } from "@/ipc/types";
import {
  addLabel,
  lanAddress,
  lanExisting,
  lanHostForm,
  lanHostname,
  lanLabel,
  lanSubtitle,
} from "./lan";

const V = "22222222-2222-2222-2222-222222222222";

const nas: LocalDevice = {
  name: "Home NAS",
  hostname: "nas.local.",
  addresses: ["192.168.1.20", "fe80::1"],
  port: 22,
  services: ["ssh", "sftp"],
};

const bare: LocalDevice = {
  name: "",
  hostname: "",
  addresses: ["10.0.0.7"],
  port: 2222,
  services: ["ssh"],
};

const host = (address: string, label = address): HostCard =>
  ({ id: "h", vaultId: V, label, address }) as unknown as HostCard;

describe("lan helpers", () => {
  it("strips the trailing dot and picks the address per mode", () => {
    expect(lanHostname(nas)).toBe("nas.local");
    expect(lanAddress(nas, "hostname")).toBe("nas.local");
    expect(lanAddress(nas, "ip")).toBe("192.168.1.20");
    expect(lanAddress(bare, "hostname")).toBe("10.0.0.7");
    expect(lanAddress({ ...bare, addresses: [] }, "ip")).toBeNull();
  });

  it("labels from the instance name, then the hostname, then the address", () => {
    expect(lanLabel(nas)).toBe("Home NAS");
    expect(lanLabel({ ...nas, name: "" })).toBe("nas");
    expect(lanLabel(bare)).toBe("10.0.0.7");
  });

  it("finds hosts that already point at the device by name or any address", () => {
    expect(lanExisting(nas, [host("NAS.local"), host("x")])?.address).toBe("NAS.local");
    expect(lanExisting(nas, [host("fe80::1")])).not.toBeNull();
    expect(lanExisting(nas, [host("192.168.1.21")])).toBeNull();
  });

  it("builds a host form without inventing data", () => {
    const f = lanHostForm(nas, V, null, "hostname", " admin ");
    expect(f).toMatchObject({
      vaultId: V,
      groupId: null,
      label: "Home NAS",
      address: "nas.local",
      port: null,
      username: "admin",
      ssh: true,
      password: null,
    });
    expect(lanHostForm(bare, V, V, "ip", "")?.port).toBe(2222);
    expect(lanHostForm({ ...bare, addresses: [] }, V, null, "ip", "")).toBeNull();
  });

  it("subtitle mentions non-default port and services", () => {
    expect(lanSubtitle(nas, "hostname")).toBe("nas.local · 192.168.1.20, fe80::1 · SSH + SFTP");
    expect(lanSubtitle(bare, "ip")).toBe("10.0.0.7 · port 2222 · SSH");
  });

  it("button label counts fresh and skipped devices", () => {
    expect(addLabel(0, 0)).toBe("Add hosts");
    expect(addLabel(1, 0)).toBe("Add host");
    expect(addLabel(3, 2)).toBe("Add 3 hosts (2 already added)");
  });
});
