import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fail, loadBaseline, printProcessOutput, repoRoot } from './baseline.mjs';

const baseline = loadBaseline();
const reportPath = path.join(os.tmpdir(), `sermon-studio-jest-${process.pid}.json`);
const jest = path.join(repoRoot, 'node_modules', 'jest', 'bin', 'jest.js');
const result = spawnSync(
  process.execPath,
  [jest, '--runInBand', '--json', `--outputFile=${reportPath}`],
  { cwd: repoRoot, encoding: 'utf8' },
);

printProcessOutput(result);
if (result.error) fail(`Jest could not start: ${result.error.message}`);
if (!fs.existsSync(reportPath)) fail('Jest did not produce its JSON result');

const report = JSON.parse(fs.readFileSync(reportPath, 'utf8'));
fs.rmSync(reportPath, { force: true });

if (result.status !== 0 || report.numFailedTests !== 0) {
  fail(`Jest reported ${report.numFailedTests} failed tests`);
}
if (report.numPassedTests < baseline.jestPassedMinimum) {
  fail(
    `Jest passed ${report.numPassedTests} tests; checkpoint minimum is ${baseline.jestPassedMinimum}`,
  );
}

console.log(
  `Jest guard passed: ${report.numPassedTests} passed, ${report.numFailedTests} failed.`,
);
