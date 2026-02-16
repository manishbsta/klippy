import { Show } from 'solid-js';
import type { ClipStore } from './lib/store';
import { SearchBox } from './components/SearchBox';
import { ClipList } from './components/ClipList';
import { EmptyState } from './components/EmptyState';
import klippyIcon from './assets/klippy-icon.png';

export const App = (props: { store: ClipStore }) => {
  return (
    <main class="h-screen bg-[radial-gradient(circle_at_15%_0%,_#bae6fd,_transparent_45%),radial-gradient(circle_at_85%_100%,_#ddd6fe,_transparent_40%),linear-gradient(180deg,_#f8fafc_0%,_#eef2ff_100%)] p-3 font-sans text-slate-900">
      <section class="mx-auto flex h-[calc(100vh-1.5rem)] max-w-[620px] flex-col overflow-hidden rounded-2xl border border-white/70 bg-white/70 shadow-[0_24px_80px_rgba(15,23,42,0.22)] backdrop-blur-xl">
        <header class="border-b border-slate-200/80 bg-[linear-gradient(120deg,rgba(255,255,255,0.95),rgba(241,245,249,0.92))] px-4 py-3">
          <div class="mb-3 flex items-center justify-between gap-3">
            <div class="flex items-center gap-3">
              <div class="flex h-12 w-12 items-center justify-center rounded-xl border border-slate-200/70 bg-white shadow-[0_8px_18px_rgba(15,23,42,0.1)]">
                <img alt="Klippy icon" class="h-8 w-8 rounded-md" src={klippyIcon} />
              </div>
              <div>
              <p class="mt-0.5 text-[11px] text-slate-500">Clipboard archive for text, URLs, and code.</p>
              </div>
            </div>
            <div class="flex items-center gap-2">
              <button
                class="rounded-xl border border-amber-300 bg-amber-50 px-3 py-1.5 text-[13px] font-medium text-amber-700 transition hover:-translate-y-[1px] hover:border-amber-500"
                onClick={() => {
                  void props.store.clearAll();
                }}
                disabled={props.store.items().length === 0}
                type="button"
              >
                Clear All
              </button>
              <button
                class="rounded-xl border border-rose-300 bg-rose-50 px-3 py-1.5 text-[13px] font-medium text-rose-700 transition hover:-translate-y-[1px] hover:border-rose-500"
                onClick={() => {
                  void props.store.stop();
                }}
                type="button"
              >
                Stop
              </button>
            </div>
          </div>
          <SearchBox
            query={props.store.query()}
            onInput={(value) => props.store.setQuery(value)}
            onKeyDown={(event) => props.store.onKeyDown(event)}
          />
        </header>

        <Show
          when={!props.store.loading()}
          fallback={<div class="p-4 text-sm text-slate-500">Loading clips...</div>}
        >
          <Show
            when={props.store.items().length > 0}
            fallback={<EmptyState />}
          >
            <ClipList
              items={props.store.items()}
              selectedIndex={props.store.selectedIndex()}
              onSelect={(idx) => props.store.setSelectedIndex(idx)}
              onCopy={(id) => props.store.copy(id)}
              onPin={(id, pinned) => props.store.pin(id, pinned)}
              onDelete={(id) => props.store.remove(id)}
            />
          </Show>
        </Show>

        <footer class="mt-auto border-t border-slate-200/80 bg-white/60 px-4 py-2 text-[11px] text-slate-500">
          <span class="rounded bg-slate-100 px-1.5 py-0.5 font-mono text-[11px]">↑↓</span> navigate ·{' '}
          <span class="rounded bg-slate-100 px-1.5 py-0.5 font-mono text-[11px]">⌘⇧V</span> show/hide
        </footer>
      </section>
    </main>
  );
};
