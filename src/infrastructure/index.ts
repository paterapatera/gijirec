import { DOMAIN_LAYER } from "../domain/index";

/** Infrastructure layer marker — depends on domain only. */
export const INFRASTRUCTURE_LAYER = "infrastructure";

export function infrastructureUsesDomain(): string {
  return DOMAIN_LAYER;
}
