export const EmptyState = (props: { paused: boolean; onResume: () => void }) => (
  <div class="flex flex-1 flex-col items-center justify-center gap-3 bg-[radial-gradient(circle_at_top,_rgba(186,230,253,0.35),_transparent_45%)] p-6 text-center">
    <h2 class="text-lg font-semibold text-slate-900">No clips yet</h2>
    {props.paused ? (
      <>
        <p class="max-w-[340px] text-sm leading-6 text-slate-500">
          Tracking is paused. Resume collection to capture new clipboard items.
        </p>
        <button
          class="rounded-xl border border-cyan-500 bg-cyan-50 px-3 py-2 text-sm font-medium text-cyan-700 transition hover:-translate-y-[1px] hover:bg-cyan-100"
          onClick={props.onResume}
          type="button"
        >
          Resume Tracking
        </button>
      </>
    ) : (
      <p class="max-w-[340px] text-sm leading-6 text-slate-500">
        Copy text, URLs, or code in any app and it will appear here instantly.
      </p>
    )}
  </div>
);
