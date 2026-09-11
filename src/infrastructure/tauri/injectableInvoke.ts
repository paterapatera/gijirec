import type { InvokeArgs, InvokeOptions } from "@tauri-apps/api/core";
import { invoke } from "@tauri-apps/api/core";

/** Injectable Tauri invoke for hooks, commands, and tests (looser than `typeof invoke`). */
export type InjectableInvokeFn = <T = unknown>(
  cmd: string,
  args?: InvokeArgs,
  options?: InvokeOptions,
) => Promise<T>;

/** Default production invoke wired into hooks and command wrappers. */
export const defaultInvoke: InjectableInvokeFn = invoke;

/** Widen a concrete test mock to {@link InjectableInvokeFn}. */
export function asInjectableInvokeFn(
  fn: (cmd: string, args?: InvokeArgs, options?: InvokeOptions) => unknown,
): InjectableInvokeFn {
  return fn as InjectableInvokeFn;
}
