import { useState, useCallback } from 'react';
import { parseSyncFile } from '@/lib/sync-parser';
import type { ParsedSyncFile } from '@/types/sync';

interface UseSyncFileResult {
  syncFile: ParsedSyncFile | null;
  loading: boolean;
  error: string | null;
  loadFile: (file: File) => Promise<void>;
  clear: () => void;
}

export function useSyncFile(): UseSyncFileResult {
  const [syncFile, setSyncFile] = useState<ParsedSyncFile | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const loadFile = useCallback(async (file: File) => {
    setLoading(true);
    setError(null);

    try {
      const arrayBuffer = await file.arrayBuffer();
      const bytes = new Uint8Array(arrayBuffer);
      const parsed = await parseSyncFile(bytes, file.name);
      setSyncFile(parsed);
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Unknown error';
      setError(message);
      setSyncFile(null);
    } finally {
      setLoading(false);
    }
  }, []);

  const clear = useCallback(() => {
    setSyncFile(null);
    setError(null);
  }, []);

  return { syncFile, loading, error, loadFile, clear };
}
