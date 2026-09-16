export interface BooksRef {
  fileName: string;
  booksId: string;
  actor: string;
}

export interface CreateBooksRequest extends BooksRef {
  companyName: string;
}

type ExistingShellCommand =
  | "foundation_health"
  | "production_encryption_required"
  | "books_create"
  | "books_open"
  | "books_verify";

function nativeInvoke<T>(
  command: ExistingShellCommand,
  args?: Record<string, unknown>,
): Promise<T> {
  const invoke = window.__TAURI__?.core?.invoke;
  if (!invoke) {
    return Promise.reject(new Error("SharkBooks native bridge is unavailable."));
  }
  return invoke<T>(command, args);
}

export function foundationHealth(): Promise<string> {
  return nativeInvoke<string>("foundation_health");
}

export function productionEncryptionRequired(): Promise<boolean> {
  return nativeInvoke<boolean>("production_encryption_required");
}

export function createBooks(request: CreateBooksRequest): Promise<unknown> {
  return nativeInvoke("books_create", { request });
}

export function openBooks(request: BooksRef): Promise<unknown> {
  return nativeInvoke("books_open", { request });
}

export function verifyBooks(request: BooksRef): Promise<unknown> {
  return nativeInvoke("books_verify", { request });
}
