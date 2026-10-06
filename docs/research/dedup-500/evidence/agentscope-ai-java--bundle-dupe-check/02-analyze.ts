import path from 'node:path';

import { guessPackageFromPath, isUnderCwd, resolvePackage, type ModuleGraph } from './graph.ts';

export interface DuplicateCopy {
  version: string;
  /** Real directory this copy's `package.json` lives in. */
  root: string;
  /** Bytes this copy contributed to the emitted output. */
  bytes: number;
  /** Distinct modules from this copy that landed in the output. */
  moduleCount: number;
  /** Human-readable path from an entry point down to this copy, when import edges were available. */
  chain: string[];
}

export interface DuplicatePackage {
  name: string;
  copies: DuplicateCopy[];
  /** Sum of every copy's bytes. */
  totalBytes: number;
  /** `totalBytes` minus the largest copy — what a successful dedupe down to one copy would recover. */
  wastedBytes: number;
  /** True when `name` matched the `--allow` list. Still reported, but shouldn't fail CI. */
  allowed: boolean;
}

export interface AnalyzeResult {
  duplicates: DuplicatePackage[];
  /** Distinct third-party packages found in the graph, duplicated or not. */
  packagesScanned: number;
  /** Modules in the graph that resolved to a package (i.e. excluding first-party app code). */
  modulesScanned: number;
}

export interface AnalyzeOptions {
  /** Package names that may be duplicated without affecting the CI exit code. */
  allow?: string[];
}

interface Bucket {
  version: string;
  bytes: number;
  modules: Set<string>;
}

export function analyze(graph: ModuleGraph, options: AnalyzeOptions = {}): AnalyzeResult {
  const allow = new Set(options.allow ?? []);
  const cwd = graph.cwd;

  // groups: package name -> root directory -> bucket
  const groups = new Map<string, Map<string, Bucket>>();
  let modulesScanned = 0;

  for (const [modulePath, entry] of Object.entries(graph.modules)) {
    const identity = resolvePackage(modulePath, cwd) ?? guessPackageFromPath(modulePath);
    if (!identity) continue; // first-party app code, not a dependency
    if (isUnderCwd(identity.root, cwd)) continue; // resolved back to the project's own package.json

    modulesScanned++;
    let byRoot = groups.get(identity.name);
    if (!byRoot) {
      byRoot = new Map();
      groups.set(identity.name, byRoot);
    }
    let bucket = byRoot.get(identity.root);
    if (!bucket) {
      bucket = { version: identity.version, bytes: 0, modules: new Set() };
      byRoot.set(identity.root, bucket);
    }
    bucket.bytes += entry.bytes;
    bucket.modules.add(modulePath);
  }

  const hasEdges = Object.values(graph.modules).some((m) => m.imports.length > 0);
  const chains = hasEdges ? findChains(graph) : null;

  const duplicates: DuplicatePackage[] = [];
  for (const [name, byRoot] of groups) {
    if (byRoot.size < 2) continue; // one physical copy: not a duplicate, however many chunks it's split across

    const copies: DuplicateCopy[] = [];
    for (const [root, bucket] of byRoot) {
      const representative = pickRepresentative(bucket.modules, chains);
      const chain = representative
        ? labelChain(chains!.path(representative), cwd)
        : fallbackChain(bucket.modules, cwd);

      copies.push({
        version: bucket.version,
        root,
        bytes: bucket.bytes,
        moduleCount: bucket.modules.size,
        chain,
      });
    }
    copies.sort((a, b) => b.bytes - a.bytes);

    const totalBytes = copies.reduce((sum, c) => sum + c.bytes, 0);
    const wastedBytes = totalBytes - copies[0]!.bytes;

    duplicates.push({ name, copies, totalBytes, wastedBytes, allowed: allow.has(name) });
  }

  duplicates.sort((a, b) => b.wastedBytes - a.wastedBytes);

  return { duplicates, packagesScanned: groups.size, modulesScanned };
}

/** Prefer the shortest-reachable module in a bucket as the one whose import chain we display. */
function pickRepresentative(modules: Set<string>, chains: ReturnType<typeof findChains> | null): string | null {
  if (!chains) return null;
  let best: string | null = null;
  let bestDistance = Infinity;
  for (const m of modules) {
    const d = chains.distance(m);
    if (d !== undefined && d < bestDistance) {
      best = m;
      bestDistance = d;
    }
  }
  return best;
}

interface Chains {
  distance(module: string): number | undefined;
  path(module: string): string[];
}

/** Multi-source BFS from every module with no importers ("entry points"), over the forward `imports` edges. */
function findChains(graph: ModuleGraph): Chains {
  const distance = new Map<string, number>();
  const predecessor = new Map<string, string>();
  const hasImporter = new Set<string>();

  for (const entry of Object.values(graph.modules)) {
    for (const target of entry.imports) hasImporter.add(target);
  }

  const queue: string[] = [];
  for (const modulePath of Object.keys(graph.modules)) {
    if (!hasImporter.has(modulePath)) {
      distance.set(modulePath, 0);
      queue.push(modulePath);
    }
  }

  for (let i = 0; i < queue.length; i++) {
    const current = queue[i]!;
    const entry = graph.modules[current];
    if (!entry) continue;
    const d = distance.get(current)!;
    for (const target of entry.imports) {
      if (!(target in graph.modules) || distance.has(target)) continue;
      distance.set(target, d + 1);
      predecessor.set(target, current);
      queue.push(target);
    }
  }

  return {
    distance: (m) => distance.get(m),
    path: (m) => {
      const out: string[] = [];
      let cur: string | undefined = m;
      const guard = new Set<string>();
      while (cur !== undefined && !guard.has(cur)) {
        out.push(cur);
        guard.add(cur);
        cur = predecessor.get(cur);
      }
      return out.reverse();
    },
  };
}

/** Turn a module-path chain into package/file labels, collapsing consecutive hops inside the same package. */
function labelChain(modulePaths: string[], cwd: string): string[] {
  const labels: string[] = [];
  let lastRoot: string | null = null;
  for (const modulePath of modulePaths) {
    const identity = resolvePackage(modulePath, cwd) ?? guessPackageFromPath(modulePath);
    const isSelf = identity ? isUnderCwd(identity.root, cwd) : true;
    const label = identity && !isSelf ? `${identity.name}@${identity.version}` : relativeLabel(modulePath, cwd);
    const root = identity && !isSelf ? identity.root : null;
    if (root !== null && root === lastRoot) continue; // same package as the previous hop
    labels.push(label);
    lastRoot = root;
  }
  return labels;
}

function relativeLabel(modulePath: string, cwd: string): string {
  const abs = path.isAbsolute(modulePath) ? modulePath : path.resolve(cwd, modulePath);
  const rel = path.relative(cwd, abs);
  return rel.startsWith('..') ? modulePath : rel;
}

/**
 * When the graph carries no import edges at all (the `dist/` source-map path),
 * fall back to whatever nested `node_modules/<pkg>` segments the module path
 * itself encodes — a package manager's broken-hoisting layout literally spells
 * out the requester chain in the directory nesting.
 */
function fallbackChain(modules: Set<string>, cwd: string): string[] {
  for (const modulePath of modules) {
    const names = nodeModulesSegments(modulePath);
    if (names.length > 1) {
      const withVersion = names.map((name, i) => {
        if (i < names.length - 1) return name;
        const identity = resolvePackage(modulePath, cwd);
        return identity ? `${identity.name}@${identity.version}` : name;
      });
      return withVersion;
    }
  }
  return ['(no import graph available for this input — see the --stats formats for full chains)'];
}

function nodeModulesSegments(modulePath: string): string[] {
  const segments = modulePath.split(/[\\/]/);
  const names: string[] = [];
  for (let i = 0; i < segments.length; i++) {
    if (segments[i] !== 'node_modules') continue;
    let name = segments[i + 1];
    if (!name) continue;
    let next = i + 2;
    if (name.startsWith('@') && segments[i + 2]) {
      name = `${name}/${segments[i + 2]}`;
      next = i + 3;
    }
    names.push(name);
    i = next - 1;
  }
  return names;
}
