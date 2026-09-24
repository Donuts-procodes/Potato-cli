#!/usr/bin/env node
"use strict";

const fs = require("fs");
const path = require("path");

// On Windows, npm generates .ps1 wrapper scripts that fail on default PowerShell ExecutionPolicy.
// This postinstall hook removes the offending .ps1 scripts in the global npm prefix so PowerShell
// resolves pot.cmd / pot.exe directly with zero permission errors.
if (process.platform === "win32") {
  const npmPrefix = process.env.APPDATA
    ? path.join(process.env.APPDATA, "npm")
    : null;

  if (npmPrefix && fs.existsSync(npmPrefix)) {
    const scriptsToRemove = [
      path.join(npmPrefix, "pot.ps1"),
      path.join(npmPrefix, "potato.ps1"),
      path.join(npmPrefix, "potato-cli.ps1"),
    ];

    for (const file of scriptsToRemove) {
      try {
        if (fs.existsSync(file)) {
          fs.unlinkSync(file);
        }
      } catch (_) {
        // Silently ignore if already removed or permission denied
      }
    }
  }
}
