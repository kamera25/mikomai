import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { TimelineEvent } from '../TimelineEvent';
import { Message } from '../../../types';

describe('TimelineEvent Component', () => {
  const formatMessageTime = (_isoString?: string) => '12:00';

  it('renders standard user message', () => {
    const msg: Message = {
      role: 'user',
      content: 'Standard message',
      timestamp: new Date().toISOString(),
    };

    render(<TimelineEvent msg={msg} formatMessageTime={formatMessageTime} />);
    expect(screen.getByText('Standard message')).toBeInTheDocument();
  });

  it('renders tool execution block', () => {
    const msg: Message = {
      role: 'ai',
      content: 'Tool running...',
      timestamp: new Date().toISOString(),
      event_type: 'ToolExecution',
      tool_id: 'network_ping',
      action_name: 'Ping Test',
      summary_text: 'Pinged 8.8.8.8 successfully',
      status: 'Success',
      raw_data: 'ping ok',
    };

    render(<TimelineEvent msg={msg} formatMessageTime={formatMessageTime} />);
    expect(screen.getByText('Ping Test')).toBeInTheDocument();
    expect(screen.getByText('Pinged 8.8.8.8 successfully')).toBeInTheDocument();
  });
});
