export const SearchBox = (props: {
  query: string;
  onInput: (value: string) => void;
  onKeyDown: (event: KeyboardEvent) => void | Promise<void>;
}) => (
  <div class="relative">
    <input
      autofocus
      class="w-full rounded-xl border border-slate-300/70 bg-white px-4 py-2.5 pr-10 text-[16px] font-medium text-slate-900 outline-none transition placeholder:font-normal placeholder:text-slate-400 focus:border-cyan-500 focus:shadow-[0_0_0_4px_rgba(14,165,233,0.15)]"
      placeholder="Search clips..."
      value={props.query}
      onInput={(event) => props.onInput(event.currentTarget.value)}
      onKeyDown={(event) => {
        void props.onKeyDown(event);
      }}
    />
    <span class="pointer-events-none absolute right-3 top-1/2 -translate-y-1/2 rounded-md bg-slate-100 px-1.5 py-0.5 font-mono text-[11px] text-slate-500">
      /
    </span>
  </div>
);
