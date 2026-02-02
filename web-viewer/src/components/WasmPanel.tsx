/**
 * WasmPanel - WASM module info and execution controls
 */

import { useState, useEffect } from 'react';
import { Cpu, Play, AlertTriangle, CheckCircle, Info, Shield } from 'lucide-react';
import { Card, CardHeader, CardTitle, CardContent } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { formatBytes } from '@/lib/content-renderer';
import { 
  getWasmModuleInfo, 
  executeWasmSandboxed,
  type WasmExecutionResult 
} from '@/lib/wasm-sandbox';
import type { ManifestPermissions } from '@/types/sync';

interface WasmPanelProps {
  wasm: Uint8Array;
  payload: Uint8Array;
  context?: Record<string, unknown>;
  permissions: ManifestPermissions;
  timeout: number;
}

interface WasmInfo {
  valid: boolean;
  exports?: string[];
  imports?: Array<{ module: string; name: string }>;
  error?: string;
}

export function WasmPanel({ wasm, payload, context, permissions, timeout }: WasmPanelProps) {
  const [wasmInfo, setWasmInfo] = useState<WasmInfo | null>(null);
  const [loading, setLoading] = useState(true);
  const [executing, setExecuting] = useState(false);
  const [result, setResult] = useState<WasmExecutionResult | null>(null);
  const [showPermissionWarning, setShowPermissionWarning] = useState(false);

  useEffect(() => {
    let mounted = true;
    
    getWasmModuleInfo(wasm).then(info => {
      if (mounted) {
        setWasmInfo(info);
        setLoading(false);
      }
    });

    return () => { mounted = false; };
  }, [wasm]);

  const handleExecute = async () => {
    if (!wasmInfo?.valid) return;

    // Check if there are network permissions that require warning
    if (permissions.allow_hosts.length > 0) {
      setShowPermissionWarning(true);
      return;
    }

    await runExecution();
  };

  const runExecution = async () => {
    setShowPermissionWarning(false);
    setExecuting(true);
    setResult(null);

    try {
      const execResult = await executeWasmSandboxed(
        wasm,
        'ExecuteWasm',
        payload,
        context,
        permissions,
        { timeout: timeout * 1000 }
      );
      setResult(execResult);
    } catch (error) {
      setResult({
        success: false,
        error: error instanceof Error ? error.message : String(error),
        executionTimeMs: 0,
      });
    } finally {
      setExecuting(false);
    }
  };

  return (
    <Card>
      <CardHeader className="pb-3">
        <CardTitle className="flex items-center gap-2 text-base">
          <Cpu className="h-4 w-4" />
          WASM Module
        </CardTitle>
      </CardHeader>
      <CardContent className="space-y-4">
        {/* Module Info */}
        <div className="flex items-center justify-between">
          <span className="text-sm text-neutral-500">Size</span>
          <span className="font-mono text-xs">{formatBytes(wasm.length)}</span>
        </div>

        {loading ? (
          <div className="flex items-center gap-2 text-sm text-neutral-500">
            <div className="h-4 w-4 animate-spin rounded-full border-2 border-neutral-200 border-t-neutral-600" />
            Validating module...
          </div>
        ) : wasmInfo?.valid ? (
          <>
            <div className="flex items-center gap-2 text-sm text-green-600">
              <CheckCircle className="h-4 w-4" />
              Valid WASM module
            </div>

            {/* Exports */}
            {wasmInfo.exports && wasmInfo.exports.length > 0 && (
              <div className="space-y-1">
                <h4 className="text-xs font-semibold uppercase tracking-wider text-neutral-500">
                  Exports ({wasmInfo.exports.length})
                </h4>
                <div className="flex flex-wrap gap-1">
                  {wasmInfo.exports.slice(0, 10).map(exp => (
                    <span 
                      key={exp} 
                      className="rounded bg-neutral-100 px-2 py-0.5 font-mono text-xs"
                    >
                      {exp}
                    </span>
                  ))}
                  {wasmInfo.exports.length > 10 && (
                    <span className="text-xs text-neutral-400">
                      +{wasmInfo.exports.length - 10} more
                    </span>
                  )}
                </div>
              </div>
            )}

            {/* Imports */}
            {wasmInfo.imports && wasmInfo.imports.length > 0 && (
              <div className="space-y-1">
                <h4 className="text-xs font-semibold uppercase tracking-wider text-neutral-500">
                  Imports ({wasmInfo.imports.length})
                </h4>
                <div className="flex flex-wrap gap-1">
                  {wasmInfo.imports.slice(0, 5).map((imp, i) => (
                    <span 
                      key={i} 
                      className="rounded bg-blue-50 px-2 py-0.5 font-mono text-xs text-blue-700"
                    >
                      {imp.module}::{imp.name}
                    </span>
                  ))}
                  {wasmInfo.imports.length > 5 && (
                    <span className="text-xs text-neutral-400">
                      +{wasmInfo.imports.length - 5} more
                    </span>
                  )}
                </div>
              </div>
            )}

            {/* Permission Warning */}
            {showPermissionWarning && (
              <div className="rounded-lg border border-amber-200 bg-amber-50 p-3 space-y-2">
                <div className="flex items-center gap-2 text-amber-700">
                  <Shield className="h-4 w-4" />
                  <span className="font-medium text-sm">Permission Required</span>
                </div>
                <p className="text-xs text-amber-600">
                  This module requests access to: {permissions.allow_hosts.join(', ')}
                </p>
                <div className="flex gap-2">
                  <Button 
                    size="sm" 
                    variant="outline"
                    onClick={() => setShowPermissionWarning(false)}
                  >
                    Cancel
                  </Button>
                  <Button 
                    size="sm"
                    onClick={runExecution}
                  >
                    Allow & Execute
                  </Button>
                </div>
              </div>
            )}

            {/* Execute Button */}
            {!showPermissionWarning && (
              <Button 
                onClick={handleExecute}
                disabled={executing}
                className="w-full"
              >
                {executing ? (
                  <>
                    <div className="h-4 w-4 mr-2 animate-spin rounded-full border-2 border-neutral-400 border-t-white" />
                    Executing...
                  </>
                ) : (
                  <>
                    <Play className="h-4 w-4 mr-2" />
                    Execute WASM
                  </>
                )}
              </Button>
            )}

            {/* Result */}
            {result && (
              <div className={`rounded-lg p-3 ${result.success ? 'bg-green-50' : 'bg-red-50'}`}>
                <div className="flex items-center gap-2 mb-2">
                  {result.success ? (
                    <CheckCircle className="h-4 w-4 text-green-600" />
                  ) : (
                    <AlertTriangle className="h-4 w-4 text-red-600" />
                  )}
                  <span className={`font-medium text-sm ${result.success ? 'text-green-700' : 'text-red-700'}`}>
                    {result.success ? 'Execution Complete' : 'Execution Failed'}
                  </span>
                  <span className="text-xs text-neutral-500 ml-auto">
                    {result.executionTimeMs.toFixed(2)}ms
                  </span>
                </div>
                {result.error && (
                  <p className="text-xs text-red-600 font-mono">{result.error}</p>
                )}
                {result.response && (
                  <pre className="text-xs font-mono bg-white rounded p-2 mt-2 overflow-auto max-h-40">
                    {JSON.stringify(result.response.result, null, 2)}
                  </pre>
                )}
              </div>
            )}
          </>
        ) : (
          <div className="flex items-center gap-2 text-sm text-red-600">
            <AlertTriangle className="h-4 w-4" />
            {wasmInfo?.error || 'Invalid WASM module'}
          </div>
        )}

        {/* Info Note */}
        <div className="flex items-start gap-2 rounded-lg bg-blue-50 p-3">
          <Info className="h-4 w-4 text-blue-500 mt-0.5 flex-shrink-0" />
          <p className="text-xs text-blue-700">
            WASM execution runs in a sandboxed environment. Full WASI stdin/stdout 
            integration is in development for complete sync-runtime compatibility.
          </p>
        </div>
      </CardContent>
    </Card>
  );
}
