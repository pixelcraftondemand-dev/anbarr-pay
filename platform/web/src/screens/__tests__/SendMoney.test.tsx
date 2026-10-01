import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router-dom';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { TransferResponse } from '../../api/types';
import { SendMoney } from '../SendMoney';

// Mock the API client (network) — the UI's contract is the client module.
const apiRequestMock = vi.fn();
vi.mock('../../api/client', async () => {
  const actual = await vi.importActual<typeof import('../../api/client')>('../../api/client');
  return {
    ...actual,
    apiRequest: (...args: unknown[]) => apiRequestMock(...args),
  };
});

import { ApiError } from '../../api/client';

function renderSend() {
  return render(
    <MemoryRouter initialEntries={['/send']}>
      <SendMoney />
    </MemoryRouter>,
  );
}

async function fillValidRecipient(user: ReturnType<typeof userEvent.setup>) {
  await user.type(screen.getByLabelText(/recipient phone or email/i), 'aminata@example.com');
  await user.click(screen.getByRole('button', { name: 'Continue' }));
}

beforeEach(() => {
  apiRequestMock.mockReset();
  // Path-aware responses: PIN verify then the transfer POST.
  apiRequestMock.mockImplementation((path: string, opts?: { body?: { pin?: string } }) => {
    if (path === '/auth/pin/verify') {
      return Promise.resolve({ pin_token: `pt-${opts?.body?.pin ?? ''}` });
    }
    return Promise.resolve({});
  });
});

describe('SendMoney — recipient step', () => {
  it('disables Continue until a valid phone or email is entered', async () => {
    const user = userEvent.setup();
    renderSend();
    const cont = screen.getByRole('button', { name: 'Continue' });
    expect(cont).toBeDisabled();

    await user.type(screen.getByLabelText(/recipient/i), 'not-a-phone');
    expect(cont).toBeDisabled();

    await user.clear(screen.getByLabelText(/recipient/i));
    await user.type(screen.getByLabelText(/recipient/i), '+23276000000');
    expect(cont).toBeEnabled();
  });
});

describe('SendMoney — amount step', () => {
  it('builds the amount from keypad digits and enables review when > 0', async () => {
    const user = userEvent.setup();
    renderSend();
    await fillValidRecipient(user);

    const amountDisplay = document.querySelector('.amount-display .num');
    expect(amountDisplay).toHaveTextContent('0');
    await user.click(screen.getByRole('button', { name: '4' }));
    await user.click(screen.getByRole('button', { name: '5' }));
    await user.click(screen.getByRole('button', { name: '0' }));
    await user.click(screen.getByRole('button', { name: '0' }));

    const review = screen.getByRole('button', { name: /review transfer/i });
    expect(review).toBeEnabled();
    // 4,5,0,0 → 4500 minor units → "45" in the display (minor ÷ 100).
    expect(document.querySelector('.amount-display .num')).toHaveTextContent('45');
    // §13.6: fee is never invented client-side.
    expect(screen.getByText(/confirmed by amberpay/i)).toBeInTheDocument();

    // Backspace removes a digit.
    await user.click(screen.getByRole('button', { name: 'Delete' }));
    expect(review).toBeEnabled();
  });

  it('keeps Review disabled at zero', async () => {
    const user = userEvent.setup();
    renderSend();
    await fillValidRecipient(user);
    expect(screen.getByRole('button', { name: /review transfer/i })).toBeDisabled();
  });
});

describe('SendMoney — review, PIN, and submission', () => {
  it('blocks submission until 4 PIN digits are entered, then posts once with the full body', async () => {
    const user = userEvent.setup();
    apiRequestMock.mockImplementation((path: string) => {
      if (path === '/auth/pin/verify') return Promise.resolve({ pin_token: 'pt' });
      if (path === '/transfers') {
        return Promise.resolve({
          id: 'tx-9',
          status: 'COMPLETED',
          total_minor: 452_250,
        } satisfies TransferResponse);
      }
      return Promise.resolve({});
    });
    renderSend();
    await fillValidRecipient(user);

    await user.click(screen.getByRole('button', { name: '4' }));
    await user.click(screen.getByRole('button', { name: '5' }));
    await user.click(screen.getByRole('button', { name: '0' }));
    await user.click(screen.getByRole('button', { name: '0' }));
    await user.click(screen.getByRole('button', { name: /review transfer/i }));

    const confirm = screen.getByRole('button', { name: /confirm transfer/i });
    expect(confirm).toBeDisabled();
    expect(screen.getByLabelText('PIN entry 0 of 4')).toBeInTheDocument();

    for (const d of ['1', '2', '3', '4']) {
      await user.click(screen.getByRole('button', { name: d }));
    }
    expect(confirm).toBeEnabled();
    await user.click(confirm);

    await waitFor(() => {
      expect(screen.getByText(/transfer confirmed/i)).toBeInTheDocument();
    });
    // One PIN verify + one transfer POST (path-scoped: unrelated reads like
    // the beneficiary picker may also fire and don't affect this assertion).
    const transferCalls = apiRequestMock.mock.calls.filter(([p]) => p === '/transfers');
    expect(transferCalls).toHaveLength(1);
    const init = transferCalls[0][1];
    expect(init.body.recipient_email_or_phone).toBe('aminata@example.com');
    // Keypad typed 4,5,0,0 → 4500 minor units (SLE 45.00). The client sends
    // minor units verbatim — no fee or conversion math happens client-side.
    expect(init.body.amount_minor).toBe(4500);
    expect(init.idempotencyKey).toEqual(expect.any(String));
  });

  it('shows the honest pending state on network loss and never claims failure', async () => {
    const user = userEvent.setup();
    apiRequestMock.mockImplementation((path: string) => {
      if (path === '/auth/pin/verify') return Promise.resolve({ pin_token: 'pt' });
      // Network loss on the transfer POST itself: outcome UNKNOWN.
      if (path === '/transfers') return Promise.reject(new ApiError(0, 'network_error', 'unreachable'));
      return Promise.resolve({});
    });
    renderSend();
    await fillValidRecipient(user);

    await user.click(screen.getByRole('button', { name: '4' }));
    await user.click(screen.getByRole('button', { name: '5' }));
    await user.click(screen.getByRole('button', { name: '0' }));
    await user.click(screen.getByRole('button', { name: '0' }));
    await user.click(screen.getByRole('button', { name: /review transfer/i }));
    for (const d of ['1', '2', '3', '4']) {
      await user.click(screen.getByRole('button', { name: d }));
    }
    await user.click(screen.getByRole('button', { name: /confirm transfer/i }));

    await waitFor(() => {
      expect(screen.getByText(/transfer pending/i)).toBeInTheDocument();
    });
    expect(screen.getByText('PENDING')).toBeInTheDocument();
    expect(screen.queryByText(/couldn't be completed/i)).not.toBeInTheDocument();
  });

  it('renders a definitive rejection with the server message', async () => {
    const user = userEvent.setup();
    apiRequestMock.mockImplementation((path: string) => {
      if (path === '/auth/pin/verify') return Promise.resolve({ pin_token: 'pt' });
      if (path === '/transfers') {
        return Promise.reject(
          new ApiError(422, 'insufficient_funds', 'Not enough available balance.'),
        );
      }
      return Promise.resolve({});
    });
    renderSend();
    await fillValidRecipient(user);

    await user.click(screen.getByRole('button', { name: '4' }));
    await user.click(screen.getByRole('button', { name: '5' }));
    await user.click(screen.getByRole('button', { name: '0' }));
    await user.click(screen.getByRole('button', { name: '0' }));
    await user.click(screen.getByRole('button', { name: /review transfer/i }));
    for (const d of ['1', '2', '3', '4']) {
      await user.click(screen.getByRole('button', { name: d }));
    }
    await user.click(screen.getByRole('button', { name: /confirm transfer/i }));

    await waitFor(() => {
      expect(
        screen.getByText(/transfer couldn't be completed/i),
      ).toBeInTheDocument();
    });
    expect(screen.getByText(/not enough available balance/i)).toBeInTheDocument();
  });
});
