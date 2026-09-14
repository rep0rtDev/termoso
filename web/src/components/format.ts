const dateFmt = new Intl.DateTimeFormat(undefined, {
  dateStyle: "medium",
  timeStyle: "short",
});
const dayFmt = new Intl.DateTimeFormat(undefined, { dateStyle: "medium" });

export function formatDateTime(iso: string | undefined | null): string {
  if (!iso) return "—";
  const d = new Date(iso);
  return Number.isNaN(d.getTime()) ? "—" : dateFmt.format(d);
}

export function formatDate(iso: string | undefined | null): string {
  if (!iso) return "—";
  const d = new Date(iso);
  return Number.isNaN(d.getTime()) ? "—" : dayFmt.format(d);
}

export function formatRelative(iso: string | undefined | null): string {
  if (!iso) return "—";
  const t = new Date(iso).getTime();
  if (Number.isNaN(t)) return "—";
  const diff = Date.now() - t;
  const future = diff < 0;
  const min = Math.round(Math.abs(diff) / 60_000);
  if (min < 1) return future ? "in a moment" : "just now";
  const unit = (n: number, u: string) => (future ? `in ${n} ${u}` : `${n} ${u} ago`);
  if (min < 60) return unit(min, "min");
  const h = Math.round(min / 60);
  if (h < 24) return unit(h, "h");
  const d = Math.round(h / 24);
  if (d < 30) return unit(d, "d");
  return dayFmt.format(new Date(t));
}

export function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let v = n / 1024;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  return `${v.toFixed(v >= 100 ? 0 : 1)} ${units[i]}`;
}

export function titleCase(s: string): string {
  return s.replace(/_/g, " ").replace(/\b\w/g, (c) => c.toUpperCase());
}
