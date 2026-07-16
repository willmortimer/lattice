#!/usr/bin/env node
/**
 * Publish the curated public guide from site/docs/.
 *
 * The repository-level docs/ directory remains the canonical architecture and
 * roadmap corpus. Public docs are intentionally smaller, product-facing, and
 * versioned separately so future specifications are not presented as shipped
 * behavior. Pages link to canonical source documents when deeper detail helps.
 */

import {
  cpSync,
  existsSync,
  mkdirSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync,
} from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const siteRoot = resolve(here, '..');
const source = resolve(siteRoot, 'docs');
const output = resolve(siteRoot, 'src', 'content', 'docs', 'docs');
const generated = resolve(siteRoot, 'src', 'generated');
const navigationPath = resolve(source, 'navigation.json');

if (!existsSync(source) || !statSync(source).isDirectory()) {
  throw new Error(`[sync-docs] curated source not found: ${source}`);
}

const navigation = JSON.parse(readFileSync(navigationPath, 'utf8'));

rmSync(output, { recursive: true, force: true });
mkdirSync(output, { recursive: true });
cpSync(source, output, {
  recursive: true,
  filter(path) {
    return !path.endsWith('navigation.json') && !path.endsWith('README.md');
  },
});

function routeFor(file) {
  if (file === 'index.md') return '/docs/';
  return `/docs/${file.replace(/\.md$/, '')}/`;
}

const sidebar = navigation.map((group) => ({
  label: group.label,
  collapsed: group.collapsed ?? false,
  items: group.items.map((item) => ({
    label: item.label,
    link: routeFor(item.file),
  })),
}));

mkdirSync(generated, { recursive: true });
writeFileSync(
  resolve(generated, 'sidebar.json'),
  `${JSON.stringify(sidebar, null, 2)}\n`,
  'utf8',
);

console.log(`[sync-docs] published curated guide from site/docs (${sidebar.length} groups)`);
