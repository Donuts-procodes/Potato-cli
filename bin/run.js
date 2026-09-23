#!/usr/bin/env node

"use strict";

const { spawnSync } = require("child_process");
const path = require("path");
const fs = require("fs");

const PLATFORMS = {
  "darwin-arm64": "@potato-cli/darwin-arm64/bin/potato",
  "darwin-x64": "@potato-cli/darwin-x64/bin/potato",
  "linux-x64": "@potato-cli/linux-x64/bin/potato",
  "linux-arm64": "@potato-cli/linux-arm64/bin/potato",
  "win32-x64": "@potato-cli/win32-x64/bin/potato.exe",
};

const key = `${process.platform}-${process.arch}`;
const target = PLATFORMS[key];

if (!target) {
  console.error(`Error: Unsupported platform or architecture: ${key}`);
  console.error(`Supported platforms: ${Object.keys(PLATFORMS).join(", ")}`);
  process.exit(1);
}

let binaryPath;
try {
  binaryPath = require.resolve(target);
} catch (_resolveErr) {
  // Fallback for local monorepo / development builds
  const binaryName = process.platform === "win32" ? "potato.exe" : "potato";
  const localFallback = path.join(__dirname, "..", "npm", key, "bin", binaryName);
  if (fs.existsSync(localFallback)) {
    binaryPath = localFallback;
  } else {
    console.error(`Error: Could not resolve native binary for platform "${key}".`);
    console.error(`Expected package: ${target}`);
    console.error(`Local fallback checked: ${localFallback}`);
    console.error(`Ensure the platform package is installed (npm install).`);
    process.exit(1);
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
