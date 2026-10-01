import { Navigate, Route, Routes } from 'react-router-dom';
import { Layout } from './components/Layout';
import { Home } from './screens/Home';
import { Activity } from './screens/Activity';
import { SendMoney } from './screens/SendMoney';
import { Topup } from './screens/Topup';
import { Security } from './screens/Security';
import { TransactionDetail } from './screens/TransactionDetail';
import { Vault } from './screens/Vault';
import { WalletDetail } from './screens/WalletDetail';
import { Wallets } from './screens/Wallets';
import { Beneficiaries } from './screens/Beneficiaries';
import { Placeholder } from './screens/Placeholder';

/**
 * No authentication layer right now: sign-in was removed with Supabase Auth
 * and nothing protects these routes yet (see docs/authentication.md for the
 * design that will replace it). Do NOT deploy this configuration anywhere
 * public — there is no login, no session, and no user identity.
 */
export function App() {
  return (
    <Routes>
      <Route element={<Layout />}>
        <Route path="/" element={<Navigate to="/home" replace />} />
        <Route path="/home" element={<Home />} />
        <Route path="/activity" element={<Activity />} />
        <Route path="/send" element={<SendMoney />} />
        <Route path="/topup" element={<Topup />} />
        <Route path="/vault" element={<Vault />} />
        <Route path="/security" element={<Security />} />
        <Route path="/wallets" element={<Wallets />} />
        <Route path="/wallets/:id" element={<WalletDetail />} />

        {/* Spec'd journeys not yet built (docs/ux-flows.md §1) — honest
            placeholders, never fake functionality (§73). */}
        <Route path="/kyc" element={<Placeholder title="Verify your identity" />} />
        <Route path="/withdraw" element={<Placeholder title="Withdraw" />} />
        <Route path="/beneficiaries" element={<Beneficiaries />} />
        <Route path="/merchant" element={<Placeholder title="Merchant dashboard" />} />
        <Route path="/checkout/:id" element={<Placeholder title="Checkout" />} />
        <Route path="/pay/:linkId" element={<Placeholder title="Payment link" />} />
        <Route path="/transactions" element={<Navigate to="/activity" replace />} />
        <Route path="/transactions/:id" element={<TransactionDetail />} />
        <Route path="/disputes" element={<Placeholder title="Disputes" />} />
        <Route path="/settings" element={<Navigate to="/security" replace />} />
        <Route path="/settings/security" element={<Navigate to="/security" replace />} />
        <Route path="/settings/security/devices" element={<Placeholder title="Devices" />} />
        <Route path="/settings/security/sessions" element={<Placeholder title="Sessions" />} />
        <Route path="/settings/api-keys" element={<Placeholder title="API keys" />} />
        <Route path="/business" element={<Placeholder title="Business accounts" />} />
        <Route path="/admin" element={<Placeholder title="Admin console" />} />
        <Route path="/support" element={<Placeholder title="Support" />} />
      </Route>

      <Route path="*" element={<Navigate to="/home" replace />} />
    </Routes>
  );
}
