#!/usr/bin/env node
import { readFileSync } from 'node:fs';
import path from 'node:path';

import { analyze } from './analyze.ts';
import { BundleDupeCheckError } from './errors.ts';
import { formatReport } from './format.ts';
import { fromDistDirectory, loadStats } from './stats.ts';

const HELP = `bundle-dupe-check — find the same package bundled twice at different versions

Usage:
  bundle-dupe-check <dist-dir>          Scan a build output directory (needs sourcemaps)
  bundle-dupe-check --stats <file>      Read an esbuild metafile, a rollup-plugin-visualizer
                                         raw-data report, or this package's own plugin output

Options:
  --allow <names>   Comma-separated package names allowed to be duplicated (repeatable)
  --json            Print the full report as JSON instead of text
  --cwd <dir>       Project root package.json lookups are resolved against (default: cwd)
  -h, --help        Show this help
  -v, --version     Print the installed version

Exit codes: 0 no unallowed duplicates, 1 unallowed duplicates found, 2 usage error.
`;

function version(): string {
  try {
    const pkg = JSON.parse(readFileSync(new URL('../package.json', import.meta.url), 'utf8')) as {
      version: string;
    };
    return pkg.version;
  } catch {
    return 'unknown';
  }
}

export async function run(argv: string[]): Promise<number> {
  let statsPath: string | undefined;
  let distDir: string | undefined;
  const allow: string[] = [];
  let json = false;
  let cwd = process.cwd();

  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i]!;
    switch (arg) {
      case '--stats':
        statsPath = argv[++i];
        break;
      case '--allow':
        allow.push(
          ...(argv[++i] ?? '')
            .split(',')
            .map((s) => s.trim())
            .filter(Boolean),
        );
        break;
      case '--json':
        json = true;
        break;
      case '--cwd':
        cwd = path.resolve(argv[++i] ?? '.');
        break;
      case '-h':
      case '--help':
        console.log(HELP);
        return 0;
      case '-v':
      case '--version':
        console.log(version());
        return 0;
      default:
        if (arg.startsWith('-')) {
          console.error(`Unknown option: ${arg}\n`);
          console.error(HELP);
          return 2;
        }
        distDir = arg;
    }
  }

  if (!statsPath && !distDir) {
    console.error(HELP);
    return 2;
  }

  let graph;
  try {
    graph = statsPath ? loadStats(statsPath, cwd) : fromDistDirectory(distDir!, cwd);
  } catch (err) {
    if (err instanceof BundleDupeCheckError) {
      console.error(err.message);
      return 2;
    }
    throw err;
  }

  const result = analyze(graph, { allow });

  if (json) {
    console.log(JSON.stringify(result, null, 2));
  } else {
    console.log(formatReport(result));
  }

  const unallowed = result.duplicates.filter((d) => !d.allowed);
  return unallowed.length > 0 ? 1 : 0;
}

/* node:coverage disable */
if (process.argv[1] && import.meta.url === new URL(process.argv[1], 'file:').href) {
  run(process.argv.slice(2)).then((code) => process.exit(code));
}
/* node:coverage enable */
