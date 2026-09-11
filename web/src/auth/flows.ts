import { authApi } from "@/api/endpoints";
import type { AuthResponse, MfaCredential, MfaMethod, Session } from "@/api/types";
import {
  beginLogin,
  beginRegistration,
  createAccountKeys,
  loadCrypto,
  recoveryVerifier,
  rewrapPrivateKey,
  rotateRecovery,
  unlockAccount,
  unlockWithRecovery,
} from "@/crypto";
import { authStore, deviceInfo } from "./store";

export type LoginOutcome =
  | { kind: "done"; session: Session }
  | { kind: "mfa"; mfaToken: string; methods: MfaMethod[] }
  | { kind: "approval"; approvalToken: string; emailHint: string };

interface PendingLogin {
  email: string;
  /** OPAQUE export key (base64); needed to unwrap the private key once the server authenticates us. */
  exportKey: string;
}

let pending: PendingLogin | null = null;

export function hasPendingLogin(): boolean {
  return pending !== null;
}

export function clearPendingLogin() {
  pending = null;
}

function finishAuth(resp: AuthResponse): LoginOutcome {
  switch (resp.status) {
    case "mfa_required":
      return { kind: "mfa", mfaToken: resp.mfa_token, methods: resp.methods };
    case "device_approval_required":
      return { kind: "approval", approvalToken: resp.approval_token, emailHint: resp.email_hint };
    case "authenticated": {
      const { status: _status, ...session } = resp;
      let privateKey: string | null = null;
      if (pending) {
        privateKey = unlockAccount(
          pending.exportKey,
          session.keys.wrapped_private_key,
          session.keys.public_key,
        );
        pending = null;
      }
      authStore.signIn(session, privateKey);
      return { kind: "done", session };
    }
  }
}

export async function login(
  email: string,
  password: string,
  ssoSession?: string,
): Promise<LoginOutcome> {
  await loadCrypto();
  const step = beginLogin(password, email);
  const start = await authApi.loginStart(email, step.request, deviceInfo(), ssoSession);
  const out = step.finish(start.opaque_response);
  pending = { email, exportKey: out.exportKey };
  try {
    const resp = await authApi.loginFinish(start.login_id, out.finalization);
    return finishAuth(resp);
  } catch (e) {
    pending = null;
    throw e;
  }
}

export async function verifyMfa(
  mfaToken: string,
  credential: MfaCredential,
): Promise<LoginOutcome> {
  return finishAuth(await authApi.mfaVerify(mfaToken, credential));
}

export async function approveDevice(approvalToken: string, code: string): Promise<LoginOutcome> {
  return finishAuth(await authApi.deviceApprove(approvalToken, code));
}

export interface RegisterInput {
  email: string;
  password: string;
  displayName?: string;
  inviteToken?: string;
  ssoSession?: string;
}

export interface RegisterOutput {
  session: Session;
  recoveryPhrase: string;
}

export async function register(input: RegisterInput): Promise<RegisterOutput> {
  await loadCrypto();
  const step = beginRegistration(input.password, input.email);
  const start = await authApi.registerStart(input.email, step.request);
  const reg = step.finish(start.opaque_response);
  const keys = createAccountKeys(reg.exportKey);
  const displayName = input.displayName?.trim();
  const resp = await authApi.registerFinish({
    email: input.email,
    opaque_upload: reg.upload,
    display_name: displayName === undefined || displayName === "" ? undefined : displayName,
    device: deviceInfo(),
    keys: {
      public_key: keys.publicKey,
      wrapped_private_key: keys.wrappedPrivateKey,
      recovery_wrapped_private_key: keys.recoveryWrappedPrivateKey,
      recovery_verifier: keys.recoveryVerifier,
      personal_vault_sealed_key: keys.personalVaultSealedKey,
    },
    invite_token: input.inviteToken,
    sso_session: input.ssoSession,
  });
  if (resp.status !== "authenticated") {
    throw new Error("Unexpected server response during registration");
  }
  const { status: _status, ...session } = resp;
  authStore.signIn(session, keys.privateKey);
  return { session, recoveryPhrase: keys.recoveryPhrase };
}

export interface RecoverOutput {
  session: Session;
  /** Fresh recovery phrase (the old one is invalidated). */
  recoveryPhrase: string;
}

export async function recoverAccount(
  email: string,
  phrase: string,
  newPassword: string,
): Promise<RecoverOutput> {
  await loadCrypto();
  const verifier = recoveryVerifier(phrase);
  const start = await authApi.recoveryStart(email, verifier);
  const privateKey = unlockWithRecovery(
    phrase,
    start.recovery_wrapped_private_key,
    start.public_key,
  );
  const step = beginRegistration(newPassword, email);
  const pw = await authApi.passwordStart(step.request, start.recovery_token);
  const reg = step.finish(pw.opaque_response);
  const rotation = rotateRecovery(privateKey);
  const resp = await authApi.passwordFinish({
    recovery_token: start.recovery_token,
    opaque_upload: reg.upload,
    wrapped_private_key: rewrapPrivateKey(privateKey, reg.exportKey),
    new_recovery: {
      recovery_wrapped_private_key: rotation.recoveryWrappedPrivateKey,
      recovery_verifier: rotation.recoveryVerifier,
    },
    revoke_other_sessions: true,
    device: deviceInfo(),
  });
  if (resp.status !== "authenticated") {
    throw new Error("Unexpected server response during recovery");
  }
  const { status: _status, ...session } = resp;
  authStore.signIn(session, privateKey);
  return { session, recoveryPhrase: rotation.recoveryPhrase };
}

/** Authenticated password change; requires the private key to be unlocked in this tab. */
export async function changePassword(
  newPassword: string,
  revokeOtherSessions: boolean,
): Promise<void> {
  const { session, privateKey } = authStore.get();
  if (!session || !privateKey) throw new Error("Unlock your account first");
  await loadCrypto();
  const step = beginRegistration(newPassword, session.user.email);
  const pw = await authApi.passwordStart(step.request);
  const reg = step.finish(pw.opaque_response);
  const resp = await authApi.passwordFinish({
    opaque_upload: reg.upload,
    wrapped_private_key: rewrapPrivateKey(privateKey, reg.exportKey),
    revoke_other_sessions: revokeOtherSessions,
  });
  if (resp.status !== "authenticated") {
    throw new Error("Unexpected server response during password change");
  }
  const { status: _status, ...next } = resp;
  authStore.signIn(next, privateKey);
}

export async function logout(): Promise<void> {
  try {
    await authApi.logout();
  } finally {
    pending = null;
    authStore.signOut();
  }
}
