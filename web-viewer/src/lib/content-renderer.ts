/**
 * Content rendering utilities for different payload types
 */

import { type ContentCategory, categorizeContentType } from '@/types/sync';

export interface RenderableContent {
  category: ContentCategory;
  /** Text content for text-based types */
  text?: string;
  /** Data URL for images */
  dataUrl?: string;
  /** Parsed JSON for JSON types */
  json?: unknown;
  /** Parsed CSV rows */
  csv?: string[][];
  /** Raw bytes for binary */
  bytes?: Uint8Array;
}

/**
 * Convert payload to renderable content based on content type
 */
export function renderPayload(
  payload: Uint8Array,
  contentType: string
): RenderableContent {
  const category = categorizeContentType(contentType);
  const result: RenderableContent = { category };

  switch (category) {
    case 'text':
      result.text = new TextDecoder().decode(payload);
      break;

    case 'json':
      try {
        const text = new TextDecoder().decode(payload);
        result.json = JSON.parse(text);
        result.text = text;
      } catch {
        result.text = new TextDecoder().decode(payload);
      }
      break;

    case 'csv':
      try {
        const text = new TextDecoder().decode(payload);
        result.csv = parseCSV(text);
        result.text = text;
      } catch {
        result.text = new TextDecoder().decode(payload);
      }
      break;

    case 'image':
      result.dataUrl = createDataUrl(payload, contentType);
      break;

    case 'binary':
    default:
      result.bytes = payload;
      break;
  }

  return result;
}

/**
 * Parse CSV string into 2D array
 */
function parseCSV(text: string): string[][] {
  const lines = text.split('\n');
  return lines.map(line => {
    const cells: string[] = [];
    let current = '';
    let inQuotes = false;
    
    for (let i = 0; i < line.length; i++) {
      const char = line[i];
      
      if (char === '"' && (i === 0 || line[i - 1] !== '\\')) {
        inQuotes = !inQuotes;
      } else if (char === ',' && !inQuotes) {
        cells.push(current.trim());
        current = '';
      } else {
        current += char;
      }
    }
    
    cells.push(current.trim());
    return cells;
  }).filter(row => row.some(cell => cell.length > 0));
}

/**
 * Create data URL from bytes
 */
function createDataUrl(data: Uint8Array, mimeType: string): string {
  const base64 = btoa(
    Array.from(data)
      .map(byte => String.fromCharCode(byte))
      .join('')
  );
  return `data:${mimeType};base64,${base64}`;
}

/**
 * Format bytes as human-readable string
 */
export function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${parseFloat((bytes / Math.pow(k, i)).toFixed(2))} ${sizes[i]}`;
}

/**
 * Format date for display
 */
export function formatDate(isoString: string): string {
  try {
    const date = new Date(isoString);
    return date.toLocaleString();
  } catch {
    return isoString;
  }
}

/**
 * Get file extension from display_ext
 */
export function normalizeExtension(ext: string): string {
  return ext.replace(/^\.+/, '').toLowerCase();
}

/**
 * Syntax highlighting for code (simple implementation)
 */
export function highlightSyntax(text: string, _ext: string): string {
  // For now, just return the text; could add syntax highlighting later
  return text;
}
