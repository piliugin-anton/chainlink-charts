import "server-only";

let shared: AbortController | null = null;
let installed = false;

/**
 * Aborts on SIGINT/SIGTERM so long-lived fetches/streams (e.g. Chainlink proxy)
 * are cancelled when you stop the server — otherwise undici/HTTP can keep
 * the Node process from exiting.
 *
 * Node disables the default SIGINT/SIGTERM exit once any listener is
 * registered, so we schedule `process.exit` after aborting so Ctrl+C still
 * tears down `next dev` / `next start` reliably.
 */
export function getProcessShutdownSignal(): AbortSignal {
  if (!shared) {
    shared = new AbortController();
  }
  if (!installed) {
    installed = true;
    const onShutdown = () => {
      shared?.abort();
      setImmediate(() => {
        process.exit(0);
      });
    };
    process.on("SIGINT", onShutdown);
    process.on("SIGTERM", onShutdown);
  }
  return shared.signal;
}
