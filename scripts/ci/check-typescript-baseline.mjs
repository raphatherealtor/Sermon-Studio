import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fail, loadBaseline, printProcessOutput, repoRoot } from './baseline.mjs';

const baseline = loadBaseline();
const tsc = path.join(repoRoot, 'node_modules', 'typescript', 'bin', 'tsc');
const result = spawnSync(process.execPath, [tsc, '--noEmit', '--pretty', 'false'], {
  cwd: repoRoot,
  encoding: 'utf8',
});

printProcessOutput(result);

if (result.error) fail(`TypeScript could not start: ${result.error.message}`);
if (result.signal) fail(`TypeScript ended from signal ${result.signal}`);

const output = `${result.stdout ?? ''}\n${result.stderr ?? ''}`;
const errorCount = (output.match(/\berror TS\d+:/g) ?? []).length;

if (result.status !== 0 && errorCount === 0) {
  fail(`TypeScript exited ${result.status} without countable diagnostics`);
}
if (errorCount > baseline.typescriptErrorMaximum) {
  fail(
    `TypeScript errors increased to ${errorCount}; checkpoint maximum is ${baseline.typescriptErrorMaximum}`,
  );
}

console.log(
  `TypeScript guard passed: ${errorCount}/${baseline.typescriptErrorMaximum} allowed errors.`,
);
