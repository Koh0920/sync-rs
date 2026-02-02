/**
 * sync-rs Web Viewer
 * 
 * A browser-based viewer for .sync files that enables:
 * - Zero-install viewing of .sync archives
 * - Manifest inspection
 * - Payload visualization
 * - Sandboxed WASM execution
 */

import { useState, useCallback } from 'react';
import { FileArchive, Github, ExternalLink, RefreshCw } from 'lucide-react';
import { DropZone } from '@/components/DropZone';
import { ManifestPanel } from '@/components/ManifestPanel';
import { PayloadViewer } from '@/components/PayloadViewer';
import { WasmPanel } from '@/components/WasmPanel';
import { EntriesPanel } from '@/components/EntriesPanel';
import { Button } from '@/components/ui/Button';
import type { ParsedSyncFile } from '@/types/sync';

function App() {
  const [syncFile, setSyncFile] = useState<ParsedSyncFile | null>(null);
  const [loading, setLoading] = useState(false);
  const [, setError] = useState<Error | null>(null);

  const handleFileLoaded = useCallback((file: ParsedSyncFile) => {
    setSyncFile(file);
    setError(null);
    setLoading(false);
  }, []);

  const handleError = useCallback((err: Error) => {
    setError(err);
    setLoading(false);
  }, []);

  const handleReset = useCallback(() => {
    setSyncFile(null);
    setError(null);
  }, []);

  return (
    <div className="min-h-screen bg-neutral-50">
      {/* Header */}
      <header className="border-b border-neutral-200 bg-white">
        <div className="mx-auto max-w-7xl px-4 py-4">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-3">
              <div className="flex h-10 w-10 items-center justify-center rounded-lg bg-gradient-to-br from-blue-500 to-purple-600">
                <FileArchive className="h-5 w-5 text-white" />
              </div>
              <div>
                <h1 className="text-lg font-semibold text-neutral-900">
                  sync-rs Web Viewer
                </h1>
                <p className="text-xs text-neutral-500">
                  View .sync files in your browser
                </p>
              </div>
            </div>
            <div className="flex items-center gap-2">
              {syncFile && (
                <Button variant="outline" size="sm" onClick={handleReset}>
                  <RefreshCw className="h-4 w-4 mr-1" />
                  Open Another
                </Button>
              )}
              <a
                href="https://github.com/anomalyco/sync-rs"
                target="_blank"
                rel="noopener noreferrer"
                className="flex items-center gap-1 text-sm text-neutral-500 hover:text-neutral-700"
              >
                <Github className="h-4 w-4" />
                <span className="hidden sm:inline">GitHub</span>
                <ExternalLink className="h-3 w-3" />
              </a>
            </div>
          </div>
        </div>
      </header>

      {/* Main Content */}
      <main className="mx-auto max-w-7xl px-4 py-8">
        {!syncFile ? (
          /* Drop Zone View */
          <div className="mx-auto max-w-2xl">
            <DropZone
              onFileLoaded={handleFileLoaded}
              onError={handleError}
              loading={loading}
              className="min-h-[400px]"
            />
            
            {/* Sample Files Section */}
            <div className="mt-8 rounded-xl border border-neutral-200 bg-white p-6">
              <h2 className="text-lg font-semibold text-neutral-900 mb-4">
                📦 Try Sample Files
              </h2>
              <p className="text-sm text-neutral-600 mb-4">
                Don't have a .sync file? Try one of these samples:
              </p>
              <div className="grid grid-cols-2 md:grid-cols-3 gap-3">
                {[
                  { name: 'hello.sync', desc: 'Plain text', icon: '📝' },
                  { name: 'data.sync', desc: 'CSV data', icon: '📊' },
                  { name: 'config.sync', desc: 'JSON config', icon: '⚙️' },
                  { name: 'image.sync', desc: 'SVG image', icon: '🖼️' },
                  { name: 'readme.sync', desc: 'Markdown', icon: '📖' },
                  { name: 'widget.sync', desc: 'HTML widget', icon: '🎨' },
                ].map((sample) => (
                  <button
                    key={sample.name}
                    onClick={async () => {
                      setLoading(true);
                      try {
                        const response = await fetch(`/samples/${sample.name}`);
                        const blob = await response.blob();
                        const file = new File([blob], sample.name, { type: 'application/octet-stream' });
                        const arrayBuffer = await file.arrayBuffer();
                        const bytes = new Uint8Array(arrayBuffer);
                        const { parseSyncFile } = await import('@/lib/sync-parser');
                        const parsed = await parseSyncFile(bytes, sample.name);
                        handleFileLoaded(parsed);
                      } catch (err) {
                        handleError(err instanceof Error ? err : new Error('Failed to load sample'));
                      }
                    }}
                    className="flex items-center gap-2 p-3 rounded-lg border border-neutral-200 hover:border-blue-300 hover:bg-blue-50 transition-colors text-left"
                  >
                    <span className="text-2xl">{sample.icon}</span>
                    <div>
                      <div className="text-sm font-medium text-neutral-900">{sample.name}</div>
                      <div className="text-xs text-neutral-500">{sample.desc}</div>
                    </div>
                  </button>
                ))}
              </div>
            </div>
            
            {/* Info Section */}
            <div className="mt-8 rounded-xl border border-neutral-200 bg-white p-6">
              <h2 className="text-lg font-semibold text-neutral-900 mb-4">
                What is a .sync file?
              </h2>
              <p className="text-sm text-neutral-600 mb-4">
                A <code className="rounded bg-neutral-100 px-1.5 py-0.5 font-mono text-xs">.sync</code> file 
                is a self-updating archive format that combines data with embedded update logic. It features:
              </p>
              <ul className="space-y-2 text-sm text-neutral-600">
                <li className="flex items-start gap-2">
                  <span className="mt-1 h-1.5 w-1.5 rounded-full bg-blue-500 flex-shrink-0" />
                  <span><strong>Zero-Copy Access:</strong> Instant data access without extraction overhead</span>
                </li>
                <li className="flex items-start gap-2">
                  <span className="mt-1 h-1.5 w-1.5 rounded-full bg-purple-500 flex-shrink-0" />
                  <span><strong>Self-Updating Logic:</strong> Embedded WASM modules for autonomous updates</span>
                </li>
                <li className="flex items-start gap-2">
                  <span className="mt-1 h-1.5 w-1.5 rounded-full bg-green-500 flex-shrink-0" />
                  <span><strong>Sandboxed Execution:</strong> Policy-driven permission model with isolation</span>
                </li>
                <li className="flex items-start gap-2">
                  <span className="mt-1 h-1.5 w-1.5 rounded-full bg-amber-500 flex-shrink-0" />
                  <span><strong>ZIP Compatible:</strong> Standard ZIP container with specialized structure</span>
                </li>
              </ul>
            </div>
          </div>
        ) : (
          /* File Viewer */
          <div className="space-y-6">
            {/* File Header */}
            <div className="flex items-center gap-4">
              <div className="flex h-12 w-12 items-center justify-center rounded-lg bg-gradient-to-br from-green-500 to-teal-600">
                <FileArchive className="h-6 w-6 text-white" />
              </div>
              <div>
                <h2 className="text-xl font-semibold text-neutral-900">
                  {syncFile.fileName}
                </h2>
                <p className="text-sm text-neutral-500">
                  Format v{syncFile.manifest.sync.version} • {syncFile.entries.length} entries
                </p>
              </div>
            </div>

            {/* Grid Layout */}
            <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
              {/* Left Column - Manifest & Entries */}
              <div className="space-y-6">
                <ManifestPanel
                  manifest={syncFile.manifest}
                  fileName={syncFile.fileName}
                  fileSize={syncFile.fileSize}
                />
                <EntriesPanel entries={syncFile.entries} />
                {syncFile.wasm && (
                  <WasmPanel
                    wasm={syncFile.wasm}
                    payload={syncFile.payload || new Uint8Array()}
                    context={syncFile.context}
                    permissions={syncFile.manifest.permissions}
                    timeout={syncFile.manifest.policy.timeout}
                  />
                )}
              </div>

              {/* Right Column - Payload */}
              <div className="lg:col-span-2">
                {syncFile.payload ? (
                  <PayloadViewer
                    payload={syncFile.payload}
                    contentType={syncFile.manifest.sync.content_type}
                    displayExt={syncFile.manifest.sync.display_ext}
                    fileName={syncFile.fileName}
                  />
                ) : (
                  <div className="flex h-64 items-center justify-center rounded-xl border border-neutral-200 bg-white text-neutral-400">
                    No payload in this archive
                  </div>
                )}
              </div>
            </div>
          </div>
        )}
      </main>

      {/* Footer */}
      <footer className="border-t border-neutral-200 bg-white mt-auto">
        <div className="mx-auto max-w-7xl px-4 py-4">
          <p className="text-center text-xs text-neutral-400">
            sync-rs Web Viewer • Part of the{' '}
            <a 
              href="https://magnetic.computer" 
              target="_blank" 
              rel="noopener noreferrer"
              className="text-blue-500 hover:underline"
            >
              Magnetic Web
            </a>
            {' '}ecosystem
          </p>
        </div>
      </footer>
    </div>
  );
}

export default App;
