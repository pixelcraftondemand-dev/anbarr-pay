export type ScreenId =
  | 'wallet'
  | 'services'
  | 'cards'
  | 'send-step-1'
  | 'send-step-2'
  | 'send-step-3'
  | 'transaction-details';

export interface TransferData {
  recipientName: string;
  recipientHandle: string;
  recipientPhone: string;
  recipientAccount: string;
  recipientAvatar?: string;
  amount: string;
  note: string;
  sourceAccount: string;
  sourceBalance: string;
  payoutRail: string;
}
