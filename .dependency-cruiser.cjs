/** @type {import('dependency-cruiser').IConfiguration} */
module.exports = {
  forbidden: [
    {
      name: "no-circular",
      severity: "error",
      from: {},
      to: { circular: true },
    },
    {
      name: "domain-no-outer",
      severity: "error",
      from: { path: "^src/domain" },
      to: { path: "^src/(infrastructure|presentation|application)" },
    },
    {
      name: "application-no-presentation",
      severity: "error",
      from: { path: "^src/application" },
      to: { path: "^src/presentation" },
    },
    {
      name: "application-no-infra-deep",
      severity: "warn",
      from: { path: "^src/application" },
      to: { path: "^src/infrastructure" },
    },
    {
      name: "scripts-isolated",
      severity: "warn",
      from: { path: "^src" },
      to: { path: "^scripts" },
    },
    {
      name: "frontend-no-rust-src",
      severity: "error",
      from: { path: "^src" },
      to: { path: "^src-tauri" },
    },
  ],
  options: {
    doNotFollow: { path: "node_modules" },
    tsPreCompilationDeps: true,
    tsConfig: { fileName: "tsconfig.json" },
  },
};
