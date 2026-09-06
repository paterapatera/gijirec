import { GlobalRegistrator } from "@happy-dom/global-registrator";

let domRegistered = false;

/** Registers happy-dom once per test run (safe across multiple test files). */
export function setupTestDom(): void {
  if (domRegistered) {
    return;
  }
  GlobalRegistrator.register();
  domRegistered = true;
}
