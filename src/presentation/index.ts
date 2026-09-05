import { createElement } from "react";
import { createRoot } from "react-dom/client";
import { APPLICATION_LAYER, applicationUsesDomain } from "../application/index";
import { DOMAIN_LAYER } from "../domain/index";
import { INFRASTRUCTURE_LAYER, infrastructureUsesDomain } from "../infrastructure/index";
import { App } from "./App";

/** Keeps layer modules in the dependency graph for architecture checks. */
const LAYER_MARKER = [
  DOMAIN_LAYER,
  applicationUsesDomain(),
  APPLICATION_LAYER,
  infrastructureUsesDomain(),
  INFRASTRUCTURE_LAYER,
  "presentation",
].join(":");

/** Presentation composition root for the frontend shell. */
export function renderApp(root: HTMLElement): void {
  root.dataset.gijirecLayers = LAYER_MARKER;
  createRoot(root).render(createElement(App));
}
