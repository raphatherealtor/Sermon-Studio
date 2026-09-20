import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

export const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');

export function loadBaseline() {
  return JSON.parse(
    fs.readFileSync(path.join(repoRoot, 'config', 'ci-baseline.json'), 'utf8'),
  );
}

export function printProcessOutput(result) {
  if (result.stdout) process.stdout.write(result.stdout);
  if (result.stderr) process.stderr.write(result.stderr);
}

export function fail(message) {
  console.error(`\nCI guard failed: ${message}`);
  process.exit(1);
}
