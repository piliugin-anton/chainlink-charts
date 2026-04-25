import "server-only";

let shared: AbortController | null = null;
let installed = false;

/**
 * Aborts on SIGINT/SIGTERM so long-lived fetches/streams (e.g. Chainlink proxy)
 * are cancelled when you stop the server — otherwise undici/HTTP can keep
 * the Node process from exiting.
 */
export function getProcessShutdownSignal(): AbortSignal {
  if (!shared) {
    shared = new AbortController();
  }
  if (!installed) {
    installed = true;
    const onShutdown = () => {
      shared?.abort();
    };
    process.on("SIGINT", onShutdown);
    process.on("SIGTERM", onShutdown);
  }
  return shared.signal;
}
