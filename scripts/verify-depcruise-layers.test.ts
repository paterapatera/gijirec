import { describe, expect, test } from "bun:test";
import { spawnSync } from "node:child_process";
import { cpSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const CONFIG = ".dependency-cruiser.cjs";

function depcruise(
  srcDir: string,
  cwd: string,
): { status: number | null; stdout: string; stderr: string } {
  const result = spawnSync("bunx", ["depcruise", srcDir, "--config", CONFIG], {
    cwd,
    encoding: "utf-8",
    shell: true,
  });
  return {
    status: result.status,
    stdout: result.stdout ?? "",
    stderr: result.stderr ?? "",
  };
}

function writeLayerFixture(root: string, domainImport: string): void {
  mkdirSync(join(root, "src", "domain"), { recursive: true });
  mkdirSync(join(root, "src", "presentation"), { recursive: true });
  mkdirSync(join(root, "src-tauri"), { recursive: true });

  writeFileSync(
    join(root, "tsconfig.json"),
    JSON.stringify(
      {
        compilerOptions: {
          target: "ES2022",
          module: "ESNext",
          moduleResolution: "Bundler",
          strict: true,
          noEmit: true,
        },
        include: ["src/**/*"],
      },
      null,
      2,
    ),
  );
  writeFileSync(join(root, "package.json"), JSON.stringify({ name: "fixture", type: "module" }));
  writeFileSync(
    join(root, "src", "presentation", "index.ts"),
    'export const PRESENTATION = "presentation";\n',
  );
  writeFileSync(
    join(root, "src", "domain", "index.ts"),
    `${domainImport}\nexport const DOMAIN = "domain";\n`,
  );
  writeFileSync(join(root, "src-tauri", "stub.ts"), 'export const RUST = "rust";\n');
}

describe("dependency-cruiser layer rules", () => {
  test("passes on clean workspace", () => {
    const result = depcruise("src", process.cwd());
    expect(result.status).toBe(0);
    expect(result.stdout).toContain("no dependency violations found");
  });

  test("fails when domain imports presentation", () => {
    const root = mkdtempSync(join(tmpdir(), "gijirec-depcruise-domain-"));
    const configPath = join(root, CONFIG);
    cpSync(join(process.cwd(), CONFIG), configPath);
    writeLayerFixture(
      root,
      'import { PRESENTATION } from "../presentation/index";\nvoid PRESENTATION;',
    );

    const result = depcruise("src", root);
    rmSync(root, { recursive: true, force: true });

    expect(result.status).not.toBe(0);
    expect(`${result.stdout}\n${result.stderr}`).toMatch(/domain-no-outer|forbidden/i);
  });

  test("fails when frontend imports src-tauri", () => {
    const root = mkdtempSync(join(tmpdir(), "gijirec-depcruise-rust-"));
    const configPath = join(root, CONFIG);
    cpSync(join(process.cwd(), CONFIG), configPath);
    writeLayerFixture(root, 'import { RUST } from "../../src-tauri/stub";\nvoid RUST;');

    const result = depcruise("src", root);
    rmSync(root, { recursive: true, force: true });

    expect(result.status).not.toBe(0);
    expect(`${result.stdout}\n${result.stderr}`).toMatch(/frontend-no-rust-src|forbidden/i);
  });
});
