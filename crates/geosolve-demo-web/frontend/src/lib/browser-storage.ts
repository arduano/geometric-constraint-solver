// SPDX-License-Identifier: GPL-3.0-or-later

export interface BrowserStorageResult<T> {
  value: T;
  issue: string | null;
}

export function readBrowserStorage(key: string, label: string): BrowserStorageResult<string | null> {
  try {
    return { value: globalThis.localStorage.getItem(key), issue: null };
  } catch (error) {
    return { value: null, issue: browserStorageIssue("read", label, error) };
  }
}

export function writeBrowserStorage(key: string, value: string, label: string): BrowserStorageResult<boolean> {
  try {
    globalThis.localStorage.setItem(key, value);
    return { value: true, issue: null };
  } catch (error) {
    return { value: false, issue: browserStorageIssue("write", label, error) };
  }
}

export function removeBrowserStorage(key: string, label: string): BrowserStorageResult<boolean> {
  try {
    globalThis.localStorage.removeItem(key);
    return { value: true, issue: null };
  } catch (error) {
    return { value: false, issue: browserStorageIssue("remove", label, error) };
  }
}

export function browserStorageIssue(operation: "read" | "write" | "remove", label: string, error: unknown) {
  const detail = error instanceof DOMException && error.name
    ? `${error.name}${error.message ? `: ${error.message}` : ""}`
    : error instanceof Error ? error.message : String(error);
  return `Browser storage could not ${operation} ${label}: ${detail}`;
}
