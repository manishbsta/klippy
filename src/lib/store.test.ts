import { describe, expect, it, vi, beforeEach } from 'vitest';
import { useClipStore } from './store';

const mocks = vi.hoisted(() => {
  return {
    listClips: vi.fn(),
    copyClip: vi.fn(),
    setPinned: vi.fn(),
    deleteClip: vi.fn(),
    clearAllClips: vi.fn(),
    getSettings: vi.fn(),
    updateSettings: vi.fn(),
    setTrackingPaused: vi.fn(),
    stopApp: vi.fn(),
  };
});

vi.mock('./api', () => ({
  listClips: mocks.listClips,
  copyClip: mocks.copyClip,
  setPinned: mocks.setPinned,
  deleteClip: mocks.deleteClip,
  clearAllClips: mocks.clearAllClips,
  getSettings: mocks.getSettings,
  updateSettings: mocks.updateSettings,
  setTrackingPaused: mocks.setTrackingPaused,
  stopApp: mocks.stopApp,
}));

describe('useClipStore', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.getSettings.mockResolvedValue({
      historyLimit: 200,
      trackingPaused: false,
      maxClipBytes: 262144,
      restoreClipboardAfterPaste: true,
      denylistBundleIds: [],
    });
    mocks.clearAllClips.mockResolvedValue(2);
    mocks.listClips.mockResolvedValue({
      items: [
        {
          id: 1,
          content: 'alpha',
          contentType: 'text',
          pinned: false,
          createdAt: new Date().toISOString(),
        },
        {
          id: 2,
          content: 'beta',
          contentType: 'text',
          pinned: false,
          createdAt: new Date().toISOString(),
        },
      ],
      total: 2,
      nextOffset: null,
    });
  });

  it('loads settings and clips', async () => {
    const store = useClipStore();
    await store.init();

    expect(mocks.getSettings).toHaveBeenCalledTimes(1);
    expect(mocks.listClips).toHaveBeenCalledTimes(1);
    expect(store.items().length).toBe(2);
  });

  it('copies selected item on Enter key', async () => {
    const store = useClipStore();
    await store.init();

    const event = new KeyboardEvent('keydown', { key: 'Enter' });
    await store.onKeyDown(event);

    expect(mocks.copyClip).toHaveBeenCalledWith(1);
  });

  it('navigates with arrow keys', async () => {
    const store = useClipStore();
    await store.init();

    await store.onKeyDown(new KeyboardEvent('keydown', { key: 'ArrowDown' }));
    expect(store.selectedIndex()).toBe(1);

    await store.onKeyDown(new KeyboardEvent('keydown', { key: 'ArrowUp' }));
    expect(store.selectedIndex()).toBe(0);
  });

  it('does not trigger delete action when typing in search', async () => {
    const store = useClipStore();
    await store.init();

    await store.onKeyDown(new KeyboardEvent('keydown', { key: 'p' }));
    await store.onKeyDown(new KeyboardEvent('keydown', { key: 'Backspace' }));

    expect(mocks.deleteClip).not.toHaveBeenCalled();
  });

  it('clears all clips and reloads list', async () => {
    const store = useClipStore();
    await store.init();
    await store.clearAll();

    expect(mocks.clearAllClips).toHaveBeenCalledTimes(1);
    expect(mocks.listClips).toHaveBeenCalledTimes(2);
  });
});
