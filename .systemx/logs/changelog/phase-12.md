# Phase 12 — Web Wallet Security & UX

**Date:** 2026-03-25
**Branch:** claude/interesting-rhodes

---

## Summary

Comprehensive security hardening and UX improvements to the web-wallet React
application. All ten objectives from the phase specification were addressed.

---

## Changes

### 1. Content Security Policy (`web-wallet/index.html`)

- Added `Content-Security-Policy` meta tag:
  - `script-src 'self'` — no inline scripts; Vite bundles everything from
    same origin
  - `style-src 'self' 'unsafe-inline'` — required because React JSX
    `style={{}}` props render as inline style attributes; moving all inline
    styles to CSS classes is a separate refactor
  - `connect-src 'self'` — RPC / metrics / marketplace calls via same-origin
    Vite proxy
  - `frame-ancestors 'none'` — clickjacking protection
  - `base-uri 'self'`, `form-action 'self'` — injection safeguards
  - `upgrade-insecure-requests` — auto-upgrade HTTP sub-resources to HTTPS
- Added `X-Content-Type-Options: nosniff` meta
- Added `referrer: no-referrer` meta

### 2. XSS Prevention

- Audited all components: no `dangerouslySetInnerHTML` usage found; React JSX
  already escapes user-provided strings
- All form inputs are controlled React state; values are never interpolated
  into raw HTML
- Replaced `alert()` / `confirm()` browser dialogs (which can be spoofed or
  blocked by XSS-injected code) with React UI components (see §6)
- Added `spellCheck={false}` and `autoComplete="off"` on address/key inputs to
  prevent browser autocomplete leaking secrets

### 3. Secure Key Storage (`web-wallet/src/lib/secure-storage.ts`)

New `SecureKeyStore` module:
- AES-256-GCM encryption of the full keystore JSON blob
- PBKDF2 key derivation (100 000 iterations, SHA-256) from a user password
- Unique 16-byte salt per browser profile, stored separately in localStorage
- IV is prepended to ciphertext; the combined hex blob is stored under
  `coinject_keys_enc_v1`
- `isSupported()` guard: requires `window.crypto.subtle` (unavailable on plain
  HTTP in non-localhost contexts, enforcing HTTPS as a side-effect)
- API: `save(name, keyPair, password)`, `list(password)`, `delete(name,
  password)`, `hasEncryptedStore()`
- The existing plaintext `KeyStore` is preserved for backward compatibility and
  testnet use

### 4. HTTPS Enforcement

- `upgrade-insecure-requests` CSP directive in `index.html` upgrades any
  accidental HTTP sub-resource requests to HTTPS
- `SecureKeyStore.isSupported()` returns `false` when `SubtleCrypto` is absent
  (only available on HTTPS or localhost), surfacing HTTPS as a requirement for
  password-protected keys
- Disabled production sourcemaps in `vite.config.ts` (`sourcemap: false`) to
  avoid leaking application source code through a CDN or server

### 5. Input Validation

- `WalletView.tsx`: `validateAccountName` (1–40 chars, rejects `<>"'\`` to
  block stored-XSS via account names) and `validatePrivateKeyHex` (exactly 64
  hex characters)
- `TransactionModal.tsx`: `validateAddress` (64-char hex) and `validateAmount`
  (positive integer with configurable minimum) applied in every form's
  `handleSubmit` before the transaction is signed; duplicate application
  removed from individual forms
- `BountySubmissionForm.tsx`: `validatePositiveInt` / `validatePositiveFloat`
  guard all numeric fields; problem-definition parsers validate array contents
  and reject NaN values

### 6. Transaction Confirmation (`web-wallet/src/components/TransactionModal.tsx`)

- Every transaction form now passes a `summary: SummaryRow[]` to
  `FormContainer`
- Clicking "Review & Submit" shows a modal confirmation dialog (not a browser
  `confirm()` dialog) listing:
  - Recipient / counterparty (truncated address)
  - Amount, fee, and total cost for transfers
  - Relevant parameters for other transaction types
- A warning that "this cannot be reversed" is shown in the confirm dialog
- Only the "Confirm & Sign" button in the confirmation dialog triggers
  `sendMutation.mutate()`

### 7. Error Handling — `alert()` replaced with Toast UI

**New file: `web-wallet/src/components/Toast.tsx`**
- `ToastProvider` context wraps the entire app; any component calls
  `useToast().showToast(type, message, detail?)`
- Toast types: `success`, `error`, `warning`, `info`
- Toasts auto-dismiss after 5 s (8 s for errors)
- Manual dismiss button on each toast
- Renders into a fixed overlay, outside the modal stacking context
- ARIA: `role="region" aria-live="polite"` on the container;
  `role="alert"` on each individual toast item

`WalletView.tsx`:
- Faucet success/error → `showToast` instead of `alert()`
- Account deletion confirmation → inline `<ConfirmDialog>` component instead
  of `confirm()`
- Account created/imported → `showToast('success', …)`

`BountySubmissionForm.tsx`:
- Submission success/failure → `showToast` instead of `alert()`
- Private salt shown in an in-page highlighted box with a copy button and
  explicit "I have saved the salt — Close" CTA instead of `alert()`

### 8. Loading States

- `AccountCard` balance: skeleton shimmer while `isLoading`
- `AccountDetails` balance: skeleton shimmer while `infoLoading`; `aria-busy`
  on the balance `<dd>`
- Faucet button: shows `Loader2` spinner icon while `isPending`
- TransactionModal account info: skeleton shimmer while `infoLoading`
- `FormContainer` submit button: shows `Loader2` spinner while `isPending`;
  `aria-busy` attribute set
- Confirmation dialog confirm button: shows spinner while submitting
- `BountySubmissionForm` submit button: shows `Loader2` spinner while
  `submitting`
- Loading spinner keyframe animation (`@keyframes spin`) and `.skeleton`
  shimmer animation added to `index.css`

### 9. Responsive Design (`web-wallet/src/index.css`)

- Media query at `max-width: 720px`:
  - Wallet columns grid (`grid-template-columns: 1fr 1fr`) stacks to single
    column via `.wallet-grid` CSS class
  - Tab navigation becomes a horizontally scrollable row to accommodate all
    four tabs on narrow screens
  - Card padding reduced from 24 px to 16 px on mobile
  - Modal max-width widened to 96% on mobile
- `#root` padding reduced to 12 px on mobile
- `tab-nav` class added to tab navigation container in `App.tsx`
- `wallet-grid` class added to wallet column container in `WalletView.tsx`

### 10. Accessibility

- `index.html`: skip-to-content link (`.skip-link` class, visually hidden
  until focused), `role="main"` on `#root`, `aria-label` on root element
- `App.tsx`:
  - `<header>`, `<main id="main-content">`, `<footer role="contentinfo">` landmark
    elements
  - Tab `<nav aria-label="Main navigation">`; each tab button has
    `role="tab"`, `aria-selected`, `aria-controls`
  - All tab icons have `aria-hidden="true"`
- `WalletView.tsx`:
  - Account list uses `<ul role="list">` with `<li>` items
  - `AccountCard` uses `role="button"`, `tabIndex={0}`, keyboard event handler
    (`Enter`/`Space`), `aria-pressed`
  - All icon buttons have `aria-label`
  - `<dl>/<dt>/<dd>` semantic structure for account details
  - `aria-live="polite"` on balance display; `aria-busy` while loading
  - `role="status"` on success messages; `role="alert"` on error messages
  - All form inputs use `<label htmlFor>` association and `aria-required`
  - Modals: `role="dialog"`, `aria-modal="true"`, `aria-labelledby`, Escape
    key closes
  - `ConfirmDialog` focuses Cancel button on mount
- `TransactionModal.tsx`:
  - `role="dialog"`, `aria-modal`, `aria-labelledby`, Escape closes
  - All form fields use `<label htmlFor>` + unique `fieldId` prop
  - Error banner has `role="alert"` and a dismiss button
  - Loading states use `aria-busy`
  - Success panel uses `role="status" aria-live="polite"`
  - Confirmation dialog: `role="dialog"`, `aria-labelledby`, auto-focuses
    Confirm button
- `BountySubmissionForm.tsx`:
  - `role="dialog"`, `aria-modal`, `aria-labelledby`
  - Privacy toggle uses `role="switch"`, `aria-checked`
  - All inputs use `<label htmlFor>` association
  - Bounty fields wrapped in `<fieldset>/<legend>`
  - Error message `role="alert"`; salt box `role="alert" aria-live="assertive"`
- `index.css`:
  - `.skip-link` utility class for skip-to-content links
  - `*:focus-visible` rule for a consistent keyboard focus ring
  - `button/input/select/textarea/a:focus-visible` ring with
    `box-shadow` enhancement
  - `.confirm-row` / `.confirm-row-label` / `.confirm-row-value` for the
    transaction confirmation summary

---

## Files Changed

| File | Type |
|------|------|
| `web-wallet/index.html` | Modified |
| `web-wallet/vite.config.ts` | Modified |
| `web-wallet/src/index.css` | Modified |
| `web-wallet/src/App.tsx` | Modified |
| `web-wallet/src/components/WalletView.tsx` | Modified |
| `web-wallet/src/components/TransactionModal.tsx` | Modified |
| `web-wallet/src/components/BountySubmissionForm.tsx` | Modified |
| `web-wallet/src/components/Toast.tsx` | **New** |
| `web-wallet/src/lib/secure-storage.ts` | **New** |
