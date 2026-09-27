export type { Grant, GrantScope, RecordResult } from './grants';
export { GrantStore, DEFAULT_TTL_MS, describeGrant } from './grants';
export type { LoadResult } from './grant-file';
export { loadGrants, saveGrants } from './grant-file';

export type { GateContext, ApprovalRequest, GateDecision } from './gate';
export { gate } from './gate';
