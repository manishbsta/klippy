import { For } from 'solid-js';
import type { Clip } from '../lib/types';
import { ClipRow } from './ClipRow';

export const ClipList = (props: {
  items: Clip[];
  selectedIndex: number;
  onSelect: (index: number) => void;
  onCopy: (id: number) => Promise<void> | void;
  onPin: (id: number, pinned: boolean) => Promise<void> | void;
  onDelete: (id: number) => Promise<void> | void;
}) => (
  <div class="flex flex-1 flex-col gap-1 overflow-y-auto bg-slate-100/60 p-1.5">
    <For each={props.items}>
      {(clip, idx) => (
        <ClipRow
          clip={clip}
          selected={idx() === props.selectedIndex}
          onSelect={() => props.onSelect(idx())}
          onCopy={() => props.onCopy(clip.id)}
          onPin={() => props.onPin(clip.id, !clip.pinned)}
          onDelete={() => props.onDelete(clip.id)}
        />
      )}
    </For>
  </div>
);
