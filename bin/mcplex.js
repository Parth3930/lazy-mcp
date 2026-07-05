#!/usr/bin/env node
const { spawnSync } = require('child_process');
const path = require('path');

const manifestPath = path.resolve(__dirname, '..', 'Cargo.toml');
const args = process.argv.slice(2);

const result = spawnSync('cargo', ['run', '--release', '--manifest-path', manifestPath, '--', ...args], {
    stdio: 'inherit'
});

process.exit(result.status ?? 1);
