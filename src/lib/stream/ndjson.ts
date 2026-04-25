/**
 * ECMAScript `\s` subset used before JSON objects in streams (avoids per-char RegExp).
 */
function isWhitespaceCode(c: number): boolean {
  if (c <= 32) {
    return c === 9 || c === 10 || c === 11 || c === 12 || c === 13 || c === 32;
  }
  return (
    c === 0xa0 ||
    c === 0xfeff ||
    c === 0x1680 ||
    c === 0x202f ||
    c === 0x205f ||
    c === 0x3000 ||
    c === 0x2028 ||
    c === 0x2029 ||
    (c >= 0x2000 && c <= 0x200a)
  );
}

const OPEN = 0x7b; // {
const CLOSE = 0x7d; // }

/**
 * Incrementally parse JSON objects (brace-balanced) from incoming chunks.
 * Incomplete object tail stays in `buffer`.
 */
export function feedJsonChunks(
  priorBuffer: string,
  chunk: string
): { buffer: string; messages: unknown[] } {
  const data = priorBuffer.length === 0 ? chunk : priorBuffer + chunk;
  const messages: unknown[] = [];
  const n = data.length;
  let i = 0;

  while (i < n) {
    while (i < n && isWhitespaceCode(data.charCodeAt(i))) {
      i += 1;
    }
    if (i >= n) {
      break;
    }
    if (data.charCodeAt(i) !== OPEN) {
      i += 1;
      continue;
    }

    const start = i;
    let depth = 0;
    let j = i;
    for (; j < n; j += 1) {
      const c = data.charCodeAt(j);
      if (c === OPEN) depth += 1;
      else if (c === CLOSE) {
        depth -= 1;
        if (depth === 0) {
          const raw = data.slice(start, j + 1);
          try {
            messages.push(JSON.parse(raw) as unknown);
          } catch {
            i = start + 1;
            break;
          }
          i = j + 1;
          break;
        }
      }
    }

    if (j >= n && depth > 0) {
      return { buffer: data.slice(start), messages };
    }
  }

  return { buffer: "", messages };
}
