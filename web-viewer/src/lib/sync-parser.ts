/**
 * .sync file parser using fflate for ZIP extraction
 * 
 * This module handles:
 * - ZIP archive extraction
 * - manifest.toml parsing and validation
 * - Entry enumeration
 */

import { unzip } from 'fflate';
import { parse as parseToml } from 'smol-toml';
import { 
  type ParsedSyncFile, 
  type SyncEntry, 
  type SyncManifest,
  SyncManifestSchema 
} from '@/types/sync';

export type SyncParseErrorCode = 'INVALID_ZIP' | 'MISSING_ENTRY' | 'INVALID_MANIFEST' | 'INVALID_TOML';

export class SyncParseError extends Error {
  readonly code: SyncParseErrorCode;

  constructor(message: string, code: SyncParseErrorCode) {
    super(message);
    this.name = 'SyncParseError';
    this.code = code;
  }
}

/**
 * Parse a .sync file from raw bytes
 */
export async function parseSyncFile(
  data: Uint8Array,
  fileName: string
): Promise<ParsedSyncFile> {
  // Unzip the archive
  const files = await unzipAsync(data);
  
  // Build entries list
  const entries: SyncEntry[] = Object.entries(files).map(([name, data]) => ({
    name,
    offset: 0, // fflate doesn't expose offset
    size: data.length,
    data,
  }));

  // Extract required entries
  const manifestEntry = files['manifest.toml'];
  if (!manifestEntry) {
    throw new SyncParseError(
      'manifest.toml not found in archive',
      'MISSING_ENTRY'
    );
  }

  // Parse manifest
  const manifest = parseManifest(manifestEntry);

  // Extract optional entries
  const payload = files['payload'];
  const wasm = files['sync.wasm'];
  const contextData = files['context.json'];
  const proof = files['sync.proof'];

  // Parse context if present
  let context: Record<string, unknown> | undefined;
  if (contextData) {
    try {
      const text = new TextDecoder().decode(contextData);
      context = JSON.parse(text);
    } catch (e) {
      console.warn('Failed to parse context.json:', e);
    }
  }

  return {
    fileName,
    fileSize: data.length,
    manifest,
    payload,
    wasm,
    context,
    proof,
    entries,
  };
}

/**
 * Promisified unzip using fflate
 */
function unzipAsync(data: Uint8Array): Promise<Record<string, Uint8Array>> {
  return new Promise((resolve, reject) => {
    unzip(data, (err, result) => {
      if (err) {
        reject(new SyncParseError(
          `Failed to unzip: ${err.message}`,
          'INVALID_ZIP'
        ));
      } else {
        resolve(result);
      }
    });
  });
}

/**
 * Parse and validate manifest.toml
 */
function parseManifest(data: Uint8Array): SyncManifest {
  const text = new TextDecoder().decode(data);
  
  let parsed: unknown;
  try {
    parsed = parseToml(text);
  } catch (e) {
    throw new SyncParseError(
      `Invalid TOML: ${e instanceof Error ? e.message : 'Unknown error'}`,
      'INVALID_TOML'
    );
  }

  // Validate with Zod
  const result = SyncManifestSchema.safeParse(parsed);
  if (!result.success) {
    const issues = result.error.issues
      .map(i => `${i.path.join('.')}: ${i.message}`)
      .join('; ');
    throw new SyncParseError(
      `Invalid manifest: ${issues}`,
      'INVALID_MANIFEST'
    );
  }

  return result.data;
}

/**
 * Read a file as Uint8Array
 */
export function readFileAsArrayBuffer(file: File): Promise<Uint8Array> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => {
      if (reader.result instanceof ArrayBuffer) {
        resolve(new Uint8Array(reader.result));
      } else {
        reject(new Error('Failed to read file as ArrayBuffer'));
      }
    };
    reader.onerror = () => reject(reader.error);
    reader.readAsArrayBuffer(file);
  });
}

/**
 * Check if the archive has expired based on TTL
 */
export function isExpired(manifest: SyncManifest): boolean {
  try {
    const createdAt = new Date(manifest.meta.created_at);
    const expiresAt = new Date(createdAt.getTime() + manifest.policy.ttl * 1000);
    return Date.now() > expiresAt.getTime();
  } catch {
    return false;
  }
}

/**
 * Calculate remaining time until expiration
 */
export function getExpiresIn(manifest: SyncManifest): number {
  try {
    const createdAt = new Date(manifest.meta.created_at);
    const expiresAt = new Date(createdAt.getTime() + manifest.policy.ttl * 1000);
    return Math.max(0, expiresAt.getTime() - Date.now());
  } catch {
    return 0;
  }
}

/**
 * Format TTL as human-readable string
 */
export function formatTtl(seconds: number): string {
  if (seconds < 60) return `${seconds}s`;
  if (seconds < 3600) return `${Math.floor(seconds / 60)}m`;
  if (seconds < 86400) return `${Math.floor(seconds / 3600)}h`;
  return `${Math.floor(seconds / 86400)}d`;
}
