# bundle-dupe-check

Find the same package bundled twice at different versions — and which
dependency pulled in each copy.

```
bundle-dupe-check found 1 duplicate package (13.7 KB could be recovered by deduping)

  date-utils  31.8 KB total, 13.7 KB wasted across 2 copies
    2.0.1          18.1 KB    1 module(s)   src/app.js -> date-picker@9.2.0 -> date-utils@2.0.1
    1.4.0          13.7 KB    1 module(s)   src/app.js -> table-widget@3.1.0 -> date-utils@1.4.0

Scanned 4 dependency module(s) across 3 package(s).
```

That's `node examples/measure.ts` — a real `analyze()` run against a real
module graph (see below). Below that is a genuine `esbuild --bundle
--metafile` build of a project with a broken hoist, run through this same
tool:

```
bundle-dupe-check found 1 duplicate package (5.9 KB could be recovered by deduping)

  shared-lib  14.8 KB total, 5.9 KB wasted across 2 copies
    2.0.0           8.9 KB    1 module(s)   src/main.js -> consumer-b@1.0.0 -> shared-lib@2.0.0
    1.0.0           5.9 KB    1 module(s)   src/main.js -> consumer-a@1.0.0 -> shared-lib@1.0.0

Scanned 4 dependency module(s) across 3 package(s).
```

## The problem

Two packages in your tree depend on incompatible versions of a third. Your
package manager can't hoist both to one copy, so it nests one inside a
dependent's own `node_modules` (or, with `file:`/workspace links, resolves it
to a different real directory entirely). Vite, Rollup, and esbuild will
happily bundle both copies — they don't second-guess your dependency graph.
The result ships twice: two copies of a state library, two copies of a
component kit, sometimes two copies of React. The symptoms are baffling
precisely because nothing about them says "duplicate version": hooks fail
with "invalid hook call", a context provider silently doesn't reach its
consumer, and the bundle is mysteriously bigger than it should be. Nothing in
a Vite or Rollup build output tells you why.

**This is a solved problem for webpack, and nowhere else.**
[`duplicate-package-checker-webpack-plugin`](https://www.npmjs.com/package/duplicate-package-checker-webpack-plugin)
has **113,194 weekly downloads** (checked against the npm downloads API) and
hasn't shipped a new version since **`3.0.0`, published 2018-03-20** — over
eight years ago. (The registry's own "last modified" timestamp on the package
reads 2022-06-16, but that's metadata churn, not a release — no version newer
than `3.0.0` exists.) It's also `webpack`-only: it reads `compilation.modules`,
a shape that doesn't exist outside webpack. `inspectpack` is actively
maintained but built the same way, around a webpack stats object. A GitHub
search for a Vite, Rollup, or esbuild equivalent turns up nothing usable —
one abandoned, unpublished, one-star repo that isn't even about the same
problem (it dedupes Vite *plugin instances*, not bundled npm packages).

### Why the obvious workaround doesn't work

- **Reading `package-lock.json` / `pnpm-lock.yaml`.** This tells you what's
  *installed*, not what's *bundled*. A duplicated install that tree-shakes
  away, or a legitimately single-copy package that a code-splitting bundler
  spreads across five chunks, both look identical from the lockfile. You need
  to know what the bundler actually decided to ship.
- **`npm ls <pkg>` / `npm dedupe`.** Same problem, plus it can't tell you the
  byte cost of each copy, and dedupe can't fix a real version conflict — only
  a compatible-range one.
- **Grepping the built bundle for a package name.** Minified output rarely
  contains a package's name at all, and even when it does, string-matching
  can't distinguish a real second copy from a shared string constant, a
  comment, or a translated identifier.

## What we investigated, and what this tool reads

The value of a tool like this lives entirely in whether it works off what a
bundler *actually* emits, so before writing anything this package's build
was verified against real output from all three:

- **esbuild's `--metafile`.** `inputs[path].imports` is exactly the forward
  import graph, and `outputs[file].inputs[path].bytesInOutput` is the exact
  post-tree-shaking byte cost per module per output — not source size.
  Read directly, no plugin needed.
- **`rollup-plugin-visualizer`'s `raw-data` (`json`) template.** Its
  `nodeMetas`/`nodeParts` structure carries the same two things — a real
  import graph (`imported`/`importedBy`) and real rendered byte sizes — for
  teams that already generate this report in CI. Read directly too.
- **Rollup's own plugin API.** Inside `generateBundle`, `this.getModuleInfo(id)`
  gives the live import graph (`importedIds`) and each chunk's `modules[id]`
  gives `renderedLength` — the actual shipped bytes, because this runs against
  the real, in-memory bundle. This is what the bundled `./rollup` plugin uses.
  Verified against a real `rollup().generate()` call, and separately against
  a real `vite build` — including one running Vite's newer Rolldown-based
  build engine, which still calls plugins with this exact shape.
- **Vite itself.** Vite's own `build.manifest` option maps entry points to
  output filenames for asset injection; it carries none of the per-module
  size or import data this tool needs. For a real answer, a plugin has to
  reach into the Rollup/Rolldown build it runs underneath — which is exactly
  what `bundle-dupe-check/rollup` does.

That gave four supported inputs: an esbuild metafile, a
`rollup-plugin-visualizer` report, this package's own plugin (for Rollup and
Vite), and — for when none of those were captured at build time — scanning a
`dist/` directory's source maps directly, attributing bytes to original files
the way `source-map-explorer` does.

Whichever input is used, package identity is resolved the same way: walk up
from a module's real (symlink-resolved) file path until a `package.json`
is found. That works for classic nested `node_modules`, for pnpm's
content-addressed store, and for npm/yarn workspaces linked with `file:` —
places where the string `node_modules` may not even appear in the resolved
path. Two copies are the same physical install iff that walk lands on the
same real directory; anything else is a real, distinct copy, whether or not
its `version` field happens to match.

## Install

```sh
npm install --save-dev bundle-dupe-check
```

Node >= 20.6. No runtime dependencies. (`rollup` is an optional peer
dependency of the `./rollup` plugin entry point only — it's never imported at
runtime, so anything that calls plugins the way Rollup does, including Vite
and Vite's Rolldown engine, works without it being installed at all.)

## Use

```sh
# Scan a build's output directory (needs sourcemaps)
npx bundle-dupe-check dist/

# Read a stats file instead — esbuild metafile, rollup-plugin-visualizer
# report, or this package's own plugin output; format is auto-detected
npx bundle-dupe-check --stats meta.json

npx bundle-dupe-check --stats meta.json --allow lodash --json
```

Exit code is `1` when an unallowed duplicate is found, `0` otherwise, `2` on
a usage error — so it drops straight into CI.

### esbuild

```sh
esbuild src/main.ts --bundle --outfile=dist/bundle.js --metafile=meta.json
npx bundle-dupe-check --stats meta.json
```

### Rollup / Vite, as a plugin

```ts
// rollup.config.js or vite.config.ts
import { bundleDupeCheck } from 'bundle-dupe-check/rollup';

export default {
  plugins: [bundleDupeCheck({ allow: ['lodash'] })],
};
```

Runs at `generateBundle`, against the real emitted bundle. Fails the build
(`this.error`) by default when an unallowed duplicate is found; pass
`failOnDuplicate: false` to only warn. `reportPath` additionally writes the
module graph as JSON, in the same format the CLI's `--stats` reads.

### rollup-plugin-visualizer

```ts
import { visualizer } from 'rollup-plugin-visualizer';
// plugins: [visualizer({ filename: 'stats.json', template: 'raw-data' })]
```

```sh
npx bundle-dupe-check --stats stats.json
```

## Options

| Option | What it does |
| --- | --- |
| `<dist-dir>` | Scan a build output directory via its source maps. |
| `--stats <file>` | Read an esbuild metafile / visualizer report / native stats file instead. |
| `--allow <names>` | Comma-separated package names allowed to be duplicated (repeatable). Still shown in the report, just doesn't fail. |
| `--json` | Print the full report as JSON. |
| `--cwd <dir>` | Project root `package.json` lookups are resolved against. Default: current directory. |

## Programmatic API

```ts
import { analyze, loadStats, fromDistDirectory, formatReport } from 'bundle-dupe-check';

const graph = loadStats('meta.json', process.cwd());
const result = analyze(graph, { allow: ['lodash'] });
console.log(formatReport(result));
```

`AnalyzeResult.duplicates` is an array of `{ name, copies, totalBytes,
wastedBytes, allowed }`, sorted by `wastedBytes` descending — so the report
always leads with the copy that actually costs something, not just the first
one found. Each `copy` carries its `version`, `bytes`, `moduleCount`, and a
`chain` of readable labels from an entry point down to that copy.

## What it does not do

- **It doesn't fix anything.** It tells you which two copies exist and who
  asked for each one; adding a `resolutions`/`overrides` entry (or upgrading
  the outdated dependency) is still on you.
- **It only sees what the bundler decided to ship.** A duplicate that gets
  fully tree-shaken away because neither copy is actually used won't show up
  — which is correct, since it never made it into the bundle.
- **`dist/` scanning needs sourcemaps**, and its byte counts are UTF-16 code
  units (JS string length) attributed by mapping span, not exact UTF-8 byte
  counts — close enough for minified, mostly-ASCII output, but an
  approximation. It also has no import graph to draw a chain from (a source
  map records where bytes came from, not who imported whom); the chain falls
  back to whatever `node_modules` nesting the source paths still show, or, if
  there's none, says so plainly instead of guessing.
- **Version/chain accuracy in `--stats` mode depends on the checkout still
  being on disk.** Package identity is resolved by reading real
  `package.json` files; analyzing a stats file on a machine (or after a clean)
  where the original `node_modules`/workspace isn't present falls back to
  parsing `node_modules/<pkg>` segments out of the path string — a name, but
  no version, and it can't tell a single hoisted copy from a true duplicate
  quite as reliably.
- **No webpack support.** That gap is already filled, however stale the
  filler; see above.

## Develop

Tests are TypeScript run directly by Node's test runner — no build, no
bundler installed:

```sh
node --test "test/*.test.ts"       # full suite, needs node 24+ for type stripping
node examples/measure.ts            # the report at the top of this README

npm run build && npm run test:dist  # what CI runs against node 20 and 22
```

The committed test fixtures include a real esbuild metafile and a real
`rollup-plugin-visualizer` report, both captured from an actual build of a
small project with a genuine version conflict (two packages depending on
different versions of a third, installed with plain `npm install`) — not
hand-typed stats. The `./rollup` plugin was additionally verified against a
real `rollup().generate()` call and a real `vite build` (including Vite's
Rolldown engine) before being committed; that throwaway project isn't part of
this repository.

## License

MIT
