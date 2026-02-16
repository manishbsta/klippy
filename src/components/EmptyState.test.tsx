import { render, screen, fireEvent } from '@solidjs/testing-library';
import { describe, expect, it, vi } from 'vitest';
import { EmptyState } from './EmptyState';

describe('EmptyState', () => {
  it('shows paused controls when tracking paused', () => {
    const onResume = vi.fn();
    render(() => <EmptyState paused={true} onResume={onResume} />);

    expect(screen.getByText('Resume Tracking')).toBeInTheDocument();
    fireEvent.click(screen.getByText('Resume Tracking'));
    expect(onResume).toHaveBeenCalledTimes(1);
  });

  it('shows guidance when active', () => {
    render(() => <EmptyState paused={false} onResume={() => undefined} />);
    expect(screen.getByText(/Copy text, URLs, or code/i)).toBeInTheDocument();
  });
});
