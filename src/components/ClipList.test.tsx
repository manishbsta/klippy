import { render, screen, fireEvent } from '@solidjs/testing-library';
import { describe, expect, it, vi } from 'vitest';
import { ClipList } from './ClipList';

vi.mock('@tauri-apps/api/core', () => ({
  convertFileSrc: (filePath: string) => `asset://${filePath}`,
}));

describe('ClipList', () => {
  it('renders clip rows and runs action handlers', () => {
    const onCopy = vi.fn();
    const onPin = vi.fn();
    const onDelete = vi.fn();
    const onSelect = vi.fn();

    render(() => (
      <ClipList
        items={[
          {
            id: 10,
            content: 'hello world',
            contentType: 'text',
            pinned: false,
            createdAt: new Date().toISOString(),
          },
        ]}
        selectedIndex={0}
        onSelect={onSelect}
        onCopy={onCopy}
        onPin={onPin}
        onDelete={onDelete}
      />
    ));

    expect(screen.getByText('hello world')).toBeInTheDocument();
    const row = screen.getByTestId('clip-row-10');
    const content = screen.getByTestId('clip-content-10');
    expect(row.className).toContain('h-[74px]');
    expect(content.className).toContain('clip-two-lines');

    fireEvent.click(row);
    expect(onCopy).toHaveBeenCalledWith(10);

    fireEvent.click(screen.getByRole('button', { name: 'Pin clip' }));
    fireEvent.click(screen.getByRole('button', { name: 'Delete clip' }));

    expect(onCopy).toHaveBeenCalledTimes(1);
    expect(onPin).toHaveBeenCalledWith(10, true);
    expect(onDelete).toHaveBeenCalledWith(10);
  });

  it('renders image metadata row', () => {
    render(() => (
      <ClipList
        items={[
          {
            id: 42,
            content: 'Image | PNG | 640x480 | 0.4 MB',
            contentType: 'image',
            pinned: false,
            createdAt: new Date().toISOString(),
            mimeType: 'image/png',
            byteSize: 400000,
            pixelWidth: 640,
            pixelHeight: 480,
          },
        ]}
        selectedIndex={0}
        onSelect={() => undefined}
        onCopy={() => undefined}
        onPin={() => undefined}
        onDelete={() => undefined}
      />
    ));

    expect(screen.getByText(/Image \| PNG \| 640x480/i)).toBeInTheDocument();
    expect(screen.getAllByText(/PNG \| 640x480 \| 0.4 MB/i).length).toBeGreaterThan(0);
  });

  it('falls back from thumbnail to original image source on error', () => {
    render(() => (
      <ClipList
        items={[
          {
            id: 43,
            content: 'Image | PNG | 20x20 | 0.1 MB',
            contentType: 'image',
            pinned: false,
            createdAt: new Date().toISOString(),
            mimeType: 'image/png',
            byteSize: 100000,
            pixelWidth: 20,
            pixelHeight: 20,
            thumbPath: '/tmp/thumb.png',
            mediaPath: '/tmp/original.png',
          },
        ]}
        selectedIndex={0}
        onSelect={() => undefined}
        onCopy={() => undefined}
        onPin={() => undefined}
        onDelete={() => undefined}
      />
    ));

    const preview = screen.getByAltText('Clipboard image preview') as HTMLImageElement;
    expect(preview.getAttribute('src')).toContain('thumb.png');

    fireEvent.error(preview);
    expect(preview.getAttribute('src')).toContain('original.png');
  });

  it('falls back to IMG placeholder after preview sources fail', () => {
    render(() => (
      <ClipList
        items={[
          {
            id: 44,
            content: 'Image | PNG | 20x20 | 0.1 MB',
            contentType: 'image',
            pinned: false,
            createdAt: new Date().toISOString(),
            mimeType: 'image/png',
            byteSize: 100000,
            pixelWidth: 20,
            pixelHeight: 20,
            thumbPath: '/tmp/thumb-only.png',
          },
        ]}
        selectedIndex={0}
        onSelect={() => undefined}
        onCopy={() => undefined}
        onPin={() => undefined}
        onDelete={() => undefined}
      />
    ));

    const preview = screen.getByAltText('Clipboard image preview');
    fireEvent.error(preview);
    expect(screen.getByText('IMG')).toBeInTheDocument();
  });
});
