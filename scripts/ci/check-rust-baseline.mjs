import { spawnSync } from 'node:child_process';
import { fail, loadBaseline, printProcessOutput, repoRoot } from './baseline.mjs';

const baseline = loadBaseline();
const result = spawnSync(
  'cargo',
  ['test', '--workspace', '--locked', '--color', 'never'],
  { cwd: repoRoot, encoding: 'utf8' },
);

printProcessOutput(result);
if (result.error) fail(`Cargo could not start: ${result.error.message}`);

const output = `${result.stdout ?? ''}\n${result.stderr ?? ''}`;
const summaries = [...output.matchAll(/test result: (ok|FAILED)\. (\d+) passed; (\d+) failed;/g)];
const passed = summaries.reduce((sum, match) => sum + Number(match[2]), 0);
const failed = summaries.reduce((sum, match) => sum + Number(match[3]), 0);

if (result.status !== 0 || failed !== 0) fail(`Rust tests reported ${failed} failures`);
if (summaries.length === 0) fail('Cargo produced no test summaries');
if (passed < baseline.rustPassedMinimum) {
  fail(`Rust passed ${passed} tests; checkpoint minimum is ${baseline.rustPassedMinimum}`);
}

console.log(`Rust guard passed: ${passed} passed, ${failed} failed.`);
