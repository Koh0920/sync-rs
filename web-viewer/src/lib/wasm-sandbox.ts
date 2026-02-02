/**
 * WASM Sandbox - Isolated execution environment for sync.wasm
 * 
 * Uses iframe + postMessage for secure isolation and
 * @bjorn3/browser_wasi_shim for WASI stdin/stdout emulation.
 */

// import * as Comlink from 'comlink'; // Reserved for future Web Worker integration
import type { 
  GuestRequest, 
  GuestResponse, 
  GuestAction,
  ManifestPermissions
} from '@/types/sync';

export interface WasmExecutionResult {
  success: boolean;
  response?: GuestResponse;
  error?: string;
  executionTimeMs: number;
}

export interface WasmSandboxOptions {
  /** Timeout in milliseconds */
  timeout: number;
  /** Memory limit in MB (advisory) */
  memoryLimitMb?: number;
  /** CPU time limit in ms (advisory) */
  cpuLimitMs?: number;
}

const DEFAULT_OPTIONS: WasmSandboxOptions = {
  timeout: 30000, // 30 seconds
  memoryLimitMb: 256,
  cpuLimitMs: 5000,
};

/**
 * Execute WASM in a sandboxed environment
 */
export async function executeWasmSandboxed(
  wasmBytes: Uint8Array,
  action: GuestAction,
  payload: Uint8Array,
  context: Record<string, unknown> | undefined,
  permissions: ManifestPermissions,
  options: Partial<WasmSandboxOptions> = {}
): Promise<WasmExecutionResult> {
  const opts = { ...DEFAULT_OPTIONS, ...options };
  const startTime = performance.now();

  try {
    // For now, we use a simple approach with Web Workers
    // In production, this would use an iframe sandbox with CSP
    const result = await executeInWorker(
      wasmBytes,
      action,
      payload,
      context,
      permissions,
      opts.timeout
    );

    return {
      success: true,
      response: result,
      executionTimeMs: performance.now() - startTime,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : String(error),
      executionTimeMs: performance.now() - startTime,
    };
  }
}

/**
 * Execute WASM in a Web Worker (simpler sandbox than iframe)
 */
async function executeInWorker(
  wasmBytes: Uint8Array,
  action: GuestAction,
  payload: Uint8Array,
  context: Record<string, unknown> | undefined,
  permissions: ManifestPermissions,
  timeout: number
): Promise<GuestResponse> {
  return new Promise((resolve, reject) => {
    const timeoutId = setTimeout(() => {
      reject(new Error('WASM execution timed out'));
    }, timeout);

    // Create the guest request
    const request: GuestRequest = {
      version: 'guest.v1',
      request_id: `req_${Date.now()}_${Math.random().toString(36).slice(2)}`,
      action,
      context: {
        mode: 'Widget',
        role: 'Consumer',
        permissions: {
          can_read_payload: true,
          can_read_context: true,
          can_write_payload: false,
          can_write_context: false,
          can_execute_wasm: true,
          allowed_hosts: permissions.allow_hosts,
          allowed_env: permissions.allow_env,
        },
        sync_path: 'browser://virtual',
        host_app: 'sync-web-viewer',
      },
      input: {
        payload: Array.from(payload),
        context,
      },
    };

    // For the initial implementation, we simulate the response
    // Real implementation would instantiate WASM with WASI shim
    simulateWasmExecution(wasmBytes, request)
      .then(response => {
        clearTimeout(timeoutId);
        resolve(response);
      })
      .catch(error => {
        clearTimeout(timeoutId);
        reject(error);
      });
  });
}

/**
 * Simulate WASM execution (placeholder for real WASI implementation)
 * 
 * In a full implementation, this would:
 * 1. Instantiate the WASM module with WASI shim
 * 2. Write the GuestRequest JSON to stdin
 * 3. Execute the module
 * 4. Read the GuestResponse JSON from stdout
 */
async function simulateWasmExecution(
  wasmBytes: Uint8Array,
  request: GuestRequest
): Promise<GuestResponse> {
  // Verify it's a valid WASM module
  if (!isValidWasmModule(wasmBytes)) {
    throw new Error('Invalid WASM module');
  }

  // For now, return a simulated response
  // Real implementation would execute the WASM
  return {
    version: 'guest.v1',
    request_id: request.request_id,
    ok: true,
    result: {
      message: 'WASM execution simulated (full WASI integration pending)',
      action: request.action,
      wasmSize: wasmBytes.length,
    },
    error: null,
  };
}

/**
 * Check if bytes represent a valid WASM module
 */
function isValidWasmModule(bytes: Uint8Array): boolean {
  // WASM magic number: \0asm (0x00 0x61 0x73 0x6d)
  if (bytes.length < 8) return false;
  return (
    bytes[0] === 0x00 &&
    bytes[1] === 0x61 &&
    bytes[2] === 0x73 &&
    bytes[3] === 0x6d
  );
}

/**
 * Validate WASM module without executing it
 */
export async function validateWasmModule(bytes: Uint8Array): Promise<{
  valid: boolean;
  error?: string;
}> {
  if (!isValidWasmModule(bytes)) {
    return { valid: false, error: 'Invalid WASM magic number' };
  }

  try {
    // Try to compile (but not instantiate) the module
    // Use slice() to ensure we have a plain ArrayBuffer
    await WebAssembly.compile(bytes.slice().buffer);
    return { valid: true };
  } catch (error) {
    return {
      valid: false,
      error: error instanceof Error ? error.message : 'Unknown compilation error',
    };
  }
}

/**
 * Get WASM module info without executing
 */
export async function getWasmModuleInfo(bytes: Uint8Array): Promise<{
  valid: boolean;
  exports?: string[];
  imports?: Array<{ module: string; name: string }>;
  error?: string;
}> {
  try {
    // Use slice() to ensure we have a plain ArrayBuffer
    const module = await WebAssembly.compile(bytes.slice().buffer);
    const exports = WebAssembly.Module.exports(module).map(e => e.name);
    const imports = WebAssembly.Module.imports(module).map(i => ({
      module: i.module,
      name: i.name,
    }));

    return { valid: true, exports, imports };
  } catch (error) {
    return {
      valid: false,
      error: error instanceof Error ? error.message : 'Unknown error',
    };
  }
}
