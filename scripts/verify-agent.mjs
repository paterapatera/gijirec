import { spawnSync } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const LOG_TAIL_LINES = 80;

/** Same scope as `bun run verify`, flattened to one failure domain per step. */
const STEPS = [
  { id: "format-check", run: () => bunRun("format-check") },
  { id: "typecheck", run: () => bunRun("typecheck") },
  { id: "typecheck:test", run: () => bunRun("typecheck:test") },
  { id: "lint", run: () => bunRun("lint") },
  { id: "arch", run: () => bunRun("arch") },
  { id: "knip", run: () => bunRun("knip") },
  { id: "dup:ts", run: () => bunRun("dup:ts") },
  { id: "dup:rust", run: () => bunRun("dup:rust") },
  { id: "test", run: () => bunRun("test") },
  { id: "test:arch", run: () => bunRun("test:arch") },
  { id: "rust:format-check", run: () => bunRun("rust:format-check") },
  { id: "rust:typecheck", run: () => bunRun("rust:typecheck") },
  { id: "rust:lint", run: () => bunRun("rust:lint") },
  { id: "rust:arch", run: () => bunRun("rust:arch") },
  { id: "rust:dead-code", run: () => bunRun("rust:dead-code") },
  {
    id: "rust:test",
    run: () =>
      spawn("bun", [
        "scripts/cargo-src-tauri.mjs",
        "test",
        "--workspace",
        "--manifest-path",
        "src-tauri/Cargo.toml",
        "--",
        "--quiet",
      ]),
  },
];

const LINE_PARSERS = [
  parseBiomeFailure,
  parseTscFailure,
  parseEslintFailure,
  parseRustErrorFailure,
  parseBunTestFailure,
  parseGenericErrorFailure,
];

function parseArgs(argv) {
  const options = {
    step: null,
    from: null,
    json: false,
    report: path.join(root, ".verify-agent", "last-report.json"),
  };

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === "--step") {
      options.step = argv[i + 1] ?? null;
      i += 1;
      continue;
    }
    if (arg === "--from") {
      options.from = argv[i + 1] ?? null;
      i += 1;
      continue;
    }
    if (arg === "--json") {
      options.json = true;
      continue;
    }
    if (arg === "--report") {
      options.report = path.resolve(argv[i + 1] ?? "");
      i += 1;
      continue;
    }
    if (arg === "--help" || arg === "-h") {
      printHelp();
      process.exit(0);
    }
    throw new Error(`Unknown argument: ${arg}`);
  }

  return options;
}

function printHelp() {
  console.log(`Usage: bun scripts/verify-agent.mjs [options]

Agent-oriented wrapper for the same gates as "bun run verify".

Options:
  --step <id>     Run one step only (e.g. rust:lint, test)
  --from <id>     Resume from a step (includes that step)
  --json          Print a machine-readable summary to stdout at the end
  --report <path> Write summary JSON (default: .verify-agent/last-report.json)
  -h, --help      Show this help

Examples:
  bun run verify:agent
  bun run verify:agent -- --step rust:lint
  bun run verify:agent -- --from test
`);
}

function bunRun(script) {
  return spawn("bun", ["run", script]);
}

function spawn(command, args) {
  const result = spawnSync(command, args, {
    cwd: root,
    encoding: "utf8",
    shell: process.platform === "win32",
    maxBuffer: 20 * 1024 * 1024,
  });

  const stdout = result.stdout ?? "";
  const stderr = result.stderr ?? "";
  const output = `${stdout}${stderr}`.trimEnd();

  return {
    exitCode: result.status ?? 1,
    output,
    stdout,
    stderr,
  };
}

function selectSteps(options) {
  const ids = STEPS.map((step) => step.id);

  if (options.step) {
    if (!ids.includes(options.step)) {
      throw new Error(`Unknown step "${options.step}". Known: ${ids.join(", ")}`);
    }
    return STEPS.filter((step) => step.id === options.step);
  }

  if (options.from) {
    const index = ids.indexOf(options.from);
    if (index === -1) {
      throw new Error(`Unknown step "${options.from}". Known: ${ids.join(", ")}`);
    }
    return STEPS.slice(index);
  }

  return STEPS;
}

function extractSuccessDetail(stepId, output) {
  const passCount = output.match(/(\d+)\s+pass\b/i);
  const failCount = output.match(/(\d+)\s+fail\b/i);
  if (passCount && failCount) {
    return `${passCount[1]} pass, ${failCount[1]} fail`;
  }

  if (stepId === "rust:test" && /test result: ok/i.test(output)) {
    const passed = output.match(/(\d+)\s+passed\b/i);
    return passed ? `${passed[1]} passed` : "passed";
  }

  if (/no dependency violations found/i.test(output)) {
    return "no violations";
  }

  return null;
}

function parseBiomeFailure(trimmed) {
  const match = trimmed.match(/^(.+?):(\d+):(\d+)\s+((?:lint|format|parse|assist)\/[^\s]+)/);
  if (!match) {
    return null;
  }
  return {
    file: match[1],
    line: Number(match[2]),
    column: Number(match[3]),
    message: match[4],
  };
}

function parseTscFailure(trimmed) {
  const match = trimmed.match(/^(.+?)\((\d+),(\d+)\):\s+error\s+TS\d+:\s+(.+)$/);
  if (!match) {
    return null;
  }
  return {
    file: match[1],
    line: Number(match[2]),
    column: Number(match[3]),
    message: match[4],
  };
}

function parseEslintFailure(trimmed) {
  const match = trimmed.match(/^(.+?):(\d+):(\d+)\s+error\s+(.+)$/);
  if (!match) {
    return null;
  }
  return {
    file: match[1],
    line: Number(match[2]),
    column: Number(match[3]),
    message: match[4],
  };
}

function parseRustArrowFailure(lines, index, trimmed) {
  const match = trimmed.match(/^-->\s+(.+?):(\d+):(\d+)/);
  if (!match) {
    return null;
  }
  return {
    file: match[1],
    line: Number(match[2]),
    column: Number(match[3]),
    message: findNextMeaningfulLine(lines, index + 1),
  };
}

function parseRustErrorFailure(trimmed) {
  const match = trimmed.match(/^(error(?:\[[^\]]+])?:\s+.+)$/);
  if (!match || trimmed.includes("could not compile")) {
    return null;
  }
  return { message: match[1] };
}

function parseBunTestFailure(trimmed) {
  if (!trimmed.startsWith("(fail)")) {
    return null;
  }
  return { message: trimmed };
}

function parseGenericErrorFailure(trimmed) {
  if (!/^error:/i.test(trimmed)) {
    return null;
  }
  if (/exited with code/i.test(trimmed) || /Some errors were emitted/i.test(trimmed)) {
    return null;
  }
  return { message: trimmed };
}

function extractFailures(output) {
  const failures = [];
  const lines = output.split(/\r?\n/);

  for (let i = 0; i < lines.length; i += 1) {
    const trimmed = lines[i].trim();
    if (!trimmed) {
      continue;
    }

    const rustArrow = parseRustArrowFailure(lines, i, trimmed);
    if (rustArrow) {
      failures.push(rustArrow);
      continue;
    }

    for (const parser of LINE_PARSERS) {
      const failure = parser(trimmed);
      if (failure) {
        failures.push(failure);
        break;
      }
    }
  }

  return dedupeFailures(failures).slice(0, 12);
}

function findNextMeaningfulLine(lines, startIndex) {
  for (let i = startIndex; i < lines.length; i += 1) {
    const line = lines[i].trim();
    if (!line || line.startsWith("|") || line.startsWith("^") || line.startsWith("-->")) {
      continue;
    }
    return line;
  }
  return "see log tail";
}

function dedupeFailures(failures) {
  const seen = new Set();
  const unique = [];

  for (const failure of failures) {
    const key = [
      failure.file ?? "",
      failure.line ?? "",
      failure.column ?? "",
      failure.message ?? "",
    ].join("|");
    if (seen.has(key)) {
      continue;
    }
    seen.add(key);
    unique.push(failure);
  }

  return unique;
}

function printLogTail(output) {
  const lines = output.split(/\r?\n/);
  const tail = lines.slice(-LOG_TAIL_LINES).join("\n");
  console.log("");
  console.log("LOG_TAIL");
  console.log(tail);
}

function printHumanSummary(report) {
  console.log("");
  if (report.status === "PASS") {
    console.log(`VERIFY_STATUS: PASS (${report.completedSteps}/${report.totalSteps})`);
    return;
  }

  console.log("PRIMARY_FAILURE");
  console.log(`  step: ${report.failedStep}`);
  console.log(`  exit_code: ${report.failedExitCode}`);
  console.log(`  rerun: bun run verify:agent -- --step ${report.failedStep}`);

  if (report.failures.length > 0) {
    console.log("  findings:");
    for (const failure of report.failures) {
      if (failure.file) {
        console.log(
          `    - ${failure.file}:${failure.line ?? "?"}:${failure.column ?? "?"} ${failure.message ?? ""}`.trimEnd(),
        );
      } else {
        console.log(`    - ${failure.message ?? "unknown error"}`);
      }
    }
  }

  console.log("");
  console.log(
    `VERIFY_STATUS: FAIL (${report.completedSteps}/${report.totalSteps} complete, failed at ${report.failedStep})`,
  );
}

function writeReport(reportPath, report) {
  mkdirSync(path.dirname(reportPath), { recursive: true });
  writeFileSync(reportPath, `${JSON.stringify(report, null, 2)}\n`, "utf8");
}

function runSelectedSteps(selectedSteps) {
  const stepResults = [];
  let failedStep = null;
  let failedExitCode = null;
  let failures = [];
  const totalSteps = selectedSteps.length;

  for (let index = 0; index < selectedSteps.length; index += 1) {
    const step = selectedSteps[index];
    const label = `[verify ${String(index + 1).padStart(2, "0")}/${String(totalSteps).padStart(2, "0")}] ${step.id}`;
    const result = step.run();
    const successDetail =
      result.exitCode === 0 ? extractSuccessDetail(step.id, result.output) : null;

    stepResults.push({
      id: step.id,
      index: index + 1,
      exitCode: result.exitCode,
      status: result.exitCode === 0 ? "OK" : "FAIL",
      detail: successDetail,
    });

    if (result.exitCode === 0) {
      const detailSuffix = successDetail ? ` (${successDetail})` : "";
      console.log(`${label} ... OK${detailSuffix}`);
      continue;
    }

    console.log(`${label} ... FAIL (exit ${result.exitCode})`);
    failedStep = step.id;
    failedExitCode = result.exitCode;
    failures = extractFailures(result.output);
    printLogTail(result.output);
    break;
  }

  return { stepResults, failedStep, failedExitCode, failures, totalSteps };
}

function main() {
  const options = parseArgs(process.argv.slice(2));
  const selectedSteps = selectSteps(options);
  const startedAt = new Date().toISOString();
  const { stepResults, failedStep, failedExitCode, failures, totalSteps } =
    runSelectedSteps(selectedSteps);

  const completedSteps = stepResults.filter((step) => step.status === "OK").length;
  const report = {
    status: failedStep ? "FAIL" : "PASS",
    startedAt,
    finishedAt: new Date().toISOString(),
    totalSteps,
    completedSteps,
    failedStep,
    failedExitCode,
    failures,
    steps: stepResults,
    rerun: failedStep ? `bun run verify:agent -- --step ${failedStep}` : null,
    verifyEquivalent: "bun run verify",
  };

  printHumanSummary(report);

  if (options.report) {
    writeReport(options.report, report);
    console.log(`REPORT_JSON: ${options.report}`);
  }

  if (options.json) {
    console.log(JSON.stringify(report));
  }

  process.exit(failedStep ? 1 : 0);
}

try {
  main();
} catch (error) {
  console.error(`verify-agent: ${error instanceof Error ? error.message : String(error)}`);
  process.exit(2);
}
