import assert from "node:assert/strict";
import { ConcurrencyController } from "./concurrency-original.mts";
import { createIssueData } from "./pr-factory-original.mts";

export type { IntentCategory } from './types';

const intentLog={info(){},warn(){}};

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
        intentLog.warn({ err }, 'LLM intent classification failed, using heuristic');
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

const log={info(){},warn(){}};

const SPAM_THRESHOLD = 25;

export class IssueScoringEngine {
  private provider?: LLMProvider;
  private intentClassifier: IntentClassifier;
  private concurrency: ConcurrencyController;

  constructor(provider?: LLMProvider, maxConcurrent = 10) {
    this.provider = provider;
    this.intentClassifier = new IntentClassifier(provider);
    this.concurrency = new ConcurrencyController(maxConcurrent);
  }

  async scoreMany(issues: IssueData[]): Promise<ScoredIssue[]> {
    if (issues.length === 0) return [];
    log.info({ count: issues.length }, 'Scoring issues');

    const results = await Promise.allSettled(
      issues.map(issue => this.concurrency.execute(() => this.score(issue)))
    );

    const scored: ScoredIssue[] = [];
    for (const [i, result] of results.entries()) {
      if (result.status === 'fulfilled') {
        scored.push(result.value);
      } else {
        log.warn({ issue: issues[i].number, err: result.reason }, 'Failed to score issue');
      }
    }
    return scored;
  }

  async score(issue: IssueData): Promise<ScoredIssue> {
    const intentResult = await this.intentClassifier.classify(issue.title, issue.body ?? '', []);

    const signals: SignalScore[] = [
      this.scoreStaleness(issue),
      this.scoreBodyQuality(issue),
      this.scoreLabelPriority(issue),
      this.scoreActivity(issue),
      this.scoreContributor(issue),
      this.scoreSpam(issue),
      this.scoreMilestone(issue),
      this.scoreReactions(issue),
      this.scoreLinkedPR(issue),
      this.scoreAssignee(issue),
      this.scoreReproducibility(issue),
      this.scoreIntent(intentResult),
    ];

    const totalWeight = signals.reduce((sum, s) => sum + s.weight, 0);
    const totalScore = totalWeight > 0
      ? Math.round(signals.reduce((sum, s) => sum + s.score * s.weight, 0) / totalWeight)
      : 0;

    const spamSignal = signals.find(s => s.name === 'spam');
    const isSpam = (spamSignal?.score ?? 100) < SPAM_THRESHOLD;

    return {
      ...issue,
      totalScore,
      signals,
      intent: intentResult.intent,
      isSpam,
      spamReasons: isSpam ? this.collectSpamReasons(issue) : [],
    };
  }

  private collectSpamReasons(issue: IssueData): string[] {
    const reasons: string[] = [];
    const body = issue.body ?? '';
    if (body.length < 20) reasons.push('Empty or very short body');
    if (issue.title.length < 10) reasons.push('Very short title');
    if (issue.labels.length === 0 && issue.commentCount === 0) reasons.push('No labels and no comments');
    const aiMarkers = [/certainly!/i, /as an ai/i, /i apologize/i];
    if (aiMarkers.some(p => p.test(body))) reasons.push('AI-generated language detected');
    if (reasons.length === 0) reasons.push('Low quality content');
    return reasons;
  }

  private scoreStaleness(issue: IssueData): SignalScore {
    const days = Math.floor((Date.now() - new Date(issue.createdAt).getTime()) / (1000 * 60 * 60 * 24));
    let score: number;
    if (days < 7) score = 100;
    else if (days <= 30) score = 70;
    else if (days <= 90) score = 40;
    else score = 15;
    return { name: 'staleness', score, weight: 0.08, reason: `${days}d old` };
  }

  private scoreBodyQuality(issue: IssueData): SignalScore {
    const len = (issue.body ?? '').length;
    let score = len > 500 ? 90 : len >= 200 ? 70 : len >= 50 ? 50 : 20;
    if (/- \[[ x]\]/.test(issue.body ?? '')) score = Math.min(100, score + 10);
    return { name: 'body_quality', score, weight: 0.08, reason: `Body: ${len} chars` };
  }

  private scoreLabelPriority(issue: IssueData): SignalScore {
    const highLabels = ['high-priority', 'urgent', 'critical', 'p0', 'p1', 'security', 'bug'];
    const labels = issue.labels.map(l => l.toLowerCase());
    if (labels.some(l => highLabels.some(h => l.includes(h)))) {
      return { name: 'label_priority', score: 95, weight: 0.10, reason: `High priority: ${issue.labels.join(', ')}` };
    }
    return { name: 'label_priority', score: 50, weight: 0.07, reason: issue.labels.length > 0 ? `Labels: ${issue.labels.join(', ')}` : 'No labels' };
  }

  private scoreActivity(issue: IssueData): SignalScore {
    const score = issue.commentCount >= 5 ? 90 : issue.commentCount >= 2 ? 70 : issue.commentCount === 1 ? 50 : 30;
    return { name: 'activity', score, weight: 0.08, reason: `${issue.commentCount} comments` };
  }

  private scoreContributor(issue: IssueData): SignalScore {
    const trustMap: Record<string, number> = { OWNER: 100, MEMBER: 90, COLLABORATOR: 85, CONTRIBUTOR: 70, NONE: 30 };
    return { name: 'contributor', score: trustMap[issue.authorAssociation] ?? 50, weight: 0.08, reason: `${issue.author} (${issue.authorAssociation})` };
  }

  private scoreSpam(issue: IssueData): SignalScore {
    let spamScore = 0;
    const reasons: string[] = [];
    const body = issue.body ?? '';

    if (body.length < 20) { spamScore += 2; reasons.push('Empty/short body'); }
    if (issue.title.length < 10) { spamScore += 2; reasons.push('Short title'); }
    if (issue.labels.length === 0 && issue.commentCount === 0) { spamScore++; reasons.push('No labels/comments'); }
    const aiMarkers = [/certainly!/i, /as an ai/i, /i apologize/i];
    if (aiMarkers.some(p => p.test(body))) { spamScore++; reasons.push('AI language'); }

    return { name: 'spam', score: Math.max(0, 100 - spamScore * 20), weight: 0.10, reason: reasons.length > 0 ? reasons.join(', ') : 'No spam signals' };
  }

  private scoreMilestone(issue: IssueData): SignalScore {
    return { name: 'milestone', score: issue.milestone ? 90 : 40, weight: 0.07, reason: issue.milestone ? `Milestone: ${issue.milestone}` : 'No milestone' };
  }

  private scoreReactions(issue: IssueData): SignalScore {
    const score = issue.reactionCount >= 10 ? 95 : issue.reactionCount >= 5 ? 80 : issue.reactionCount >= 1 ? 60 : 30;
    return { name: 'reaction_score', score, weight: 0.10, reason: `${issue.reactionCount} reactions` };
  }

  private scoreLinkedPR(issue: IssueData): SignalScore {
    return { name: 'has_linked_pr', score: issue.linkedPRs.length > 0 ? 90 : 30, weight: 0.08, reason: issue.linkedPRs.length > 0 ? `Linked to PR(s): ${issue.linkedPRs.map(n => `#${n}`).join(', ')}` : 'No linked PR' };
  }

  private scoreAssignee(issue: IssueData): SignalScore {
    return { name: 'assignee_status', score: issue.assignees.length > 0 ? 80 : 30, weight: 0.07, reason: issue.assignees.length > 0 ? `Assigned: ${issue.assignees.join(', ')}` : 'Unassigned' };
  }

  private scoreReproducibility(issue: IssueData): SignalScore {
    const body = issue.body ?? '';
    let score = 40;
    if (/steps?\s*to\s*reproduce/i.test(body)) score += 20;
    if (/expected|actual/i.test(body)) score += 20;
    if (/```/.test(body)) score += 10;
    score = Math.min(100, score);
    return { name: 'reproducibility', score, weight: 0.07, reason: score >= 80 ? 'Has reproduction steps' : 'Missing reproduction info' };
  }

  private scoreIntent(result: IntentResult): SignalScore {
    const scores: Record<string, number> = { bugfix: 90, feature: 85, refactor: 60, dependency: 35, docs: 30, chore: 25 };
    return { name: 'intent', score: scores[result.intent] ?? 50, weight: 0.09, reason: `${result.intent} (${result.reason})` };
  }
}

const tests=[];const hooks=[];
const describe=(_name,fn)=>fn();const beforeEach=fn=>hooks.push(fn);const it=(name,fn)=>tests.push({name,fn});
const expect=v=>({toBe:x=>assert.equal(v,x),toBeGreaterThan:x=>assert.ok(v>x),toBeLessThanOrEqual:x=>assert.ok(v<=x),toBeGreaterThanOrEqual:x=>assert.ok(v>=x),toBeDefined:()=>assert.notEqual(v,undefined),toHaveLength:x=>assert.equal(v.length,x)});


describe('IssueScoringEngine', () => {
  let engine: IssueScoringEngine;

  beforeEach(() => {
    engine = new IssueScoringEngine();
  });

  it('scores a typical issue', async () => {
    const issue = createIssueData({ commentCount: 3, reactionCount: 5 });
    const scored = await engine.score(issue);
    expect(scored.totalScore).toBeGreaterThan(0);
    expect(scored.totalScore).toBeLessThanOrEqual(100);
    expect(scored.signals.length).toBe(12);
    expect(scored.isSpam).toBe(false);
  });

  it('detects spam issues (empty body, short title)', async () => {
    const issue = createIssueData({ body: '', title: 'hi', labels: [], commentCount: 0 });
    const scored = await engine.score(issue);
    expect(scored.isSpam).toBe(true);
    expect(scored.spamReasons.length).toBeGreaterThan(0);
  });

  it('scores high for popular issues with reactions', async () => {
    const popular = createIssueData({ reactionCount: 20, commentCount: 10, labels: ['bug', 'high-priority'] });
    const boring = createIssueData({ reactionCount: 0, commentCount: 0 });
    const scoredPop = await engine.score(popular);
    const scoredBor = await engine.score(boring);
    expect(scoredPop.totalScore).toBeGreaterThan(scoredBor.totalScore);
  });

  it('includes has_linked_pr signal', async () => {
    const withPR = createIssueData({ linkedPRs: [42] });
    const scored = await engine.score(withPR);
    const signal = scored.signals.find(s => s.name === 'has_linked_pr');
    expect(signal).toBeDefined();
    expect(signal!.score).toBe(90);
  });

  it('includes reproducibility signal', async () => {
    const withSteps = createIssueData({ body: 'Steps to reproduce:\n1. Open app\n2. Click login\n\nExpected: Login works\nActual: Crash' });
    const scored = await engine.score(withSteps);
    const signal = scored.signals.find(s => s.name === 'reproducibility');
    expect(signal).toBeDefined();
    expect(signal!.score).toBeGreaterThanOrEqual(80);
  });

  it('includes assignee signal', async () => {
    const assigned = createIssueData({ assignees: ['alice'] });
    const unassigned = createIssueData({ assignees: [] });
    const scoredA = await engine.score(assigned);
    const scoredU = await engine.score(unassigned);
    const sigA = scoredA.signals.find(s => s.name === 'assignee_status')!;
    const sigU = scoredU.signals.find(s => s.name === 'assignee_status')!;
    expect(sigA.score).toBeGreaterThan(sigU.score);
  });

  it('includes intent signal', async () => {
    const issue = createIssueData({ title: 'fix: crash on startup' });
    const scored = await engine.score(issue);
    expect(scored.intent).toBe('bugfix');
  });

  it('scores many issues in parallel', async () => {
    const issues = Array.from({ length: 10 }, (_, i) => createIssueData({ number: i + 1 }));
    const scored = await engine.scoreMany(issues);
    expect(scored).toHaveLength(10);
  });

  describe('individual signals', () => {
    it('staleness: fresh issues score higher', async () => {
      const fresh = createIssueData({ createdAt: new Date(Date.now() - 86400000).toISOString() });
      const stale = createIssueData({ createdAt: new Date(Date.now() - 86400000 * 120).toISOString() });
      const scoredF = await engine.score(fresh);
      const scoredS = await engine.score(stale);
      const sigF = scoredF.signals.find(s => s.name === 'staleness')!;
      const sigS = scoredS.signals.find(s => s.name === 'staleness')!;
      expect(sigF.score).toBeGreaterThan(sigS.score);
    });

    it('body_quality: long body scores higher', async () => {
      const good = createIssueData({ body: 'a'.repeat(600) });
      const bad = createIssueData({ body: 'hi' });
      const scoredG = await engine.score(good);
      const scoredB = await engine.score(bad);
      const sigG = scoredG.signals.find(s => s.name === 'body_quality')!;
      const sigB = scoredB.signals.find(s => s.name === 'body_quality')!;
      expect(sigG.score).toBeGreaterThan(sigB.score);
    });

    it('label_priority: high priority labels score higher', async () => {
      const high = createIssueData({ labels: ['critical'] });
      const none = createIssueData({ labels: [] });
      const scoredH = await engine.score(high);
      const scoredN = await engine.score(none);
      const sigH = scoredH.signals.find(s => s.name === 'label_priority')!;
      const sigN = scoredN.signals.find(s => s.name === 'label_priority')!;
      expect(sigH.score).toBeGreaterThan(sigN.score);
    });

    it('milestone: issues with milestone score higher', async () => {
      const withMs = createIssueData({ milestone: 'v1.0' });
      const without = createIssueData({ milestone: undefined });
      const scoredW = await engine.score(withMs);
      const scoredWo = await engine.score(without);
      const sigW = scoredW.signals.find(s => s.name === 'milestone')!;
      const sigWo = scoredWo.signals.find(s => s.name === 'milestone')!;
      expect(sigW.score).toBeGreaterThan(sigWo.score);
    });

    it('contributor: OWNER scores higher than NONE', async () => {
      const owner = createIssueData({ authorAssociation: 'OWNER' });
      const anon = createIssueData({ authorAssociation: 'NONE' });
      const scoredO = await engine.score(owner);
      const scoredA = await engine.score(anon);
      const sigO = scoredO.signals.find(s => s.name === 'contributor')!;
      const sigA = scoredA.signals.find(s => s.name === 'contributor')!;
      expect(sigO.score).toBeGreaterThan(sigA.score);
    });

    it('reaction_score: high reactions score higher', async () => {
      const popular = createIssueData({ reactionCount: 15 });
      const quiet = createIssueData({ reactionCount: 0 });
      const scoredP = await engine.score(popular);
      const scoredQ = await engine.score(quiet);
      const sigP = scoredP.signals.find(s => s.name === 'reaction_score')!;
      const sigQ = scoredQ.signals.find(s => s.name === 'reaction_score')!;
      expect(sigP.score).toBeGreaterThan(sigQ.score);
    });
  });
});
for(const t of tests){for(const h of hooks)await h();await t.fn();}
console.log(JSON.stringify({originalCallbacks:tests.length,passed:tests.length,names:tests.map(t=>t.name),scope:'original IssueScoringEngine, IntentClassifier and fixtures; import/logger/test registration/assert adapters; original controller; no provider or GitHub'}));
