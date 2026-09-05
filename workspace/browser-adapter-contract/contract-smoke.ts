import {
  BrowserBooksAdapter,
  TauriBooksAdapter,
  type BooksApplicationAdapter,
  type BrowserBooksBackend,
  type InvokeFn,
} from "./adapter";

declare const nativeInvoke: InvokeFn;
declare const browserBackend: BrowserBooksBackend;

const nativeAdapter: BooksApplicationAdapter = new TauriBooksAdapter(nativeInvoke);
const browserAdapter: BooksApplicationAdapter = new BrowserBooksAdapter(browserBackend);

void nativeAdapter;
void browserAdapter;
