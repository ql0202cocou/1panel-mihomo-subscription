// Minimal API client. Management calls are same-origin JSON; a 401 triggers the
// registered unauthorized handler so the SPA can redirect to login.

export interface ApiError {
  status: number;
  code?: string;
  message?: string;
  details?: string[];
}

let onUnauthorized: (() => void) | null = null;

export function setUnauthorizedHandler(fn: (() => void) | null): void {
  onUnauthorized = fn;
}

export async function api<T>(path: string, opts: RequestInit = {}): Promise<T> {
  const res = await fetch(path, {
    headers: { "Content-Type": "application/json", ...(opts.headers ?? {}) },
    ...opts,
  });

  if (res.status === 401) {
    onUnauthorized?.();
    throw { status: 401 } as ApiError;
  }

  if (!res.ok) {
    let body: { error?: Partial<ApiError> } = {};
    try {
      body = await res.json();
    } catch {
      // non-JSON error body; keep the status only
    }
    throw { status: res.status, ...(body.error ?? {}) } as ApiError;
  }

  if (res.status === 204) {
    return undefined as T;
  }
  const contentType = res.headers.get("content-type") ?? "";
  if (contentType.includes("application/json")) {
    return (await res.json()) as T;
  }
  return (await res.text()) as unknown as T;
}
