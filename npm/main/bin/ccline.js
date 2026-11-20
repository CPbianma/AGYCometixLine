#!/usr/bin/env node
'use strict';

// The historical filename is retained only for local checkout compatibility.
const { spawnSync } = require('node:child_process');
const filesystem = require('node:fs');
const operatingSystem = require('node:os');
const path = require('node:path');

const toolDirectory = process.env.AGYLINE_HOME || path.join(
  operatingSystem.homedir(),
  '.gemini',
  'antigravity-cli',
  'agyline'
);
const executableName = process.platform === 'win32' ? 'agyline.exe' : 'agyline';
const executablePath = path.resolve(toolDirectory, executableName);

if (!filesystem.existsSync(executablePath)) {
  console.error(`agyline is not installed at ${executablePath}`);
  console.error('Build this fork with cargo build --release --locked, then run its agyline install command.');
  console.error('This local helper never downloads a binary or falls back to an upstream package or PATH.');
  process.exitCode = 1;
} else {
  const result = spawnSync(executablePath, process.argv.slice(2), {
    stdio: 'inherit',
    shell: false
  });

  if (result.error) {
    console.error(`Could not start the installed agyline: ${result.error.message}`);
    process.exitCode = 1;
  } else if (result.signal) {
    console.error(`The installed agyline terminated with signal ${result.signal}.`);
    process.exitCode = 1;
  } else {
    process.exitCode = Number.isInteger(result.status) ? result.status : 1;
  }
}
