/** Detects macOS from the browser user agent (testable via injectable override). */
export function detectMacos(): boolean {
  return navigator.userAgent.includes("Mac");
}
