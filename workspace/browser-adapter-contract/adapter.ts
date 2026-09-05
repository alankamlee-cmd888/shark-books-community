export interface BooksMetadata {
  books_id: string;
  company_slug: string;
  company_name: string;
  database_schema_version: number;
  books_format_version: number;
  application_schema_version: number;
  facade_api_version: number;
  crate_version: string;
}

export interface CreateBooksRequest {
  fileName: string;
  booksId: string;
  companyName: string;
  actor: string;
}

export interface OpenBooksRequest {
  fileName: string;
  booksId: string;
  actor: string;
}

export interface ShellBooksStatus {
  fileName: string;
  metadata: BooksMetadata;
  schemaVersion: number;
  productionEncryptionRequired: boolean;
}

export interface ShellVerifyStatus {
  fileName: string;
  booksId: string;
  schemaVersion: number;
  productionEncryptionRequired: boolean;
}

export interface BalanceLine {
  code: string;
  account_type: string;
  debit_total: number;
  credit_total: number;
}

export interface TrialBalance {
  accounts: BalanceLine[];
  total_debits: number;
  total_credits: number;
  balanced: boolean;
}

/**
 * Platform-neutral application seam for the six stable SBC-1 shell operations.
 * Implementations may transport/delegate these operations differently, but must
 * not duplicate accounting rules in the adapter layer.
 */
export interface BooksApplicationAdapter {
  foundationHealth(): Promise<string>;
  productionEncryptionRequired(): Promise<boolean>;
  createBooks(request: CreateBooksRequest): Promise<ShellBooksStatus>;
  openBooks(request: OpenBooksRequest): Promise<ShellBooksStatus>;
  verifyBooks(request: OpenBooksRequest): Promise<ShellVerifyStatus>;
  trialBalance(request: OpenBooksRequest): Promise<TrialBalance>;
}

export type InvokeFn = <T>(
  command: string,
  args?: Record<string, unknown>,
) => Promise<T>;

/** Native adapter: transport only. Persistence remains behind the Rust facade. */
export class TauriBooksAdapter implements BooksApplicationAdapter {
  public constructor(private readonly invoke: InvokeFn) {}

  public foundationHealth(): Promise<string> {
    return this.invoke<string>("foundation_health");
  }

  public productionEncryptionRequired(): Promise<boolean> {
    return this.invoke<boolean>("production_encryption_required");
  }

  public createBooks(request: CreateBooksRequest): Promise<ShellBooksStatus> {
    return this.invoke<ShellBooksStatus>("books_create", { request });
  }

  public openBooks(request: OpenBooksRequest): Promise<ShellBooksStatus> {
    return this.invoke<ShellBooksStatus>("books_open", { request });
  }

  public verifyBooks(request: OpenBooksRequest): Promise<ShellVerifyStatus> {
    return this.invoke<ShellVerifyStatus>("books_verify", { request });
  }

  public trialBalance(request: OpenBooksRequest): Promise<TrialBalance> {
    return this.invoke<TrialBalance>("books_trial_balance", { request });
  }
}

/**
 * Contract only: the future browser persistence implementation supplies this
 * backend. SBC-1F intentionally provides no OPFS, SQLite/WASM or durability
 * implementation and makes no native/browser parity claim.
 */
export type BrowserBooksBackend = BooksApplicationAdapter & {
  readonly kind: "browser-persistence";
};

/** Browser adapter: delegates application semantics without accounting logic. */
export class BrowserBooksAdapter implements BooksApplicationAdapter {
  public constructor(private readonly backend: BrowserBooksBackend) {}

  public foundationHealth(): Promise<string> {
    return this.backend.foundationHealth();
  }

  public productionEncryptionRequired(): Promise<boolean> {
    return this.backend.productionEncryptionRequired();
  }

  public createBooks(request: CreateBooksRequest): Promise<ShellBooksStatus> {
    return this.backend.createBooks(request);
  }

  public openBooks(request: OpenBooksRequest): Promise<ShellBooksStatus> {
    return this.backend.openBooks(request);
  }

  public verifyBooks(request: OpenBooksRequest): Promise<ShellVerifyStatus> {
    return this.backend.verifyBooks(request);
  }

  public trialBalance(request: OpenBooksRequest): Promise<TrialBalance> {
    return this.backend.trialBalance(request);
  }
}
