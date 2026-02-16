import { render, screen, fireEvent } from '@solidjs/testing-library';
import { describe, expect, it, vi } from 'vitest';
import { ClipList } from './ClipList';

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

    fireEvent.click(screen.getByText('Copy'));
    fireEvent.click(screen.getByText('Pin'));
    fireEvent.click(screen.getByText('Delete'));

    expect(onCopy).toHaveBeenCalledWith(10);
    expect(onPin).toHaveBeenCalledWith(10, true);
    expect(onDelete).toHaveBeenCalledWith(10);
  });
});
