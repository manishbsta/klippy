import { createSignal, onCleanup } from 'solid-js';
import type { Clip } from '../lib/types';

const formatTimestamp = (iso: string) =>
  new Date(iso).toLocaleString([], {
    month: 'short',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  });

const contentClassByType = (type: Clip['contentType']) => {
  if (type === 'url') {
    return 'break-all whitespace-pre-line text-sky-800 underline decoration-sky-300/70 underline-offset-4';
  }
  if (type === 'code') {
    return 'break-words whitespace-pre-wrap rounded-md bg-slate-100/70 px-2 py-1 font-mono text-[13px] leading-[1.5] text-slate-800';
  }
  return 'break-words whitespace-pre-line text-slate-800';
};

export const ClipRow = (props: {
  clip: Clip;
  selected: boolean;
  onSelect: () => void;
  onCopy: () => Promise<void> | void;
  onPin: () => Promise<void> | void;
  onDelete: () => Promise<void> | void;
}) => {
  const [copied, setCopied] = createSignal(false);
  let copiedTimer: number | undefined;

  onCleanup(() => {
    if (copiedTimer !== undefined) {
      window.clearTimeout(copiedTimer);
    }
  });

  const showCopiedFlash = () => {
    setCopied(true);
    if (copiedTimer !== undefined) {
      window.clearTimeout(copiedTimer);
    }
    copiedTimer = window.setTimeout(() => {
      setCopied(false);
    }, 850);
  };

  const onCopyFromPreview = async () => {
    await props.onCopy();
    showCopiedFlash();
  };

  return (
    <article
      class={`group relative h-[168px] cursor-pointer overflow-hidden rounded-xl border p-3 transition-all duration-200 ${
        props.selected
          ? 'border-emerald-500/60 bg-white shadow-[0_12px_28px_rgba(16,24,40,0.12)]'
          : 'border-slate-200/80 bg-white/75 hover:-translate-y-[1px] hover:border-slate-300 hover:bg-white hover:shadow-[0_8px_24px_rgba(16,24,40,0.10)]'
      }`}
      onClick={() => {
        void onCopyFromPreview();
      }}
      onMouseEnter={props.onSelect}
    >
      <div
        class={`pointer-events-none absolute right-3 top-3 rounded-full bg-emerald-600 px-2 py-1 text-[10px] font-semibold uppercase tracking-[0.12em] text-white transition-all duration-200 ${
          copied() ? 'translate-y-0 opacity-100' : '-translate-y-1 opacity-0'
        }`}
      >
        Copied
      </div>

      <div class="pointer-events-none absolute left-3 top-3 flex items-center gap-2 text-[9px] text-slate-500">
        {props.clip.pinned ? (
          <span aria-label="Pinned" class="rounded bg-amber-100 px-1.5 py-0.5 font-semibold uppercase tracking-[0.06em] text-amber-700">
            PINNED
          </span>
        ) : null}
        <span class="rounded bg-slate-100 px-1.5 py-0.5 font-mono text-[9px]">{formatTimestamp(props.clip.createdAt)}</span>
      </div>

      <div class="h-full pb-12 pt-8">
        <p
          class={`clip-five-lines block w-full pr-2 text-left text-[14px] leading-[1.5] transition-colors hover:text-black ${contentClassByType(props.clip.contentType)}`}
        >
          {props.clip.content}
        </p>
      </div>

      <div class="absolute bottom-3 left-3 right-3 rounded-lg border border-slate-200/80 bg-white/95 px-2 py-1 shadow-sm opacity-0 transition duration-150 group-hover:opacity-100">
        <div class="flex flex-wrap items-center gap-2 text-xs">
        <button
          class="rounded-md border border-slate-300 bg-white px-2.5 py-1 hover:border-slate-500"
          onClick={(event) => {
            event.stopPropagation();
            void onCopyFromPreview();
          }}
          type="button"
        >
          Copy
        </button>
        <button
          class="rounded-md border border-slate-300 bg-white px-2.5 py-1 hover:border-slate-500"
          onClick={(event) => {
            event.stopPropagation();
            void props.onPin();
          }}
          type="button"
        >
          {props.clip.pinned ? 'Unpin' : 'Pin'}
        </button>
        <button
          class="rounded-md border border-rose-300 bg-rose-50 px-2.5 py-1 text-rose-700 hover:border-rose-500"
          onClick={(event) => {
            event.stopPropagation();
            void props.onDelete();
          }}
          type="button"
        >
          Delete
        </button>
        </div>
      </div>
    </article>
  );
};
