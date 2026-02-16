import { createSignal } from 'solid-js';
import {
  clearAllClips,
  copyClip,
  deleteClip,
  listClips,
  setPinned,
  stopApp,
} from './api';
import type { Clip } from './types';

const PAGE_SIZE = 100;

export interface ClipStore {
  items: () => Clip[];
  loading: () => boolean;
  query: () => string;
  selectedIndex: () => number;
  init: () => Promise<void>;
  reload: () => Promise<void>;
  setQuery: (value: string) => void;
  setSelectedIndex: (value: number) => void;
  onKeyDown: (event: KeyboardEvent) => Promise<void>;
  copy: (id: number) => Promise<void>;
  pin: (id: number, pinned: boolean) => Promise<void>;
  remove: (id: number) => Promise<void>;
  clearAll: () => Promise<void>;
  stop: () => Promise<void>;
}

export const useClipStore = (): ClipStore => {
  const [items, setItems] = createSignal<Clip[]>([]);
  const [loading, setLoading] = createSignal(true);
  const [query, setQuery] = createSignal('');
  const [selectedIndex, setSelectedIndex] = createSignal(0);
  let debounceTimer: number | undefined;

  const reload = async () => {
    setLoading(true);
    try {
      const page = await listClips(query().trim() === '' ? null : query(), PAGE_SIZE, 0);
      setItems(page.items);
      const currentSelected = selectedIndex();
      if (page.items.length === 0) {
        setSelectedIndex(0);
      } else if (currentSelected >= page.items.length) {
        setSelectedIndex(page.items.length - 1);
      }
    } finally {
      setLoading(false);
    }
  };

  const init = async () => {
    await reload();
  };

  const copy = async (id: number) => {
    await copyClip(id);
  };

  const pin = async (id: number, pinned: boolean) => {
    await setPinned(id, pinned);
    await reload();
  };

  const remove = async (id: number) => {
    await deleteClip(id);
    await reload();
  };

  const clearAll = async () => {
    await clearAllClips();
    await reload();
  };

  const stop = async () => {
    await stopApp();
  };

  const setQueryDebounced = (value: string) => {
    setQuery(value);
    if (debounceTimer !== undefined) {
      window.clearTimeout(debounceTimer);
    }
    debounceTimer = window.setTimeout(() => {
      void reload().catch(() => undefined);
    }, 90);
  };

  const onKeyDown = async (event: KeyboardEvent) => {
    if (event.defaultPrevented || event.isComposing) {
      return;
    }

    const currentItems = items();
    if (currentItems.length === 0) {
      return;
    }

    if (event.key === 'ArrowDown') {
      event.preventDefault();
      setSelectedIndex((idx) => Math.min(idx + 1, currentItems.length - 1));
      return;
    }
    if (event.key === 'ArrowUp') {
      event.preventDefault();
      setSelectedIndex((idx) => Math.max(idx - 1, 0));
      return;
    }

    // Enter-to-copy disabled by product requirement.
  };

  return {
    items,
    loading,
    query,
    selectedIndex,
    init,
    reload,
    setQuery: setQueryDebounced,
    setSelectedIndex,
    onKeyDown,
    copy,
    pin,
    remove,
    clearAll,
    stop,
  };
};
