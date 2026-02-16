import { render, screen } from '@solidjs/testing-library';
import { describe, expect, it } from 'vitest';
import { EmptyState } from './EmptyState';

describe('EmptyState', () => {
  it('shows guidance text', () => {
    render(() => <EmptyState />);
    expect(screen.getByText(/Copy text, URLs, or code/i)).toBeInTheDocument();
  });
});
