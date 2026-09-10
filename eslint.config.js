import eslint from "@eslint/js";
import { defineConfig } from "eslint/config";
import sonarjs from "eslint-plugin-sonarjs";
import tseslint from "typescript-eslint";

export default defineConfig(
  {
    files: ["**/*.{js,mjs,jsx,ts,mts,tsx}"],
    extends: [
      eslint.configs.recommended,
      tseslint.configs.strictTypeChecked,
      sonarjs.configs.recommended,
    ],
    languageOptions: {
      parserOptions: {
        projectService: {
          allowDefaultProject: ["eslint.config.js", "postcss.config.js", "tailwind.config.ts"],
        },
        tsconfigRootDir: import.meta.dirname,
      },
    },
    rules: {
      "max-lines-per-function": ["error", { max: 80, skipBlankLines: true, skipComments: true }],
      "max-params": ["error", 4],
      complexity: ["error", 12],
      "sonarjs/cognitive-complexity": ["error", 15],
      "@typescript-eslint/no-explicit-any": "error",
      "@typescript-eslint/consistent-type-imports": "error",
      "no-unused-vars": "off",
      "@typescript-eslint/no-unused-vars": [
        "error",
        { argsIgnorePattern: "^_", varsIgnorePattern: "^_" },
      ],
    },
  },
  {
    // Plain JS entry points are not part of the TS project (no allowJs), so lint them
    // without type information instead of failing in the project service.
    files: ["eslint.config.js", "scripts/**/*.mjs"],
    extends: [tseslint.configs.disableTypeChecked],
  },
  {
    // Node CLI wrappers: declare Node globals and allow spawning `cargo` from PATH.
    files: ["scripts/**/*.mjs"],
    languageOptions: {
      globals: { process: "readonly" },
    },
    rules: {
      "sonarjs/no-os-command-from-path": "off",
    },
  },
  {
    ignores: [
      "node_modules/**",
      "dist/**",
      "coverage/**",
      "src-tauri/**",
      "docs/**",
      ".agents/**",
      "**/*.cjs",
      "scripts/**/*.test.ts",
      "src/**/*.test.ts",
      "src/**/*.test.tsx",
      "src/test-setup.ts",
    ],
  },
);
