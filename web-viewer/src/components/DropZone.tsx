/**
 * DropZone - File drop area for .sync files
 */

import React, { useCallback, useState } from 'react';
import { Upload, FileArchive, AlertCircle } from 'lucide-react';
import { cn } from '@/lib/utils';
import { parseSyncFile, readFileAsArrayBuffer } from '@/lib/sync-parser';
import type { ParsedSyncFile } from '@/types/sync';

interface DropZoneProps {
  onFileLoaded: (file: ParsedSyncFile) => void;
  onError: (error: Error) => void;
  loading?: boolean;
  className?: string;
}

export function DropZone({ onFileLoaded, onError, loading = false, className }: DropZoneProps) {
  const [isDragging, setIsDragging] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleFile = useCallback(async (file: File) => {
    setError(null);

    if (!file.name.endsWith('.sync')) {
      const err = new Error('Please drop a .sync file');
      setError(err.message);
      onError(err);
      return;
    }

    try {
      const data = await readFileAsArrayBuffer(file);
      const parsed = await parseSyncFile(data, file.name);
      onFileLoaded(parsed);
    } catch (err) {
      const error = err instanceof Error ? err : new Error(String(err));
      setError(error.message);
      onError(error);
    }
  }, [onFileLoaded, onError]);

  const handleDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    setIsDragging(true);
  }, []);

  const handleDragLeave = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    setIsDragging(false);
  }, []);

  const handleDrop = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    setIsDragging(false);

    const files = e.dataTransfer.files;
    if (files.length > 0) {
      handleFile(files[0]);
    }
  }, [handleFile]);

  const handleClick = useCallback(() => {
    const input = document.createElement('input');
    input.type = 'file';
    input.accept = '.sync';
    input.onchange = (e) => {
      const file = (e.target as HTMLInputElement).files?.[0];
      if (file) {
        handleFile(file);
      }
    };
    input.click();
  }, [handleFile]);

  return (
    <div
      onClick={handleClick}
      onDragOver={handleDragOver}
      onDragLeave={handleDragLeave}
      onDrop={handleDrop}
      className={cn(
        'relative flex flex-col items-center justify-center gap-4 rounded-xl border-2 border-dashed p-12 transition-all cursor-pointer',
        isDragging
          ? 'border-blue-500 bg-blue-50'
          : 'border-neutral-300 hover:border-neutral-400 hover:bg-neutral-50',
        loading && 'pointer-events-none opacity-50',
        className
      )}
    >
      {loading ? (
        <div className="flex flex-col items-center gap-3">
          <div className="h-12 w-12 animate-spin rounded-full border-4 border-neutral-200 border-t-blue-500" />
          <p className="text-sm text-neutral-500">Parsing .sync file...</p>
        </div>
      ) : error ? (
        <div className="flex flex-col items-center gap-3 text-red-600">
          <AlertCircle className="h-12 w-12" />
          <p className="text-sm font-medium">{error}</p>
          <p className="text-xs text-neutral-500">Click or drop to try again</p>
        </div>
      ) : (
        <>
          <div className="rounded-full bg-neutral-100 p-4">
            {isDragging ? (
              <FileArchive className="h-8 w-8 text-blue-500" />
            ) : (
              <Upload className="h-8 w-8 text-neutral-400" />
            )}
          </div>
          <div className="text-center">
            <p className="text-lg font-medium text-neutral-700">
              {isDragging ? 'Drop your .sync file' : 'Drop a .sync file here'}
            </p>
            <p className="mt-1 text-sm text-neutral-500">
              or click to browse
            </p>
          </div>
          <div className="mt-2 flex items-center gap-2 rounded-full bg-neutral-100 px-3 py-1.5">
            <FileArchive className="h-4 w-4 text-neutral-500" />
            <span className="text-xs font-medium text-neutral-600">.sync</span>
          </div>
        </>
      )}
    </div>
  );
}
