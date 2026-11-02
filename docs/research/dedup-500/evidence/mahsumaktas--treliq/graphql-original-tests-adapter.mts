import assert from "node:assert/strict";
/**
 * GitHub GraphQL queries and response mappers for Treliq.
 *
 * Replaces multiple REST API calls with single GraphQL queries,
 * reducing API call count by ~80% per scan.
 */


// ─── Issue Reference Patterns (shared with scanner) ───

const ISSUE_REF_PATTERN = /(?:close[sd]?|fix(?:e[sd])?|resolve[sd]?|related\s+to|addresses|refs?)\s+#(\d+)/gi;
const LOOSE_ISSUE_REF = /#(\d+)/g;

function extractIssueNumbers(text: string): number[] {
  const issueNumbers: number[] = [];
  for (const match of text.matchAll(ISSUE_REF_PATTERN)) {
    issueNumbers.push(parseInt(match[1], 10));
  }
  if (issueNumbers.length === 0) {
    for (const match of text.matchAll(LOOSE_ISSUE_REF)) {
      const num = parseInt(match[1], 10);
      if (num > 0 && num < 100000) issueNumbers.push(num);
    }
  }
  return issueNumbers;
}

const TEST_PATTERNS = [/test\//, /spec\//, /__test__/, /__tests__/, /\.test\./, /\.spec\./, /_test\.go$/, /Test\.java$/];

// ─── GraphQL Queries ───

/**
 * Lightweight PR list query — for cache comparison only.
 * Fetches number, updatedAt, headSha for each open PR.
 */
export const PR_LIST_QUERY = `
  query($owner: String!, $repo: String!, $first: Int!, $after: String) {
    repository(owner: $owner, name: $repo) {
      pullRequests(first: $first, after: $after, states: OPEN, orderBy: {field: UPDATED_AT, direction: DESC}) {
        pageInfo { hasNextPage endCursor }
        nodes {
          number
          updatedAt
          headRefOid
        }
      }
    }
  }
`;

/**
 * Full PR details query — fetches all fields needed for scoring in one call.
 * Replaces: pulls.get + pulls.listFiles + checks.listForRef + pulls.listReviews
 */
export const PR_DETAILS_QUERY = `
  query($owner: String!, $repo: String!, $first: Int!, $after: String) {
    repository(owner: $owner, name: $repo) {
      pullRequests(first: $first, after: $after, states: OPEN, orderBy: {field: UPDATED_AT, direction: DESC}) {
        pageInfo { hasNextPage endCursor }
        nodes {
          number
          title
          body
          isDraft
          createdAt
          updatedAt
          author { login }
          authorAssociation
          headRefName
          baseRefName
          headRefOid
          additions
          deletions
          changedFiles
          milestone { title }
          labels(first: 50) { nodes { name } }
          reviewRequests(first: 20) {
            nodes {
              requestedReviewer {
                ... on User { login }
                ... on Team { name }
              }
            }
          }
          reviews(last: 50) {
            nodes { state }
            totalCount
          }
          comments { totalCount }
          mergeable
          files(first: 100) { nodes { path } }
          commits(last: 1) {
            nodes {
              commit {
                statusCheckRollup {
                  state
                }
              }
            }
            totalCount
          }
        }
      }
    }
  }
`;

/**
 * Single PR detail query — for webhook scoring and `score` command.
 */
export const SINGLE_PR_QUERY = `
  query($owner: String!, $repo: String!, $number: Int!) {
    repository(owner: $owner, name: $repo) {
      pullRequest(number: $number) {
        number
        title
        body
        isDraft
        createdAt
        updatedAt
        author { login }
        authorAssociation
        headRefName
        baseRefName
        headRefOid
        additions
        deletions
        changedFiles
        milestone { title }
        labels(first: 50) { nodes { name } }
        reviewRequests(first: 20) {
          nodes {
            requestedReviewer {
              ... on User { login }
              ... on Team { name }
            }
          }
        }
        reviews(last: 50) {
          nodes { state }
          totalCount
        }
        comments { totalCount }
        mergeable
        files(first: 100) { nodes { path } }
        commits(last: 1) {
          nodes {
            commit {
              statusCheckRollup {
                state
              }
            }
          }
          totalCount
        }
      }
    }
  }
`;

// ─── Issue GraphQL Queries ───

/**
 * Lightweight issue list query — for cache comparison only.
 * Fetches number, updatedAt for each open issue.
 */
export const ISSUE_LIST_QUERY = `
  query($owner: String!, $repo: String!, $first: Int!, $after: String) {
    repository(owner: $owner, name: $repo) {
      issues(first: $first, after: $after, states: OPEN, orderBy: {field: UPDATED_AT, direction: DESC}) {
        pageInfo { hasNextPage endCursor }
        nodes {
          number
          updatedAt
        }
      }
    }
  }
`;

/**
 * Full issue details query — fetches all fields needed for scoring in one call.
 */
export const ISSUE_DETAILS_QUERY = `
  query($owner: String!, $repo: String!, $first: Int!, $after: String) {
    repository(owner: $owner, name: $repo) {
      issues(first: $first, after: $after, states: OPEN, orderBy: {field: UPDATED_AT, direction: DESC}) {
        pageInfo { hasNextPage endCursor }
        nodes {
          number
          title
          body
          createdAt
          updatedAt
          author { login }
          authorAssociation
          labels(first: 50) { nodes { name } }
          milestone { title }
          comments { totalCount }
          reactions { totalCount }
          state
          stateReason
          locked
          assignees(first: 20) { nodes { login } }
        }
      }
    }
  }
`;

// ─── GraphQL Mergeable Enum → PRData Mapping ───

const GRAPHQL_MERGEABLE_MAP: Record<string, PRData['mergeable']> = {
  MERGEABLE: 'mergeable',
  CONFLICTING: 'conflicting',
  UNKNOWN: 'unknown',
};

// ─── CI Status Mapping ───

const GRAPHQL_CI_MAP: Record<string, PRData['ciStatus']> = {
  SUCCESS: 'success',
  FAILURE: 'failure',
  ERROR: 'failure',
  EXPECTED: 'success',
  PENDING: 'pending',
};

// ─── Response Mapper ───

/**
 * Maps a GraphQL PR node to the PRData interface.
 * Handles all field transformations and client-side computed fields.
 */
export function mapGraphQLToPRData(node: any, owner: string, repo: string): { pr: PRData; headSha: string } {
  // Changed files list
  const changedFiles: string[] = (node.files?.nodes ?? []).map((f: any) => f.path);

  // CI status from statusCheckRollup
  let ciStatus: PRData['ciStatus'] = 'unknown';
  const lastCommit = node.commits?.nodes?.[0];
  if (lastCommit?.commit?.statusCheckRollup?.state) {
    ciStatus = GRAPHQL_CI_MAP[lastCommit.commit.statusCheckRollup.state] ?? 'unknown';
  }

  // Review state
  let reviewState: PRData['reviewState'] = 'none';
  const reviews = node.reviews?.nodes ?? [];
  const reviewStates = reviews.map((r: any) => r.state);
  if (reviewStates.includes('APPROVED')) reviewState = 'approved';
  else if (reviewStates.includes('CHANGES_REQUESTED')) reviewState = 'changes_requested';
  else if (reviewStates.some((s: string) => s === 'COMMENTED')) reviewState = 'commented';

  // Labels
  const labels: string[] = (node.labels?.nodes ?? []).map((l: any) => l.name);

  // Requested reviewers (GraphQL field: reviewRequests → requestedReviewer)
  const requestedReviewers: string[] = (node.reviewRequests?.nodes ?? [])
    .map((r: any) => r.requestedReviewer?.login ?? r.requestedReviewer?.name ?? '')
    .filter(Boolean);

  // Test detection
  const testFilesChanged = changedFiles.filter(f => TEST_PATTERNS.some(p => p.test(f)));
  const hasTests = testFilesChanged.length > 0;

  // Age
  const ageInDays = Math.floor((Date.now() - new Date(node.createdAt).getTime()) / (1000 * 60 * 60 * 24));

  // Issue references
  const text = `${node.title} ${node.body ?? ''}`;
  const issueNumbers = extractIssueNumbers(text);

  // Mergeable
  const mergeable = GRAPHQL_MERGEABLE_MAP[node.mergeable] ?? 'unknown';

  const headSha = node.headRefOid ?? '';

  const pr: PRData = {
    number: node.number,
    title: node.title,
    body: node.body ?? '',
    author: node.author?.login ?? 'unknown',
    authorAssociation: node.authorAssociation ?? 'NONE',
    createdAt: node.createdAt,
    updatedAt: node.updatedAt,
    headRef: node.headRefName ?? '',
    baseRef: node.baseRefName ?? '',
    filesChanged: node.changedFiles ?? 0,
    additions: node.additions ?? 0,
    deletions: node.deletions ?? 0,
    commits: node.commits?.totalCount ?? 0,
    labels,
    ciStatus,
    hasIssueRef: issueNumbers.length > 0,
    issueNumbers,
    changedFiles,
    diffUrl: `https://github.com/${owner}/${repo}/pull/${node.number}.diff`,
    hasTests,
    testFilesChanged,
    ageInDays,
    mergeable,
    reviewState,
    reviewCount: node.reviews?.totalCount ?? 0,
    commentCount: node.comments?.totalCount ?? 0,
    isDraft: node.isDraft ?? false,
    milestone: node.milestone?.title ?? undefined,
    requestedReviewers,
    codeowners: [], // Filled separately via CODEOWNERS file
  };

  return { pr, headSha };
}

/**
 * Maps a GraphQL Issue node to the IssueData interface.
 */
export function mapGraphQLToIssueData(node: any): IssueData {
  return {
    number: node.number,
    title: node.title,
    body: node.body ?? '',
    author: node.author?.login ?? 'unknown',
    authorAssociation: node.authorAssociation ?? 'NONE',
    createdAt: node.createdAt,
    updatedAt: node.updatedAt,
    labels: (node.labels?.nodes ?? []).map((l: any) => l.name),
    milestone: node.milestone?.title ?? undefined,
    commentCount: node.comments?.totalCount ?? 0,
    reactionCount: node.reactions?.totalCount ?? 0,
    state: node.state?.toLowerCase() as 'open' | 'closed',
    stateReason: node.stateReason ?? null,
    isLocked: node.locked ?? false,
    assignees: (node.assignees?.nodes ?? []).map((a: any) => a.login),
    linkedPRs: [],
  };
}
const results=[];
const describe=(_name,fn)=>fn();
const it=(name,fn)=>{fn();results.push({name,passed:true})};
const expect=(v)=>({toBe:x=>assert.equal(v,x),toEqual:x=>assert.deepEqual(v,x),toContain:x=>assert.ok(v.includes(x)),toBeUndefined:()=>assert.equal(v,undefined),toBeGreaterThanOrEqual:x=>assert.ok(v>=x)});


describe('mapGraphQLToPRData', () => {
  it('maps a full GraphQL node to PRData', () => {
    const node = {
      number: 42,
      title: 'feat: improve parser',
      body: 'Fixes #101 with better error handling',
      isDraft: false,
      createdAt: '2026-02-10T10:00:00.000Z',
      updatedAt: '2026-02-11T10:00:00.000Z',
      author: { login: 'alice' },
      authorAssociation: 'MEMBER',
      headRefName: 'feat/parser',
      baseRefName: 'main',
      headRefOid: 'abc123',
      additions: 120,
      deletions: 30,
      changedFiles: 2,
      milestone: { title: 'v1.1' },
      labels: { nodes: [{ name: 'high-priority' }] },
      reviewRequests: {
        nodes: [
          { requestedReviewer: { login: 'bob' } },
          { requestedReviewer: { name: 'core-team' } },
        ],
      },
      reviews: {
        nodes: [{ state: 'APPROVED' }],
        totalCount: 1,
      },
      comments: { totalCount: 3 },
      mergeable: 'MERGEABLE',
      files: { nodes: [{ path: 'src/parser.ts' }, { path: 'test/parser.test.ts' }] },
      commits: {
        nodes: [{ commit: { statusCheckRollup: { state: 'SUCCESS' } } }],
        totalCount: 4,
      },
    };

    const { pr, headSha } = mapGraphQLToPRData(node, 'owner', 'repo');

    expect(headSha).toBe('abc123');
    expect(pr.number).toBe(42);
    expect(pr.author).toBe('alice');
    expect(pr.ciStatus).toBe('success');
    expect(pr.mergeable).toBe('mergeable');
    expect(pr.reviewState).toBe('approved');
    expect(pr.requestedReviewers).toEqual(['bob', 'core-team']);
    expect(pr.hasIssueRef).toBe(true);
    expect(pr.issueNumbers).toEqual([101]);
    expect(pr.hasTests).toBe(true);
    expect(pr.testFilesChanged).toEqual(['test/parser.test.ts']);
    expect(pr.diffUrl).toBe('https://github.com/owner/repo/pull/42.diff');
  });

  it('maps fallbacks for missing optional fields', () => {
    const node = {
      number: 7,
      title: 'docs: update README #55',
      body: 'see #55',
      createdAt: '2026-02-12T10:00:00.000Z',
      updatedAt: '2026-02-13T10:00:00.000Z',
      author: null,
      authorAssociation: null,
      headRefName: null,
      baseRefName: null,
      headRefOid: null,
      additions: null,
      deletions: null,
      changedFiles: null,
      milestone: null,
      labels: null,
      reviewRequests: null,
      reviews: {
        nodes: [{ state: 'COMMENTED' }],
        totalCount: 1,
      },
      comments: { totalCount: 0 },
      mergeable: 'UNKNOWN',
      files: { nodes: [{ path: 'README.md' }] },
      commits: {
        nodes: [{ commit: { statusCheckRollup: { state: 'PENDING' } } }],
        totalCount: 1,
      },
      isDraft: undefined,
    };

    const { pr, headSha } = mapGraphQLToPRData(node, 'acme', 'app');

    expect(headSha).toBe('');
    expect(pr.author).toBe('unknown');
    expect(pr.authorAssociation).toBe('NONE');
    expect(pr.headRef).toBe('');
    expect(pr.baseRef).toBe('');
    expect(pr.ciStatus).toBe('pending');
    expect(pr.mergeable).toBe('unknown');
    expect(pr.reviewState).toBe('commented');
    expect(pr.requestedReviewers).toEqual([]);
    expect(pr.hasTests).toBe(false);
    expect(pr.hasIssueRef).toBe(true);
    expect(pr.issueNumbers).toEqual([55, 55]);
    expect(pr.ageInDays).toBeGreaterThanOrEqual(0);
  });

  it('maps CHANGES_REQUESTED to changes_requested', () => {
    const node = {
      number: 9,
      title: 'refactor: cleanup',
      body: '',
      createdAt: '2026-02-12T10:00:00.000Z',
      updatedAt: '2026-02-13T10:00:00.000Z',
      author: { login: 'dev' },
      authorAssociation: 'CONTRIBUTOR',
      headRefName: 'refactor',
      baseRefName: 'main',
      headRefOid: 'sha9',
      additions: 1,
      deletions: 1,
      changedFiles: 1,
      reviews: {
        nodes: [{ state: 'CHANGES_REQUESTED' }],
        totalCount: 1,
      },
      comments: { totalCount: 0 },
      mergeable: 'CONFLICTING',
      files: { nodes: [{ path: 'src/a.ts' }] },
      commits: {
        nodes: [{ commit: { statusCheckRollup: { state: 'ERROR' } } }],
        totalCount: 1,
      },
      labels: { nodes: [] },
      reviewRequests: { nodes: [] },
      isDraft: false,
    };

    const { pr } = mapGraphQLToPRData(node, 'owner', 'repo');

    expect(pr.reviewState).toBe('changes_requested');
    expect(pr.ciStatus).toBe('failure');
    expect(pr.mergeable).toBe('conflicting');
  });
});

describe('Issue GraphQL', () => {
  it('exports ISSUE_DETAILS_QUERY as a string', () => {
    expect(typeof ISSUE_DETAILS_QUERY).toBe('string');
    expect(ISSUE_DETAILS_QUERY).toContain('issues');
  });

  it('exports ISSUE_LIST_QUERY as a string', () => {
    expect(typeof ISSUE_LIST_QUERY).toBe('string');
    expect(ISSUE_LIST_QUERY).toContain('issues');
  });

  it('maps GraphQL issue node to IssueData', () => {
    const node = {
      number: 42,
      title: 'Bug: login fails',
      body: 'Steps to reproduce...',
      createdAt: '2026-01-01T00:00:00Z',
      updatedAt: '2026-01-02T00:00:00Z',
      author: { login: 'alice' },
      authorAssociation: 'MEMBER',
      labels: { nodes: [{ name: 'bug' }] },
      milestone: { title: 'v1.0' },
      comments: { totalCount: 3 },
      reactions: { totalCount: 5 },
      state: 'OPEN',
      stateReason: null,
      locked: false,
      assignees: { nodes: [{ login: 'bob' }] },
    };

    const issue = mapGraphQLToIssueData(node);
    expect(issue.number).toBe(42);
    expect(issue.title).toBe('Bug: login fails');
    expect(issue.author).toBe('alice');
    expect(issue.labels).toEqual(['bug']);
    expect(issue.milestone).toBe('v1.0');
    expect(issue.commentCount).toBe(3);
    expect(issue.reactionCount).toBe(5);
    expect(issue.state).toBe('open');
    expect(issue.assignees).toEqual(['bob']);
    expect(issue.linkedPRs).toEqual([]);
  });

  it('handles null fields gracefully', () => {
    const node = {
      number: 1,
      title: 'Test',
      body: null,
      createdAt: '2026-01-01T00:00:00Z',
      updatedAt: '2026-01-01T00:00:00Z',
      author: null,
      authorAssociation: 'NONE',
      labels: { nodes: [] },
      milestone: null,
      comments: { totalCount: 0 },
      reactions: { totalCount: 0 },
      state: 'OPEN',
      stateReason: null,
      locked: false,
      assignees: { nodes: [] },
    };

    const issue = mapGraphQLToIssueData(node);
    expect(issue.body).toBe('');
    expect(issue.author).toBe('unknown');
    expect(issue.milestone).toBeUndefined();
  });

  it('maps closed issue state and stateReason', () => {
    const node = {
      number: 2,
      title: 'Closed issue',
      body: 'Done',
      createdAt: '2026-01-01T00:00:00Z',
      updatedAt: '2026-01-02T00:00:00Z',
      author: { login: 'dev' },
      authorAssociation: 'CONTRIBUTOR',
      labels: { nodes: [] },
      milestone: null,
      comments: { totalCount: 0 },
      reactions: { totalCount: 0 },
      state: 'CLOSED',
      stateReason: 'completed',
      locked: true,
      assignees: { nodes: [] },
    };

    const issue = mapGraphQLToIssueData(node);
    expect(issue.state).toBe('closed');
    expect(issue.stateReason).toBe('completed');
    expect(issue.isLocked).toBe(true);
  });
});
console.log(JSON.stringify({originalCallbacks:results.length,passed:results.length,results,scope:"original mapper/query and test bodies; import and registration/assertion adapters only; no Jest or GitHub"}));
