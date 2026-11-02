import assert from "node:assert/strict";

const log={warn(){}};

const NON_RETRYABLE = new Set([400, 401, 403, 404, 422]);

export interface RetryOptions {
  maxRetries?: number;
  baseDelay?: number;
  maxDelay?: number;
  onThrottle?: () => void;
}

export class RetryableProvider implements LLMProvider {
  get name() { return this.inner.name; }
  get supportsEmbeddings() { return this.inner.supportsEmbeddings; }

  private inner: LLMProvider;
  private maxRetries: number;
  private baseDelay: number;
  private maxDelay: number;
  private onThrottle?: () => void;

  constructor(inner: LLMProvider, opts: RetryOptions = {}) {
    this.inner = inner;
    this.maxRetries = opts.maxRetries ?? 3;
    this.baseDelay = opts.baseDelay ?? 1000;
    this.maxDelay = opts.maxDelay ?? 30000;
    this.onThrottle = opts.onThrottle;
  }

  async generateText(prompt: string, options?: { temperature?: number; maxTokens?: number }): Promise<string> {
    return this.withRetry(() => this.inner.generateText(prompt, options), 'generateText');
  }

  async generateEmbedding(text: string): Promise<number[]> {
    return this.withRetry(() => this.inner.generateEmbedding(text), 'generateEmbedding');
  }

  get generateEmbeddingBatch(): ((texts: string[]) => Promise<number[][]>) | undefined {
    const inner = this.inner as any;
    if (typeof inner.generateEmbeddingBatch !== 'function') return undefined;
    return (texts: string[]) => this.withRetry(() => inner.generateEmbeddingBatch(texts), 'generateEmbeddingBatch');
  }

  private async withRetry<T>(fn: () => Promise<T>, label: string): Promise<T> {
    for (let attempt = 0; attempt <= this.maxRetries; attempt++) {
      try {
        return await fn();
      } catch (err: any) {
        const status = err.status ?? this.extractStatus(err.message);

        if (NON_RETRYABLE.has(status)) throw err;
        if (attempt === this.maxRetries) throw err;

        let delay: number;
        if (status === 429) {
          this.onThrottle?.();
          const retryAfter = err.retryAfter;
          if (typeof retryAfter === 'number') {
            delay = retryAfter * 1000;
          } else {
            delay = this.baseDelay * Math.pow(2, attempt);
          }
          log.warn({ attempt, delay, label }, 'Rate limited (429), backing off');
        } else {
          delay = this.baseDelay * Math.pow(2, attempt);
          log.warn({ attempt, delay, label, err: err.message }, 'Retrying after error');
        }

        delay = Math.min(delay, this.maxDelay);
        // Add jitter (50-100% of delay) to prevent thundering herd
        delay = delay * (0.5 + Math.random() * 0.5);
        await new Promise(r => setTimeout(r, delay));
      }
    }
    throw new Error('Unreachable');
  }

  private extractStatus(message: string): number | undefined {
    const match = message?.match?.(/\b(4\d{2}|5\d{2})\b/);
    return match ? parseInt(match[1]) : undefined;
  }
}
const delays=[];
globalThis.setTimeout=(fn,ms)=>{delays.push(ms);fn();return 0};
Math.random=()=>0;
let calls=0;
const inner={name:'synthetic',async generateText(){calls++;if(calls===1)throw Object.assign(new Error('429'),{status:429,retryAfter:10});return 'ok'},async generateEmbedding(){return [1]}};
assert.equal(await new RetryableProvider(inner,{maxRetries:1}).generateText('synthetic'),'ok');
assert.deepEqual(delays,[5000]);
console.log(JSON.stringify({controls:1,passed:1,requestedRetryAfterMs:10000,scheduledDelayMs:5000,providerCalls:calls,scope:'original retry wrapper body, inert logger, synthetic provider, intercepted timers and fixed random; no network or real waiting'}));
