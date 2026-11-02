import assert from "node:assert/strict";
/**
 * GitHub App Configuration
 *
 * Detects whether Treliq is running in PAT mode (personal access token)
 * or GitHub App mode (installation-based authentication).
 *
 * PAT mode: Set GITHUB_TOKEN env var (existing behavior)
 * App mode: Set GITHUB_APP_ID + GITHUB_PRIVATE_KEY env vars
 */

import { readFileSync } from 'fs';

export type AuthMode = 'pat' | 'app';

export interface GitHubAppConfig {
  appId: number;
  privateKey: string;
  webhookSecret: string;
  clientId?: string;
  clientSecret?: string;
}

/**
 * Detect authentication mode based on environment variables
 */
export function getAuthMode(): AuthMode {
  return process.env.GITHUB_APP_ID ? 'app' : 'pat';
}

/**
 * Load GitHub App configuration from environment
 * Returns null if not in app mode
 */
export function getAppConfig(): GitHubAppConfig | null {
  if (getAuthMode() !== 'app') return null;

  const appId = parseInt(process.env.GITHUB_APP_ID!, 10);
  if (isNaN(appId)) {
    throw new Error('GITHUB_APP_ID must be a valid number');
  }

  // Private key can be provided directly or via file path
  let privateKey: string;
  if (process.env.GITHUB_PRIVATE_KEY) {
    privateKey = process.env.GITHUB_PRIVATE_KEY.replace(/\\n/g, '\n');
  } else if (process.env.GITHUB_PRIVATE_KEY_PATH) {
    try {
      privateKey = readFileSync(process.env.GITHUB_PRIVATE_KEY_PATH, 'utf-8');
    } catch (err: any) {
      throw new Error(`Failed to read private key from ${process.env.GITHUB_PRIVATE_KEY_PATH}: ${err.message}`);
    }
  } else {
    throw new Error('GITHUB_PRIVATE_KEY or GITHUB_PRIVATE_KEY_PATH is required in app mode');
  }

  const webhookSecret = process.env.GITHUB_WEBHOOK_SECRET || '';

  return {
    appId,
    privateKey,
    webhookSecret,
    clientId: process.env.GITHUB_CLIENT_ID,
    clientSecret: process.env.GITHUB_CLIENT_SECRET,
  };
}

/**
 * Validate that all required configuration is present for the current auth mode
 */
export function validateConfig(): { mode: AuthMode; valid: boolean; errors: string[] } {
  const mode = getAuthMode();
  const errors: string[] = [];

  if (mode === 'pat') {
    if (!process.env.GITHUB_TOKEN) {
      errors.push('GITHUB_TOKEN environment variable is required');
    }
  } else {
    try {
      getAppConfig();
    } catch (err: any) {
      errors.push(err.message);
    }
  }

  return { mode, valid: errors.length === 0, errors };
}

process.env={};
const tests=[],before=[],after=[];
const describe=(_name,fn)=>fn(),beforeEach=fn=>before.push(fn),afterEach=fn=>after.push(fn),it=(name,fn)=>tests.push({name,fn});
const expect=v=>({toBe:x=>assert.equal(v,x),toEqual:x=>assert.deepEqual(v,x),toBeNull:()=>assert.equal(v,null),not:{toBeNull:()=>assert.notEqual(v,null)},toContain:x=>assert.ok(v.includes(x)),toBeGreaterThan:x=>assert.ok(v>x),toThrow:x=>assert.throws(v,e=>e.message.includes(x))});

/**
 * Unit tests for app-config
 */


describe('app-config', () => {
  let originalEnv: NodeJS.ProcessEnv;

  beforeEach(() => {
    originalEnv = { ...process.env };
  });

  afterEach(() => {
    process.env = originalEnv;
  });

  describe('getAuthMode', () => {
    it('returns pat when no GITHUB_APP_ID', () => {
      delete process.env.GITHUB_APP_ID;
      expect(getAuthMode()).toBe('pat');
    });

    it('returns app when GITHUB_APP_ID is set', () => {
      process.env.GITHUB_APP_ID = '12345';
      expect(getAuthMode()).toBe('app');
    });
  });

  describe('getAppConfig', () => {
    it('returns null in PAT mode', () => {
      delete process.env.GITHUB_APP_ID;
      expect(getAppConfig()).toBeNull();
    });

    it('returns config in app mode with valid settings', () => {
      process.env.GITHUB_APP_ID = '12345';
      process.env.GITHUB_PRIVATE_KEY = '-----BEGIN RSA PRIVATE KEY-----\ntest\n-----END RSA PRIVATE KEY-----';
      process.env.GITHUB_WEBHOOK_SECRET = 'secret123';
      process.env.GITHUB_CLIENT_ID = 'client123';
      process.env.GITHUB_CLIENT_SECRET = 'secret456';

      const config = getAppConfig();
      expect(config).not.toBeNull();
      expect(config?.appId).toBe(12345);
      expect(config?.privateKey).toContain('BEGIN RSA PRIVATE KEY');
      expect(config?.webhookSecret).toBe('secret123');
      expect(config?.clientId).toBe('client123');
      expect(config?.clientSecret).toBe('secret456');
    });

    it('throws when GITHUB_APP_ID is not a number', () => {
      process.env.GITHUB_APP_ID = 'not-a-number';
      process.env.GITHUB_PRIVATE_KEY = 'key';

      expect(() => getAppConfig()).toThrow('GITHUB_APP_ID must be a valid number');
    });

    it('handles escaped newlines in private key', () => {
      process.env.GITHUB_APP_ID = '12345';
      process.env.GITHUB_PRIVATE_KEY = 'line1\\nline2\\nline3';

      const config = getAppConfig();
      expect(config?.privateKey).toBe('line1\nline2\nline3');
    });

    it('defaults webhook secret to empty string', () => {
      process.env.GITHUB_APP_ID = '12345';
      process.env.GITHUB_PRIVATE_KEY = 'test-key';
      delete process.env.GITHUB_WEBHOOK_SECRET;

      const config = getAppConfig();
      expect(config?.webhookSecret).toBe('');
    });

    it('includes optional client credentials when provided', () => {
      process.env.GITHUB_APP_ID = '12345';
      process.env.GITHUB_PRIVATE_KEY = 'test-key';
      process.env.GITHUB_CLIENT_ID = 'client123';
      process.env.GITHUB_CLIENT_SECRET = 'secret456';

      const config = getAppConfig();
      expect(config?.clientId).toBe('client123');
      expect(config?.clientSecret).toBe('secret456');
    });
  });

  describe('validateConfig', () => {
    it('returns errors in PAT mode when GITHUB_TOKEN is missing', () => {
      delete process.env.GITHUB_APP_ID;
      delete process.env.GITHUB_TOKEN;

      const result = validateConfig();
      expect(result.mode).toBe('pat');
      expect(result.valid).toBe(false);
      expect(result.errors).toContain('GITHUB_TOKEN environment variable is required');
    });

    it('returns no errors in valid PAT mode', () => {
      delete process.env.GITHUB_APP_ID;
      process.env.GITHUB_TOKEN = 'ghp_test123';

      const result = validateConfig();
      expect(result.mode).toBe('pat');
      expect(result.valid).toBe(true);
      expect(result.errors).toEqual([]);
    });

    it('returns errors in app mode with invalid config', () => {
      process.env.GITHUB_APP_ID = '12345';
      delete process.env.GITHUB_PRIVATE_KEY;
      delete process.env.GITHUB_PRIVATE_KEY_PATH;

      const result = validateConfig();
      expect(result.mode).toBe('app');
      expect(result.valid).toBe(false);
      expect(result.errors.length).toBeGreaterThan(0);
    });

    it('returns no errors in valid app mode', () => {
      process.env.GITHUB_APP_ID = '12345';
      process.env.GITHUB_PRIVATE_KEY = '-----BEGIN RSA PRIVATE KEY-----\ntest\n-----END RSA PRIVATE KEY-----';

      const result = validateConfig();
      expect(result.mode).toBe('app');
      expect(result.valid).toBe(true);
      expect(result.errors).toEqual([]);
    });
  });
});
for(const t of tests){for(const h of before)h();try{await t.fn()}finally{for(const h of after)h()}}
console.log(JSON.stringify({originalCallbacks:tests.length,passed:tests.length,scope:'original config and test bodies; registration/assertion adapter; process environment replaced with empty object before tests; synthetic values only; no file-key path exercised'}));
