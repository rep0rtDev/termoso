import init, * as wasm from "./pkg/termoso_wasm";
import wasmUrl from "./pkg/termoso_wasm_bg.wasm?url";

export type { GeneratedKeyPair, NewAccountKeys, RecoveryRotation } from "./pkg/termoso_wasm";

let ready: Promise<void> | null = null;

/** Loads the WebAssembly module once; safe to call repeatedly. */
export function loadCrypto(): Promise<void> {
  ready ??= init({ module_or_path: wasmUrl }).then(() => undefined);
  return ready;
}

export function protocolVersion(): string {
  return wasm.protocol_version();
}

export interface OpaqueStep<TResult> {
  request: string;
  finish(response: string): TResult;
}

export interface RegistrationOutput {
  upload: string;
  exportKey: string;
}

export interface LoginOutput {
  finalization: string;
  exportKey: string;
}

/** OPAQUE identifier: the server binds registration records to the lowercase e-mail. */
export function opaqueUserId(email: string): string {
  return email.trim().toLowerCase();
}

export function beginRegistration(password: string, email: string): OpaqueStep<RegistrationOutput> {
  const state = new wasm.OpaqueRegistration(password);
  return {
    request: state.request,
    finish(response) {
      const out = state.finish(password, opaqueUserId(email), response);
      const result = { upload: out.upload, exportKey: out.export_key };
      out.free();
      return result;
    },
  };
}

export function beginLogin(password: string, email: string): OpaqueStep<LoginOutput> {
  const state = new wasm.OpaqueLogin(password);
  return {
    request: state.request,
    finish(response) {
      const out = state.finish(password, opaqueUserId(email), response);
      const result = { finalization: out.finalization, exportKey: out.export_key };
      out.free();
      return result;
    },
  };
}

export const createAccountKeys = wasm.create_account_keys;
export const unlockAccount = wasm.unlock_account;
export const unlockWithRecovery = wasm.unlock_with_recovery;
export const rewrapPrivateKey = wasm.rewrap_private_key;
export const rotateRecovery = wasm.rotate_recovery;
export const recoveryVerifier = wasm.recovery_verifier;
export const recoveryWords = wasm.recovery_words;
export const publicKeyOf = wasm.public_key_of;
export const generateKeyPair = wasm.generate_key_pair;
export const generateVaultKey = wasm.generate_vault_key;
export const sealVaultKey = wasm.seal_vault_key;
export const openVaultKey = wasm.open_vault_key;
export const encryptField = wasm.encrypt_field;
export const decryptField = wasm.decrypt_field;
export const encryptEntity = wasm.encrypt_entity;
export const decryptEntity = wasm.decrypt_entity;
export const encryptLabeled = wasm.encrypt_labeled;
export const decryptLabeled = wasm.decrypt_labeled;
