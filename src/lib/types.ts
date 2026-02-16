export type ContentType = 'text' | 'url' | 'code';

export interface Clip {
  id: number;
  content: string;
  contentType: ContentType;
  pinned: boolean;
  createdAt: string;
}

export interface Settings {
  historyLimit: number;
  trackingPaused: boolean;
  maxClipBytes: number;
  restoreClipboardAfterPaste: boolean;
  denylistBundleIds: string[];
}

export interface SettingsPatch {
  historyLimit?: number;
  trackingPaused?: boolean;
  maxClipBytes?: number;
  restoreClipboardAfterPaste?: boolean;
  denylistBundleIds?: string[];
}

export interface ClipPage {
  items: Clip[];
  total: number;
  nextOffset: number | null;
}
