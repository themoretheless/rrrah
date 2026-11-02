import assert from "node:assert/strict";
import { ConcurrencyController } from "./concurrency-original.mts";
/**
 * DiffAnalyzer — Fetches PR diffs and analyzes code changes via LLM
 */


const log = {info(){},warn(){}};

const MAX_DIFF_LENGTH = 10000;
const VALID_RISKS = ['low', 'medium', 'high', 'critical'] as const;
const VALID_CHANGE_TYPES = ['additive', 'modifying', 'removing', 'mixed'] as const;

export class DiffAnalyzer {
  private octokit: Octokit;
  private owner: string;
  private repo: string;
  private provider?: LLMProvider;
  private concurrency: ConcurrencyController;

  constructor(octokit: Octokit, owner: string, repo: string, provider?: LLMProvider, maxConcurrent = 3) {
    this.octokit = octokit;
    this.owner = owner;
    this.repo = repo;
    this.provider = provider;
    this.concurrency = new ConcurrencyController(maxConcurrent);
  }

  async analyzeMany(prNumbers: number[], cc?: ConcurrencyController): Promise<DiffAnalysis[]> {
    if (!this.provider || prNumbers.length === 0) return [];

    const controller = cc ?? this.concurrency;
    log.info({ count: prNumbers.length }, 'Analyzing PR diffs');

    const results = await Promise.allSettled(
      prNumbers.map(num => controller.execute(() => this.analyzeOne(num)))
    );

    const analyses: DiffAnalysis[] = [];
    for (const result of results) {
      if (result.status === 'fulfilled' && result.value) {
        analyses.push(result.value);
      }
    }

    log.info({ analyzed: analyses.length, total: prNumbers.length }, 'Diff analysis complete');
    return analyses;
  }

  private async analyzeOne(prNumber: number): Promise<DiffAnalysis | null> {
    try {
      const diff = await this.fetchDiff(prNumber);
      if (!diff) return null;

      const truncated = diff.slice(0, MAX_DIFF_LENGTH);
      const prompt = `Analyze this PR diff. Return JSON:
{"codeQuality": <0-100>, "riskAssessment": "<low|medium|high|critical>",
 "changeType": "<additive|modifying|removing|mixed>",
 "affectedAreas": ["<area1>", ...], "summary": "<brief>"}

Diff:
${truncated}`;

      const text = await this.provider!.generateText(prompt, { temperature: 0.1, maxTokens: 200 });
      return this.parseResponse(prNumber, text);
    } catch (err) {
      log.warn({ prNumber, err }, 'Diff analysis failed');
      return null;
    }
  }

  private async fetchDiff(prNumber: number): Promise<string | null> {
    try {
      const response = await this.octokit.request('GET /repos/{owner}/{repo}/pulls/{pull_number}', {
        owner: this.owner,
        repo: this.repo,
        pull_number: prNumber,
        headers: { accept: 'application/vnd.github.diff' },
      });
      return typeof response.data === 'string' ? response.data : String(response.data);
    } catch (err) {
      log.warn({ prNumber, err }, 'Failed to fetch diff');
      return null;
    }
  }

  private parseResponse(prNumber: number, text: string): DiffAnalysis | null {
    const match = text.match(/\{[\s\S]*\}/);
    if (!match) return null;

    try {
      const parsed = JSON.parse(match[0]);
      return {
        prNumber,
        codeQuality: Math.max(0, Math.min(100, Number(parsed.codeQuality) || 50)),
        riskAssessment: VALID_RISKS.includes(parsed.riskAssessment) ? parsed.riskAssessment : 'medium',
        changeType: VALID_CHANGE_TYPES.includes(parsed.changeType) ? parsed.changeType : 'mixed',
        affectedAreas: Array.isArray(parsed.affectedAreas) ? parsed.affectedAreas.map(String) : [],
        summary: String(parsed.summary ?? ''),
      };
    } catch {
      return null;
    }
  }
}
const prompts=[];
let calls=0;
const provider={async generateText(prompt){prompts.push(prompt);return JSON.stringify({codeQuality:0,riskAssessment:'low',changeType:'additive'});}};
const sdk={async request(){calls++;return {data:'x'.repeat(10050)}}};
const analyzer=new DiffAnalyzer(sdk,'synthetic','owned',provider);
const rows=await analyzer.analyzeMany([1]);
assert.equal(rows[0].codeQuality,50);
assert.equal(prompts[0].split('Diff:\n')[1].length,10000);
const failing=new DiffAnalyzer(sdk,'synthetic','owned',{async generateText(){throw Error('synthetic failure')}});
assert.deepEqual(await failing.analyzeMany([2]),[]);
assert.equal(calls,2);
console.log(JSON.stringify({controls:3,passed:3,observations:['explicit zero quality becomes 50','10050-character diff silently truncated to 10000 in model prompt','provider failure returns empty analyses without retry'],scope:'unchanged DiffAnalyzer class body; inert logger, actual original controller, synthetic SDK/provider; no GitHub or model calls'}));
