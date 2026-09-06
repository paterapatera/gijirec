import { spawnSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const targetDir = path.join(root, "src-tauri", "target");
const args = process.argv.slice(2);
const result = spawnSync("cargo", args, {
  cwd: root,
  stdio: "inherit",
  env: { ...process.env, CARGO_TARGET_DIR: targetDir },
  shell: process.platform === "win32",
});
process.exit(result.status ?? 1);
