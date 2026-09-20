import fs from 'node:fs';
import path from 'node:path';
import { fail, repoRoot } from './baseline.mjs';

const sourceExtensions = new Set(['.js', '.jsx', '.ts', '.tsx']);
const violations = [];

function filesUnder(relativeRoot) {
  const root = path.join(repoRoot, relativeRoot);
  if (!fs.existsSync(root)) return [];
  const found = [];
  const visit = (directory) => {
    for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
      const absolute = path.join(directory, entry.name);
      if (entry.isDirectory()) visit(absolute);
      else found.push(absolute);
    }
  };
  visit(root);
  return found;
}

function relative(file) {
  return path.relative(repoRoot, file).replaceAll('\\', '/');
}

function importSpecifiers(source) {
  const specs = [];
  const staticImport = /\b(?:import|export)\s+(?:type\s+)?(?:[^'\"]*?\s+from\s+)?['\"]([^'\"]+)['\"]/g;
  const dynamicImport = /\bimport\(\s*['\"]([^'\"]+)['\"]\s*\)/g;
  for (const regex of [staticImport, dynamicImport]) {
    for (const match of source.matchAll(regex)) specs.push(match[1]);
  }
  return specs;
}

const frontendFiles = [...filesUnder('src'), ...filesUnder('ui')]
  .filter((file) => sourceExtensions.has(path.extname(file)));

for (const file of frontendFiles) {
  const rel = relative(file);
  const source = fs.readFileSync(file, 'utf8');
  const specs = importSpecifiers(source);

  const isReactComponent = rel.startsWith('src/app/') || rel.startsWith('src/components/');
  if (isReactComponent) {
    for (const spec of specs) {
      if (/MockSermonBackend|TauriSermonBackend/.test(spec)) {
        violations.push(`${rel}: React component imports concrete backend ${spec}`);
      }
    }
  }

  if (source.includes('@tauri-apps/')) {
    const allowed =
      rel === 'src/lib/backend/TauriSermonBackend.ts' ||
      rel.startsWith('src/lib/backend/__tests__/') ||
      rel === 'ui/src/api.ts';
    if (!allowed) violations.push(`${rel}: Tauri import escaped an approved adapter boundary`);
  }

  for (const spec of specs) {
    if (/^(typst|@typst|pdf-lib|pdfkit|jspdf|better-sqlite3|sqlite3|sql\.js|rusqlite)(\/|$)/.test(spec)) {
      violations.push(`${rel}: forbidden frontend backend implementation dependency ${spec}`);
    }
  }
}

for (const file of [...filesUnder('src'), ...filesUnder('ui')]) {
  if (path.extname(file) === '.typ') violations.push(`${relative(file)}: Typst belongs in the Rust core`);
}

const nextConfig = fs.readFileSync(path.join(repoRoot, 'next.config.mjs'), 'utf8');
if (!/\boutput\s*:\s*['\"]export['\"]/.test(nextConfig)) {
  violations.push('next.config.mjs: static export output is not enabled');
}

for (const file of [...filesUnder('src/app'), ...filesUnder('src/pages')]) {
  const rel = relative(file);
  if (/(^|\/)(route)\.(js|jsx|ts|tsx)$/.test(rel) || rel.startsWith('src/pages/api/')) {
    violations.push(`${rel}: API routes are forbidden by the static frontend contract`);
  }
  if (sourceExtensions.has(path.extname(file))) {
    const source = fs.readFileSync(file, 'utf8');
    if (/^[\s;]*['\"]use server['\"];?/m.test(source)) {
      violations.push(`${rel}: server actions are forbidden by the static frontend contract`);
    }
  }
}

if (violations.length > 0) fail(`architecture violations:\n- ${violations.join('\n- ')}`);
console.log('Architecture guard passed: backend boundaries and static export contract are intact.');
