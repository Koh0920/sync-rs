/**
 * Type definitions for .sync file format
 * 
 * These types mirror the Rust implementations in sync-format crate
 * for browser-side parsing and validation.
 */

import { z } from 'zod';

// =============================================================================
// Zod Schemas (Runtime Validation)
// =============================================================================

export const SyncSectionSchema = z.object({
  version: z.string(),
  content_type: z.string(),
  display_ext: z.string(),
});

export const ManifestMetadataSchema = z.object({
  created_by: z.string(),
  created_at: z.string(),
  hash_algo: z.string(),
});

export const ManifestPolicySchema = z.object({
  ttl: z.number(),
  timeout: z.number(),
});

export const ManifestPermissionsSchema = z.object({
  allow_hosts: z.array(z.string()).default([]),
  allow_env: z.array(z.string()).default([]),
});

export const ManifestOwnershipSchema = z.object({
  owner_capsule: z.string().optional(),
  write_allowed: z.boolean().default(false),
});

export const ManifestVerificationSchema = z.object({
  enabled: z.boolean().default(false),
  vm_type: z.string().optional(),
  proof_type: z.string().optional(),
});

export const SyncManifestSchema = z.object({
  sync: SyncSectionSchema,
  meta: ManifestMetadataSchema,
  policy: ManifestPolicySchema,
  permissions: ManifestPermissionsSchema.default({ allow_hosts: [], allow_env: [] }),
  ownership: ManifestOwnershipSchema.default({ write_allowed: false }),
  verification: ManifestVerificationSchema.default({ enabled: false }),
});

// =============================================================================
// TypeScript Types (Inferred from Zod Schemas)
// =============================================================================

export type SyncSection = z.infer<typeof SyncSectionSchema>;
export type ManifestMetadata = z.infer<typeof ManifestMetadataSchema>;
export type ManifestPolicy = z.infer<typeof ManifestPolicySchema>;
export type ManifestPermissions = z.infer<typeof ManifestPermissionsSchema>;
export type ManifestOwnership = z.infer<typeof ManifestOwnershipSchema>;
export type ManifestVerification = z.infer<typeof ManifestVerificationSchema>;
export type SyncManifest = z.infer<typeof SyncManifestSchema>;

// =============================================================================
// Archive Entry Types
// =============================================================================

export interface SyncEntry {
  /** Entry name within the archive (e.g., "manifest.toml", "payload") */
  name: string;
  /** Byte offset within the archive */
  offset: number;
  /** Uncompressed size in bytes */
  size: number;
  /** Raw data of the entry */
  data: Uint8Array;
}

export interface ParsedSyncFile {
  /** Original filename */
  fileName: string;
  /** Total file size in bytes */
  fileSize: number;
  /** Parsed and validated manifest */
  manifest: SyncManifest;
  /** Payload data (raw bytes) */
  payload?: Uint8Array;
  /** WASM module bytes */
  wasm?: Uint8Array;
  /** Context JSON for WASM execution */
  context?: Record<string, unknown>;
  /** Proof data if present */
  proof?: Uint8Array;
  /** All entries in the archive */
  entries: SyncEntry[];
}

// =============================================================================
// WASM Execution Types (Matching sync-runtime GuestSession)
// =============================================================================

export const GUEST_PROTOCOL_VERSION = 'guest.v1';

export type GuestMode = 'Widget' | 'Headless';

export type GuestContextRole = 'Consumer' | 'Owner';

export interface GuestPermission {
  can_read_payload: boolean;
  can_read_context: boolean;
  can_write_payload: boolean;
  can_write_context: boolean;
  can_execute_wasm: boolean;
  allowed_hosts: string[];
  allowed_env: string[];
}

export interface GuestContext {
  mode: GuestMode;
  role: GuestContextRole;
  permissions: GuestPermission;
  sync_path: string;
  host_app: string | null;
}

export type GuestAction =
  | 'ReadPayload'
  | 'ReadContext'
  | 'WritePayload'
  | 'WriteContext'
  | 'ExecuteWasm'
  | 'UpdatePayload';

export interface GuestRequest {
  version: string;
  request_id: string;
  action: GuestAction;
  context: GuestContext;
  input: unknown;
}

export type GuestErrorCode =
  | 'PermissionDenied'
  | 'InvalidRequest'
  | 'ExecutionFailed'
  | 'HostUnavailable'
  | 'ProtocolError'
  | 'IoError';

export interface GuestError {
  code: GuestErrorCode;
  message: string;
}

export interface GuestResponse {
  version: string;
  request_id: string;
  ok: boolean;
  result: unknown | null;
  error: GuestError | null;
}

// =============================================================================
// Content Types
// =============================================================================

export type ContentCategory = 
  | 'text'
  | 'image'
  | 'json'
  | 'csv'
  | 'binary'
  | 'unknown';

export function categorizeContentType(contentType: string): ContentCategory {
  const ct = contentType.toLowerCase();
  
  if (ct.startsWith('text/') || ct === 'application/javascript') {
    return 'text';
  }
  if (ct.startsWith('image/')) {
    return 'image';
  }
  if (ct === 'application/json' || ct.endsWith('+json')) {
    return 'json';
  }
  if (ct === 'text/csv' || ct === 'application/csv') {
    return 'csv';
  }
  if (ct.startsWith('application/')) {
    return 'binary';
  }
  return 'unknown';
}
