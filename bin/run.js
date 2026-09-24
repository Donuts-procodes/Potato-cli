#!/usr/bin/env node

"use strict";

const { spawnSync } = require("child_process");
const path = require("path");
const fs = require("fs");

const PLATFORMS = {
  "darwin-arm64": "@potato-agent/darwin-arm64/bin/potato",
  "darwin-x64": "@potato-agent/darwin-x64/bin/potato",
  "linux-x64": "@potato-agent/linux-x64/bin/potato",
  "linux-arm64": "@potato-agent/linux-arm64/bin/potato",
  "win32-x64": "@potato-agent/win32-x64/bin/potato.exe",
};

const key = `${process.platform}-${process.arch}`;
const target = PLATFORMS[key];

if (!target) {
  console.error(`Error: Unsupported platform or architecture: ${key}`);
  console.error(`Supported platforms: ${Object.keys(PLATFORMS).join(", ")}`);
  process.exit(1);
}

function resolveBinary() {
  // Strategy 1: Standard node require.resolve (works for npm, yarn, bun)
  try {
    return require.resolve(target);
  } catch (_) {
    // continue to next strategy
  }

  // Strategy 2: pnpm hoisted or node_modules relative path
  const packageName = target.split("/bin/")[0];
  const binName = target.split("/bin/")[1];
  const searchRoots = [
    path.join(__dirname, "..", "node_modules", packageName, "bin", binName),
    path.join(__dirname, "..", "..", packageName, "bin", binName),
    path.join(__dirname, "..", "..", "..", "node_modules", packageName, "bin", binName),
  ];

  for (const candidate of searchRoots) {
    if (fs.existsSync(candidate)) {
      return candidate;
    }
  }

  // Strategy 3: Monorepo / local development build
  const localFallback = path.join(__dirname, "..", "npm", key, "bin", binName);
  if (fs.existsSync(localFallback)) {
    return localFallback;
  }

  // Strategy 4: Cargo target directory (during cargo development)
  const cargoTarget = path.join(
    __dirname,
    "..",
    "target",
    "release",
    process.platform === "win32" ? "potato.exe" : "potato"
  );
  if (fs.existsSync(cargoTarget)) {
    return cargoTarget;
  }

  console.error(`Error: Could not resolve native binary for platform "${key}".`);
  console.error(`Expected package: ${target}`);
  console.error(`\nIf you installed via npm, bun, yarn, or pnpm, ensure optionalDependencies were not skipped.`);
  console.error(`To reinstall:`);
  console.error(`  npm install -g potato-agent`);
  console.error(`  bun add -g potato-agent`);
  console.error(`  pnpm add -g potato-agent`);
  console.error(`  yarn global add potato-agent`);
  process.exit(1);
}

const binaryPath = resolveBinary();

// Ensure binary is executable on Unix-like systems (npm/yarn extraction sometimes drops +x)
if (process.platform !== "win32") {
  try {
    const stats = fs.statSync(binaryPath);
    if ((stats.mode & 0o111) === 0) {
      fs.chmodSync(binaryPath, stats.mode | 0o755);
    }
  } catch (_) {
    // ignore chmod failures if permission denied
  }
}

// Forward all arguments, stdin, stdout, stderr to the native binary
const result = spawnSync(binaryPath, process.argv.slice(2), {
  stdio: "inherit",
  env: process.env,
});

if (result.error) {
  console.error(`Error spawning binary: ${result.error.message}`);
  process.exit(1);
}

process.exit(result.status ?? 1);
