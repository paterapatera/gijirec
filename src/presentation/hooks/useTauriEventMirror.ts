import type { Dispatch, SetStateAction } from "react";
import { useEffect, useState } from "react";
import type { InjectableInvokeFn } from "../../infrastructure/tauri/injectableInvoke";

export interface UseTauriEventMirrorOptions<S, ListenFn, Handles> {
  /** When false, skips invoke sync and event subscriptions (default true). */
  enabled?: boolean;
  initialState: S;
  invokeFn: InjectableInvokeFn;
  listenFn: ListenFn;
  syncInitial: (
    invokeFn: InjectableInvokeFn,
    setState: Dispatch<SetStateAction<S>>,
  ) => Promise<void>;
  subscribeEvents: (
    listenFn: ListenFn,
    setState: Dispatch<SetStateAction<S>>,
    isCancelled: () => boolean,
  ) => Promise<Handles | undefined>;
  cleanupHandles: (handles: Handles) => void;
}

/**
 * Shared mount/unmount pattern for Tauri hooks: initial invoke sync, event listen, cancel guard.
 */
export function useTauriEventMirror<S, ListenFn, Handles>(
  options: UseTauriEventMirrorOptions<S, ListenFn, Handles>,
): S {
  const {
    enabled = true,
    initialState,
    invokeFn,
    listenFn,
    syncInitial,
    subscribeEvents,
    cleanupHandles,
  } = options;
  const [state, setState] = useState<S>(initialState);

  useEffect(() => {
    if (!enabled) {
      return;
    }

    let cancelled = false;
    let cleanupListeners: (() => void) | undefined;

    void syncInitial(invokeFn, setState);
    void subscribeEvents(listenFn, setState, () => cancelled).then((handles) => {
      if (handles === undefined) {
        return;
      }
      cleanupListeners = () => {
        cleanupHandles(handles);
      };
      if (cancelled) {
        cleanupListeners();
      }
    });

    return () => {
      cancelled = true;
      cleanupListeners?.();
    };
  }, [enabled, invokeFn, listenFn, syncInitial, subscribeEvents, cleanupHandles]);

  return state;
}
