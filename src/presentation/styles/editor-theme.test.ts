import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import path from "node:path";

const stylesDir = import.meta.dir;

function readStyle(name: string): string {
  return readFileSync(path.join(stylesDir, name), "utf8");
}

describe("editor theme CSS", () => {
  test("editor-theme.css defines v1 palette panel and lock variables", () => {
    const css = readStyle("editor-theme.css");

    expect(css).toMatch(/--jagged-ice:\s*#c1e7e6/i);
    expect(css).toMatch(/--hawkes-blue:\s*#cfdeff/i);
    expect(css).toMatch(/--classic-rose:\s*#ffccef/i);
    expect(css).toMatch(/--plum:\s*#8f3a7b/i);
    expect(css).toMatch(/--casal:\s*#2c6b6a/i);
    expect(css).toMatch(/--alto:\s*#dedede/i);
    expect(css).toMatch(/--chicago:\s*#5f5f5f/i);
    expect(css).toMatch(/--oregon:\s*#9b4100/i);
  });

  test("globals.css bridges shadcn semantic tokens and imports editor-theme", () => {
    const css = readStyle("globals.css");

    expect(css).toMatch(/editor-theme\.css/);
    expect(css).toMatch(/--primary:/);
    expect(css).toMatch(/--foreground:/);
    expect(css).toMatch(/--border:/);
    expect(css).toMatch(/@tailwind base/);
  });

  test("main entry imports globals.css", () => {
    const main = readFileSync(path.join(stylesDir, "../../main.ts"), "utf8");

    expect(main).toMatch(/globals\.css/);
  });
});
