import { reactive } from "vue";
import {
  createBooks,
  homeStatus,
  openBooks,
  verifyBooks,
  type BooksRef,
  type OwnerHomeStatus,
} from "./tauri";
import { errorMessage, slugBooksName } from "./format";

export interface BooksSessionState {
  booksName: string;
  businessName: string;
  context: BooksRef | null;
  isOpen: boolean;
  busy: boolean;
  message: string;
  error: string;
  home: OwnerHomeStatus | null;
  storageRootId: string | null;
  storageRootLabel: string;
}

export interface BooksSession {
  state: BooksSessionState;
  create(): Promise<void>;
  open(): Promise<void>;
  verify(): Promise<void>;
  refreshHome(): Promise<void>;
  requireContext(): BooksRef;
  setStorageRoot(rootId: string, label: string): void;
}

function deriveContext(booksName: string): BooksRef {
  const slug = slugBooksName(booksName);
  return { fileName: `${slug}.sqlite`, booksId: slug, actor: "owner" };
}

export function createBooksSession(): BooksSession {
  const state = reactive<BooksSessionState>({
    booksName: "My books",
    businessName: "My business",
    context: null,
    isOpen: false,
    busy: false,
    message: "Choose Create or Open to start.",
    error: "",
    home: null,
    storageRootId: null,
    storageRootLabel: "No document storage folder selected for this session.",
  });

  function requireContext(): BooksRef {
    if (!state.context || !state.isOpen) throw new Error("Open your books before using this feature.");
    return state.context;
  }

  function clearStorageRoot(): void {
    state.storageRootId = null;
    state.storageRootLabel = "No document storage folder selected for this session.";
  }

  function setStorageRoot(rootId: string, label: string): void {
    state.storageRootId = rootId;
    state.storageRootLabel = label || "Document storage folder selected for this session.";
  }

  async function refreshHome(): Promise<void> {
    state.home = await homeStatus(requireContext());
  }

  async function create(): Promise<void> {
    state.busy = true;
    state.error = "";
    try {
      const context = deriveContext(state.booksName);
      const companyName = state.businessName.trim();
      if (!companyName) throw new Error("Enter your business name.");
      await createBooks({ ...context, companyName });
      state.context = context;
      state.isOpen = true;
      clearStorageRoot();
      await refreshHome();
      state.message = "Books created and opened.";
    } catch (error) {
      state.error = errorMessage(error);
      state.isOpen = false;
      state.context = null;
      state.home = null;
      clearStorageRoot();
    } finally { state.busy = false; }
  }

  async function open(): Promise<void> {
    state.busy = true;
    state.error = "";
    try {
      const context = deriveContext(state.booksName);
      await openBooks(context);
      state.context = context;
      state.isOpen = true;
      clearStorageRoot();
      await refreshHome();
      state.message = "Books opened.";
    } catch (error) {
      state.error = errorMessage(error);
      state.isOpen = false;
      state.context = null;
      state.home = null;
      clearStorageRoot();
    } finally { state.busy = false; }
  }

  async function verify(): Promise<void> {
    state.busy = true;
    state.error = "";
    try {
      await verifyBooks(requireContext());
      await refreshHome();
      state.message = "Books check completed.";
    } catch (error) { state.error = errorMessage(error); }
    finally { state.busy = false; }
  }

  return { state, create, open, verify, refreshHome, requireContext, setStorageRoot };
}
