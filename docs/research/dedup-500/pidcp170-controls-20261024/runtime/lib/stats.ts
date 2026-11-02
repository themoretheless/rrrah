// Explicit adapter: collect lifetime counters without accessing user storage.
export const bumps: unknown[] = [];
export function bumpLifetime(value: unknown): void {bumps.push(value);}
