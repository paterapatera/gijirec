import { DOMAIN_LAYER } from "../domain/index";

/** Application layer marker — depends on domain only. */
export const APPLICATION_LAYER = "application";

export function applicationUsesDomain(): string {
  return DOMAIN_LAYER;
}
