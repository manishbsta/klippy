import { createEffect, createSignal, onCleanup } from 'solid-js';
import { convertFileSrc } from '@tauri-apps/api/core';
import type { Clip } from '../lib/types';

const formatTimestamp = (iso: string) =>
  new Date(iso).toLocaleString([], {
    month: 'short',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  });

const contentClassByType = (type: Clip['contentType']) => {
  if (type === 'image') {
    return 'text-slate-700';
  }
  if (type === 'url') {
    return 'break-all whitespace-pre-line text-sky-700 underline decoration-sky-300/70 underline-offset-2';
  }
  if (type === 'code') {
    return 'break-words whitespace-pre-wrap font-mono text-[11px] leading-[1.3] text-slate-700';
  }
  return 'break-words whitespace-pre-line text-slate-800';
};

const imageMetaText = (clip: Clip) => {
  const parts: string[] = [];
  if (clip.mimeType) {
    parts.push(clip.mimeType.replace('image/', '').toUpperCase());
  }
  if (clip.pixelWidth && clip.pixelHeight) {
    parts.push(`${clip.pixelWidth}x${clip.pixelHeight}`);
  }
  if (typeof clip.byteSize === 'number' && clip.byteSize > 0) {
    parts.push(`${(clip.byteSize / (1024 * 1024)).toFixed(1)} MB`);
  }
  return parts.join(' | ');
};

const mediaSrc = (path?: string | null) => (path ? convertFileSrc(path) : null);

export const ClipRow = (props: {
  clip: Clip;
  selected: boolean;
  onSelect: () => void;
  onCopy: () => Promise<void> | void;
  onPin: () => Promise<void> | void;
  onDelete: () => Promise<void> | void;
}) => {
  const [copied, setCopied] = createSignal(false);
  const [imageFallbackStep, setImageFallbackStep] = createSignal(0);
  const hasThumb = () => Boolean(props.clip.thumbPath);
  const hasOriginal = () => Boolean(props.clip.mediaPath);
  const previewSrc = () => {
    const thumb = mediaSrc(props.clip.thumbPath);
    const original = mediaSrc(props.clip.mediaPath);
    if (imageFallbackStep() === 0) {
      return thumb ?? original;
    }
    if (imageFallbackStep() === 1) {
      return original;
    }
    return null;
  };
  let copiedTimer: number | undefined;

  createEffect(() => {
    props.clip.id;
    setImageFallbackStep(0);
  });

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
      data-testid={`clip-row-${props.clip.id}`}
      class={`group relative h-[74px] cursor-pointer overflow-hidden rounded-md border px-2 py-1.5 transition-colors duration-150 ${
        props.selected
          ? 'border-emerald-500/60 bg-white shadow-[0_4px_12px_rgba(15,23,42,0.08)]'
          : 'border-slate-300/80 bg-white hover:border-slate-400'
      }`}
      onClick={() => {
        void onCopyFromPreview();
      }}
      onMouseEnter={props.onSelect}
    >
      <div
        class={`pointer-events-none absolute right-8 top-1 rounded-full bg-emerald-600 px-1 py-0.5 text-[7px] font-semibold uppercase tracking-[0.06em] text-white transition-all duration-200 ${
          copied() ? 'translate-y-0 opacity-100' : '-translate-y-1 opacity-0'
        }`}
      >
        Copied
      </div>

      <div class="pointer-events-none absolute left-2 top-1 flex items-center gap-1 text-[7px] text-slate-500">
        {props.clip.pinned ? (
          <span aria-label="Pinned" class="rounded bg-amber-100 px-1 py-0.5 font-semibold uppercase tracking-[0.04em] text-amber-700">
            PINNED
          </span>
        ) : null}
        <span class="rounded bg-slate-100 px-1 py-0.5 font-mono text-[7px]">
          {formatTimestamp(props.clip.createdAt)}
        </span>
      </div>

      <div class="h-full pb-0 pt-4">
        {props.clip.contentType === 'image' ? (
          <div class="flex h-full items-center gap-2 pr-8">
            <div class="flex h-9 w-9 shrink-0 items-center justify-center overflow-hidden rounded border border-slate-200 bg-slate-100">
              {previewSrc() ? (
                <img
                  alt="Clipboard image preview"
                  class="h-full w-full object-cover"
                  src={previewSrc()!}
                  onError={() => {
                    const step = imageFallbackStep();
                    if (step === 0 && hasThumb() && hasOriginal()) {
                      setImageFallbackStep(1);
                      return;
                    }
                    setImageFallbackStep(2);
                  }}
                />
              ) : (
                <span class="font-mono text-[9px] uppercase text-slate-500">IMG</span>
              )}
            </div>
            <div class="min-w-0">
              <p
                data-testid={`clip-content-${props.clip.id}`}
                class="clip-two-lines block w-full text-left font-medium text-[11px] leading-[1.2] text-slate-800"
              >
                {props.clip.content}
              </p>
              <p class="truncate text-[9px] text-slate-500">{imageMetaText(props.clip)}</p>
            </div>
          </div>
        ) : (
          <p
            data-testid={`clip-content-${props.clip.id}`}
            class={`clip-two-lines block w-full pr-8 text-left text-[12px] leading-[1.25] transition-colors hover:text-black ${contentClassByType(props.clip.contentType)}`}
          >
            {props.clip.content}
          </p>
        )}
      </div>

      <div class="absolute right-1 top-1/2 flex -translate-y-1/2 flex-col items-center gap-0.5">
        <button
          aria-label={props.clip.pinned ? 'Unpin clip' : 'Pin clip'}
          class={`flex h-[18px] w-[18px] items-center justify-center rounded border bg-white shadow-sm transition ${
            props.clip.pinned
              ? 'border-amber-300 text-amber-700 hover:border-amber-500 hover:bg-amber-50'
              : 'border-slate-300 text-slate-700 hover:border-slate-500 hover:bg-slate-50'
          }`}
          onClick={(event) => {
            event.stopPropagation();
            void props.onPin();
          }}
          type="button"
        >
          <svg aria-hidden="true" class="h-2.5 w-2.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M8 3h8l-2 6v4l2 2v1H8v-1l2-2V9z"></path>
            <path d="M12 16v5"></path>
          </svg>
        </button>
        <button
          aria-label="Delete clip"
          class="flex h-[18px] w-[18px] items-center justify-center rounded border border-rose-300 bg-rose-50 text-rose-700 shadow-sm transition hover:border-rose-500 hover:bg-rose-100"
          onClick={(event) => {
            event.stopPropagation();
            void props.onDelete();
          }}
          type="button"
        >
          <svg aria-hidden="true" class="h-2.5 w-2.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M3 6h18"></path>
            <path d="M8 6V4h8v2"></path>
            <path d="M19 6l-1 14H6L5 6"></path>
          </svg>
        </button>
      </div>
    </article>
  );
};
