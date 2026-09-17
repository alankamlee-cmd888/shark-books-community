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
}

export interface BooksSession {
  state: BooksSessionState;
  create(): Promise<void>;
  open(): Promise<void>;
  verify(): Promise<void>;
  refreshHome(): Promise<void>;
  requireContext(): BooksRef;
}

function deriveContext(booksName: string): BooksRef {
  const slug = slugBooksName(booksName);
  return {
    fileName: `${slug}.sqlite`,
    booksId: slug,
    actor: "owner",
  };
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
  });

  function requireContext(): BooksRef {
    if (!state.context || !state.isOpen) {
      throw new Error("Open your books before using this feature.");
    }
    return state.context;
  }

  async function refreshHome(): Promise<void> {
    const books = requireContext();
    state.home = await homeStatus(books);
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
      await refreshHome();
      state.message = "Books created and opened.";
    } catch (error) {
      state.error = errorMessage(error);
      state.isOpen = false;
      state.context = null;
      state.home = null;
    } finally {
      state.busy = false;
    }
  }

  async function open(): Promise<void> {
    state.busy = true;
    state.error = "";
    try {
      const context = deriveContext(state.booksName);
      await openBooks(context);
      state.context = context;
      state.isOpen = true;
      await refreshHome();
      state.message = "Books opened.";
    } catch (error) {
      state.error = errorMessage(error);
      state.isOpen = false;
      state.context = null;
      state.home = null;
    } finally {
      state.busy = false;
    }
  }

  async function verify(): Promise<void> {
    state.busy = true;
    state.error = "";
    try {
      await verifyBooks(requireContext());
      await refreshHome();
      state.message = "Books check completed.";
    } catch (error) {
      state.error = errorMessage(error);
    } finally {
      state.busy = false;
    }
  }

  return { state, create, open, verify, refreshHome, requireContext };
}
