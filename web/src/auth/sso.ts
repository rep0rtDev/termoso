import { authApi } from "@/api/endpoints";

const SSO_NEXT_KEY = "termoso.sso.next";

/** Redirects the browser to the identity provider; `next` is restored after the callback. */
export async function startSso(provider: string, next: string): Promise<void> {
  sessionStorage.setItem(SSO_NEXT_KEY, next);
  const redirect = `${window.location.origin}/sso/callback`;
  const r = await authApi.ssoStart(provider, redirect);
  window.location.assign(r.authorization_url);
}

export function takeSsoNext(): string {
  const v = sessionStorage.getItem(SSO_NEXT_KEY);
  sessionStorage.removeItem(SSO_NEXT_KEY);
  return v?.startsWith("/") ? v : "/account";
}
