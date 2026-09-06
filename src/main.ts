/// <reference types="vite/client" />

import "./presentation/styles/globals.css";
import { renderApp } from "./presentation/index";

const root = document.getElementById("root");
if (root === null) {
  throw new Error("root element is missing");
}
renderApp(root);
