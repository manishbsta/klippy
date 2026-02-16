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
  <div class="grid flex-1 gap-2 overflow-y-auto bg-[linear-gradient(180deg,rgba(255,255,255,0.65),rgba(248,250,252,0.75))] p-3">
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
