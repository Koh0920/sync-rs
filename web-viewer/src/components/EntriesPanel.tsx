/**
 * EntriesPanel - Display all entries in the archive
 */

import { Archive, FileText, Code, Binary, Database } from 'lucide-react';
import { Card, CardHeader, CardTitle, CardContent } from '@/components/ui/Card';
import { formatBytes } from '@/lib/content-renderer';
import type { SyncEntry } from '@/types/sync';

interface EntriesPanelProps {
  entries: SyncEntry[];
}

export function EntriesPanel({ entries }: EntriesPanelProps) {
  return (
    <Card>
      <CardHeader className="pb-3">
        <CardTitle className="flex items-center gap-2 text-base">
          <Archive className="h-4 w-4" />
          Archive Entries ({entries.length})
        </CardTitle>
      </CardHeader>
      <CardContent>
        <div className="space-y-1">
          {entries.map((entry, i) => (
            <div 
              key={i}
              className="flex items-center justify-between rounded-lg px-3 py-2 hover:bg-neutral-50"
            >
              <div className="flex items-center gap-2">
                <EntryIcon name={entry.name} />
                <span className="font-mono text-sm">{entry.name}</span>
              </div>
              <span className="text-xs text-neutral-500">
                {formatBytes(entry.size)}
              </span>
            </div>
          ))}
        </div>
      </CardContent>
    </Card>
  );
}

function EntryIcon({ name }: { name: string }) {
  if (name === 'manifest.toml') {
    return <FileText className="h-4 w-4 text-blue-500" />;
  }
  if (name === 'sync.wasm') {
    return <Code className="h-4 w-4 text-purple-500" />;
  }
  if (name === 'payload') {
    return <Database className="h-4 w-4 text-green-500" />;
  }
  if (name === 'context.json') {
    return <Code className="h-4 w-4 text-amber-500" />;
  }
  if (name === 'sync.proof') {
    return <Binary className="h-4 w-4 text-red-500" />;
  }
  return <Binary className="h-4 w-4 text-neutral-400" />;
}
