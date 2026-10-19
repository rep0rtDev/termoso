import type { ApiErrorBody } from "./types";
import { msg, tr } from "@/i18n";

export const API_BASE = "/api/v1";

export class ApiError extends Error {
  readonly status: number;
  readonly code: string;
  readonly details: unknown;

  constructor(status: number, body: ApiErrorBody) {
    super(body.message);
    this.name = "ApiError";
    this.status = status;
    this.code = body.code;
    this.details = body.details;
  }
}

/** Server error codes with a translated wording; anything else shows the server's English message. */
const CODE_MESSAGES: Record<string, string> = {
  permission_denied: msg("Your role does not allow this action."),
  mfa_required: msg("This team requires two-factor authentication."),
  invalid_credentials: msg("Wrong email or password."),
  rate_limited: msg("Too many attempts. Try again later."),
  turnstile_required: msg("Please confirm you are human and try again."),
  email_not_verified: msg("Confirm your email address first."),
  not_found: msg("Not found."),
  conflict: msg("This already exists."),
  network: msg("Cannot reach the server. Check your connection."),
  sso_required: msg("This account must sign in through single sign-on (SSO)."),
  client_update_required: msg(
    "This client is older than the server allows. Please update Termoso.",
  ),
  ip_not_allowed: msg("Sign-in from this network is not allowed by your team's IP allow-list."),
  device_pending_approval: msg(
    "This device is waiting for approval by a team admin. You will be able to sign in once it is approved.",
  ),
  device_rejected: msg("This device was rejected by a team admin. Ask them to approve it again."),
};

export function errorMessage(err: unknown): string {
  if (err instanceof ApiError) {
    const known = CODE_MESSAGES[err.code];
    if (known) {
      const perm = (err.details as { permission?: string } | null)?.permission;
      return perm ? `${tr(known)} (${perm})` : tr(known);
    }
    return err.message;
  }
  if (err instanceof TypeError)
    return tr(CODE_MESSAGES.network ?? "Cannot reach the server. Check your connection.");
  if (err instanceof Error) return err.message;
  return tr("Something went wrong");
}

type TokenSource = () => string | null;
type UnauthorizedHandler = () => void;

let tokenSource: TokenSource = () => null;
let onUnauthorized: UnauthorizedHandler = () => undefined;

export function configureClient(opts: { token: TokenSource; onUnauthorized: UnauthorizedHandler }) {
  tokenSource = opts.token;
  onUnauthorized = opts.onUnauthorized;
}

export interface RequestOptions {
  /** Skip the bearer header even when a session exists (public auth endpoints). */
  anonymous?: boolean;
  /** Override the bearer token (e.g. right after login before the store updates). */
  token?: string;
  query?: object;
  signal?: AbortSignal;
}

async function parseError(res: Response): Promise<ApiError> {
  let body: ApiErrorBody = { code: "http_error", message: `HTTP ${res.status}` };
  try {
    const json: unknown = await res.json();
    if (isErrorBody(json)) body = json;
  } catch {
    // non-JSON error body (proxy, gateway…) – keep the generic message
  }
  return new ApiError(res.status, body);
}

function isErrorBody(v: unknown): v is ApiErrorBody {
  return (
    typeof v === "object" &&
    v !== null &&
    typeof (v as { code?: unknown }).code === "string" &&
    typeof (v as { message?: unknown }).message === "string"
  );
}

export interface ResponseWithStatus<T> {
  status: number;
  data: T;
}

export async function request<T>(
  method: string,
  path: string,
  body?: unknown,
  opts: RequestOptions = {},
): Promise<T> {
  const r = await requestWithStatus<T>(method, path, body, opts);
  return r.data;
}

export async function requestWithStatus<T>(
  method: string,
  path: string,
  body?: unknown,
  opts: RequestOptions = {},
): Promise<ResponseWithStatus<T>> {
  const url = new URL(API_BASE + path, window.location.origin);
  if (opts.query) {
    for (const [k, v] of Object.entries(opts.query) as [string, unknown][]) {
      if (typeof v === "string" && v !== "") url.searchParams.set(k, v);
      else if (typeof v === "number" || typeof v === "boolean") url.searchParams.set(k, String(v));
    }
  }
  const headers = new Headers({ Accept: "application/json" });
  if (body !== undefined) headers.set("Content-Type", "application/json");
  const token = opts.anonymous ? null : (opts.token ?? tokenSource());
  if (token) headers.set("Authorization", `Bearer ${token}`);

  const res = await fetch(url, {
    method,
    headers,
    body: body === undefined ? undefined : JSON.stringify(body),
    credentials: "omit",
    cache: "no-store",
    signal: opts.signal ?? null,
  });

  if (!res.ok) {
    const err = await parseError(res);
    if (res.status === 401 && !opts.anonymous && !opts.token) onUnauthorized();
    throw err;
  }
  const text = await res.text();
  const data = text.length === 0 ? (undefined as T) : (JSON.parse(text) as T);
  return { status: res.status, data };
}

/** Send a raw body (an image) and parse the JSON reply. */
export async function upload<T>(method: string, path: string, blob: Blob): Promise<T> {
  const headers = new Headers({ Accept: "application/json" });
  if (blob.type !== "") headers.set("Content-Type", blob.type);
  const token = tokenSource();
  if (token) headers.set("Authorization", `Bearer ${token}`);
  const res = await fetch(new URL(API_BASE + path, window.location.origin), {
    method,
    headers,
    body: blob,
    credentials: "omit",
    cache: "no-store",
  });
  if (!res.ok) {
    const err = await parseError(res);
    if (res.status === 401) onUnauthorized();
    throw err;
  }
  return (await res.json()) as T;
}

/** Fetch a binary resource with the session token; `null` when it does not exist. */
export async function fetchBlob(path: string): Promise<Blob | null> {
  const headers = new Headers();
  const token = tokenSource();
  if (token) headers.set("Authorization", `Bearer ${token}`);
  const res = await fetch(new URL(API_BASE + path, window.location.origin), {
    headers,
    credentials: "omit",
  });
  if (res.status === 404) return null;
  if (!res.ok) throw await parseError(res);
  return res.blob();
}

export const http = {
  get: <T>(path: string, opts?: RequestOptions) => request<T>("GET", path, undefined, opts),
  post: <T>(path: string, body?: unknown, opts?: RequestOptions) =>
    request<T>("POST", path, body, opts),
  put: <T>(path: string, body?: unknown, opts?: RequestOptions) =>
    request<T>("PUT", path, body, opts),
  patch: <T>(path: string, body?: unknown, opts?: RequestOptions) =>
    request<T>("PATCH", path, body, opts),
  delete: <T>(path: string, body?: unknown, opts?: RequestOptions) =>
    request<T>("DELETE", path, body, opts),
};
