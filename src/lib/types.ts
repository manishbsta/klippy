export type ContentType = 'text' | 'url' | 'code' | 'image';

export interface Clip {
  id: number;
  content: string;
  contentType: ContentType;
  pinned: boolean;
  createdAt: string;
  mediaPath?: string | null;
  thumbPath?: string | null;
  mimeType?: string | null;
  byteSize?: number;
  pixelWidth?: number | null;
  pixelHeight?: number | null;
}

export interface ClipPage {
  items: Clip[];
  total: number;
  nextOffset: number | null;
}
