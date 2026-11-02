import assert from "node:assert/strict";
import { MockLLMProvider } from "./mock-provider-original.mts";
import { createPRData } from "./pr-factory-original.mts";

export type { IntentCategory } from './types';

const log={warn(){},info(){}};

const VALID_INTENTS: IntentCategory[] = ['bugfix', 'feature', 'refactor', 'dependency', 'docs', 'chore'];

export interface IntentResult {
  intent: IntentCategory;
  confidence: number;
  reason: string;
}

const CONVENTIONAL_MAP: Record<string, IntentCategory> = {
  fix: 'bugfix',
  hotfix: 'bugfix',
  feat: 'feature',
  feature: 'feature',
  refactor: 'refactor',
  perf: 'refactor',
  docs: 'docs',
  doc: 'docs',
  ci: 'chore',
  build: 'chore',
  style: 'chore',
  test: 'chore',
  chore: 'chore',
};

const CONVENTIONAL_RE = /^(\w+)(\([^)]*\))?!?:/;

export class IntentClassifier {
  private provider?: LLMProvider;

  constructor(provider?: LLMProvider) {
    this.provider = provider;
  }

  /** Full classification pipeline: conventional -> LLM -> heuristic */
  async classify(title: string, body: string, changedFiles: string[]): Promise<IntentResult> {
    // 1. Try conventional commit
    const conventional = this.classifyFromTitle(title);
    if (conventional) return conventional;

    // 2. Try LLM
    if (this.provider) {
      try {
        return await this.classifyWithLLM(title, body, changedFiles);
      } catch (err) {
        log.warn({ err }, 'LLM intent classification failed, using heuristic');
      }
    }

    // 3. Heuristic fallback
    return this.classifyFromHeuristic(title, changedFiles);
  }

  classifyFromTitle(title: string): IntentResult | null {
    const match = title.match(CONVENTIONAL_RE);
    if (!match) return null;

    const prefix = match[1].toLowerCase();
    const scope = match[2] ?? '';

    // Special case: chore(deps) -> dependency
    if ((prefix === 'chore' || prefix === 'build') && /deps|dependencies/i.test(scope)) {
      return { intent: 'dependency', confidence: 1.0, reason: `Conventional commit: ${prefix}(deps)` };
    }

    const intent = CONVENTIONAL_MAP[prefix];
    if (!intent) return null;

    return { intent, confidence: 1.0, reason: `Conventional commit: ${prefix}` };
  }

  classifyFromHeuristic(title: string, changedFiles: string[]): IntentResult {
    const lower = title.toLowerCase();

    // Dependency signals
    if (/\b(bump|upgrade|update|dependabot|renovate)\b/i.test(lower)) {
      const depFiles = changedFiles.filter(f => /package\.json|package-lock|yarn\.lock|Gemfile|requirements\.txt|go\.mod|Cargo\.toml/i.test(f));
      if (depFiles.length > 0 || /bump|dependabot|renovate/i.test(lower)) {
        return { intent: 'dependency', confidence: 0.8, reason: 'Dependency update keywords' };
      }
    }

    // Docs signals
    const allDocs = changedFiles.length > 0 && changedFiles.every(f =>
      /\.(md|txt|rst|adoc)$/i.test(f) || /readme|license|changelog|contributing|docs\//i.test(f)
    );
    if (allDocs) {
      return { intent: 'docs', confidence: 0.8, reason: 'All changed files are documentation' };
    }

    // Bugfix signals
    if (/\b(fix|bug|crash|error|issue|resolve|patch|hotfix)\b/i.test(lower)) {
      return { intent: 'bugfix', confidence: 0.7, reason: 'Bugfix keywords in title' };
    }

    // Refactor signals
    if (/\b(refactor|restructure|reorganize|cleanup|clean up|simplify|extract|move)\b/i.test(lower)) {
      return { intent: 'refactor', confidence: 0.7, reason: 'Refactor keywords in title' };
    }

    // Default to feature
    return { intent: 'feature', confidence: 0.5, reason: 'Default classification' };
  }

  async classifyWithLLM(title: string, body: string, changedFiles: string[]): Promise<IntentResult> {
    const filesStr = changedFiles.slice(0, 20).join(', ');
    const input = `Title: ${title}\nBody: ${(body ?? '').slice(0, 1000)}\nFiles: ${filesStr}`.slice(0, 2000);

    const prompt = `Classify this PR/Issue intent into exactly one category: bugfix, feature, refactor, dependency, docs, chore.
Return JSON: {"intent": "<category>", "confidence": <0-1>, "reason": "<brief>"}
${input}`;

    const text = await this.provider!.generateText(prompt, { temperature: 0.1, maxTokens: 100 });

    try {
      const match = text.match(/\{[^}]+\}/);
      if (match) {
        const parsed = JSON.parse(match[0]);
        if (VALID_INTENTS.includes(parsed.intent)) {
          return {
            intent: parsed.intent,
            confidence: Math.max(0, Math.min(1, Number(parsed.confidence) || 0.5)),
            reason: String(parsed.reason ?? ''),
          };
        }
      }
    } catch { /* invalid JSON, fall through to heuristic */ }

    return this.classifyFromHeuristic(title, changedFiles);
  }
}
const tests=[];const describe=(_name,fn)=>fn();const it=(name,fn)=>tests.push({name,fn});
const expect=v=>({toBe:x=>assert.equal(v,x),toEqual:x=>assert.deepEqual(v,x),toBeNull:()=>assert.equal(v,null),toHaveLength:x=>assert.equal(v.length,x),rejects:{toThrow:async x=>assert.rejects(v,e=>e.message.includes(x))}});


describe('IntentClassifier', () => {
  describe('classifyFromTitle (conventional commit)', () => {
    it('detects bugfix from fix: prefix', () => {
      const classifier = new IntentClassifier();
      const result = classifier.classifyFromTitle('fix: resolve null pointer in auth');
      expect(result).toEqual({ intent: 'bugfix', confidence: 1.0, reason: 'Conventional commit: fix' });
    });

    it('detects feature from feat: prefix', () => {
      const classifier = new IntentClassifier();
      const result = classifier.classifyFromTitle('feat: add dark mode support');
      expect(result).toEqual({ intent: 'feature', confidence: 1.0, reason: 'Conventional commit: feat' });
    });

    it('detects refactor from refactor: prefix', () => {
      const classifier = new IntentClassifier();
      const result = classifier.classifyFromTitle('refactor(auth): extract service layer');
      expect(result).toEqual({ intent: 'refactor', confidence: 1.0, reason: 'Conventional commit: refactor' });
    });

    it('detects dependency from chore(deps): prefix', () => {
      const classifier = new IntentClassifier();
      const result = classifier.classifyFromTitle('chore(deps): bump lodash from 4.17.20 to 4.17.21');
      expect(result).toEqual({ intent: 'dependency', confidence: 1.0, reason: 'Conventional commit: chore(deps)' });
    });

    it('detects docs from docs: prefix', () => {
      const classifier = new IntentClassifier();
      const result = classifier.classifyFromTitle('docs: update API reference');
      expect(result).toEqual({ intent: 'docs', confidence: 1.0, reason: 'Conventional commit: docs' });
    });

    it('detects chore from ci:/build:/style:/test: prefixes', () => {
      const classifier = new IntentClassifier();
      expect(classifier.classifyFromTitle('ci: fix GitHub Actions workflow')!.intent).toBe('chore');
      expect(classifier.classifyFromTitle('build: update webpack config')!.intent).toBe('chore');
      expect(classifier.classifyFromTitle('test: add unit tests for auth')!.intent).toBe('chore');
    });

    it('detects bugfix from hotfix: prefix', () => {
      const classifier = new IntentClassifier();
      const result = classifier.classifyFromTitle('hotfix: critical auth fix');
      expect(result).toEqual({ intent: 'bugfix', confidence: 1.0, reason: 'Conventional commit: hotfix' });
    });

    it('detects feature from feature: prefix', () => {
      const classifier = new IntentClassifier();
      const result = classifier.classifyFromTitle('feature: add payment flow');
      expect(result).toEqual({ intent: 'feature', confidence: 1.0, reason: 'Conventional commit: feature' });
    });

    it('detects refactor from perf: prefix', () => {
      const classifier = new IntentClassifier();
      const result = classifier.classifyFromTitle('perf: optimize db queries');
      expect(result).toEqual({ intent: 'refactor', confidence: 1.0, reason: 'Conventional commit: perf' });
    });

    it('detects docs from doc: prefix', () => {
      const classifier = new IntentClassifier();
      const result = classifier.classifyFromTitle('doc: fix typo in readme');
      expect(result).toEqual({ intent: 'docs', confidence: 1.0, reason: 'Conventional commit: doc' });
    });

    it('detects dependency from build(deps): prefix', () => {
      const classifier = new IntentClassifier();
      const result = classifier.classifyFromTitle('build(deps): bump webpack from 5.0 to 5.1');
      expect(result).toEqual({ intent: 'dependency', confidence: 1.0, reason: 'Conventional commit: build(deps)' });
    });

    it('returns null for non-conventional titles', () => {
      const classifier = new IntentClassifier();
      expect(classifier.classifyFromTitle('Update the login page')).toBeNull();
      expect(classifier.classifyFromTitle('Bump dependencies')).toBeNull();
    });
  });

  describe('classifyFromHeuristic (keyword fallback)', () => {
    it('detects dependency from bump/update keywords', () => {
      const classifier = new IntentClassifier();
      const pr = createPRData({ title: 'Bump lodash to 4.17.21', changedFiles: ['package.json', 'package-lock.json'] });
      const result = classifier.classifyFromHeuristic(pr.title, pr.changedFiles);
      expect(result.intent).toBe('dependency');
    });

    it('detects docs from all-docs files', () => {
      const classifier = new IntentClassifier();
      const pr = createPRData({ title: 'Update getting started guide', changedFiles: ['README.md', 'docs/setup.md'] });
      const result = classifier.classifyFromHeuristic(pr.title, pr.changedFiles);
      expect(result.intent).toBe('docs');
    });

    it('detects bugfix from fix keywords', () => {
      const classifier = new IntentClassifier();
      const pr = createPRData({ title: 'Fix crash on login page', changedFiles: ['src/auth.ts'] });
      const result = classifier.classifyFromHeuristic(pr.title, pr.changedFiles);
      expect(result.intent).toBe('bugfix');
    });

    it('defaults to feature for unknown patterns', () => {
      const classifier = new IntentClassifier();
      const pr = createPRData({ title: 'Add new dashboard component', changedFiles: ['src/dashboard.tsx'] });
      const result = classifier.classifyFromHeuristic(pr.title, pr.changedFiles);
      expect(result.intent).toBe('feature');
    });

    it('detects refactor from refactor keywords', () => {
      const classifier = new IntentClassifier();
      const pr = createPRData({ title: 'Restructure the auth module', changedFiles: ['src/auth.ts'] });
      const result = classifier.classifyFromHeuristic(pr.title, pr.changedFiles);
      expect(result.intent).toBe('refactor');
    });

    it('detects dependency from dependabot/renovate titles', () => {
      const classifier = new IntentClassifier();
      const pr1 = createPRData({ title: 'Update lodash by dependabot', changedFiles: ['package.json'] });
      expect(classifier.classifyFromHeuristic(pr1.title, pr1.changedFiles).intent).toBe('dependency');

      const pr2 = createPRData({ title: 'Configure renovate for deps', changedFiles: ['renovate.json'] });
      expect(classifier.classifyFromHeuristic(pr2.title, pr2.changedFiles).intent).toBe('dependency');
    });
  });

  describe('classifyWithLLM', () => {
    it('parses valid LLM JSON response', async () => {
      const provider = new MockLLMProvider();
      provider.generateTextResponse = '{"intent": "bugfix", "confidence": 0.95, "reason": "Fixes null pointer"}';
      const classifier = new IntentClassifier(provider);
      const result = await classifier.classifyWithLLM('Fix null pointer', 'Resolves crash', ['src/auth.ts']);
      expect(result).toEqual({ intent: 'bugfix', confidence: 0.95, reason: 'Fixes null pointer' });
    });

    it('throws on LLM failure (classify handles fallback)', async () => {
      const provider = new MockLLMProvider();
      provider.generateTextResponse = () => { throw new Error('LLM down'); };
      const classifier = new IntentClassifier(provider);
      await expect(classifier.classifyWithLLM('fix: auth crash', 'Fixes auth', ['src/auth.ts']))
        .rejects.toThrow('LLM down');
    });

    it('falls back on invalid JSON', async () => {
      const provider = new MockLLMProvider();
      provider.generateTextResponse = 'not json at all';
      const classifier = new IntentClassifier(provider);
      const result = await classifier.classifyWithLLM('Add new feature', '', ['src/new.ts']);
      expect(result.intent).toBe('feature');
      expect(result.confidence).toBe(0.5);
    });
  });

  describe('classify (full pipeline)', () => {
    it('uses conventional commit first, skips LLM', async () => {
      const provider = new MockLLMProvider();
      const classifier = new IntentClassifier(provider);
      const result = await classifier.classify('feat: add dark mode', '', []);
      expect(result.intent).toBe('feature');
      expect(result.confidence).toBe(1.0);
      expect(provider.generateTextCalls).toHaveLength(0);
    });

    it('falls through to LLM for non-conventional titles', async () => {
      const provider = new MockLLMProvider();
      provider.generateTextResponse = '{"intent": "refactor", "confidence": 0.88, "reason": "Restructuring code"}';
      const classifier = new IntentClassifier(provider);
      const result = await classifier.classify('Restructure the auth module', '', ['src/auth.ts']);
      expect(result.intent).toBe('refactor');
      expect(provider.generateTextCalls).toHaveLength(1);
    });

    it('works without LLM provider (heuristic only)', async () => {
      const classifier = new IntentClassifier();
      const result = await classifier.classify('Fix login crash', '', ['src/auth.ts']);
      expect(result.intent).toBe('bugfix');
    });
  });
});
for(const t of tests)await t.fn();
console.log(JSON.stringify({originalCallbacks:tests.length,passed:tests.length,names:tests.map(t=>t.name),scope:'original IntentClassifier, original fixtures/mock provider; import/registration/assertion adapters only; no real model'}));
