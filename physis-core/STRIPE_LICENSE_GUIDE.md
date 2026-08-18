# Selling physis-core Licenses via Stripe

This guide outlines a minimal flow to sell commercial licenses for physis-core using Stripe.

## 1. Set Up Stripe

1. Create a Stripe account at https://stripe.com.
2. In the Dashboard, go to **Developers → API keys** and note your **Publishable key** (`pk_test_...`).
3. Create a **Product** called "physis-core Commercial License".
4. Add a **Price** (e.g., $29/month or $299/year). Record the **Price ID** (`price_1HsL...`).

## 2. Client‑Side Checkout (no server required)

Use Stripe Checkout hosted page. Embed the following HTML (or serve it from a help window inside the studio):

```html
<!DOCTYPE html>
<html><head><title>physis-core License</title>
<script src="https://checkout.stripe.com/v3/"></script></head>
<body>
<h2>Buy a physis-core Commercial License</h2>
<button id="buy-btn"
        data-key="pk_test_YOUR_PUBLISHABLE_KEY"
        data-address="true"
        data-email="true"
        data-name="physis-core License"
        data-description="Commercial license for physis-core semiotic‑grid engine"
        data-amount="2900">Purchase $29.00 / month</button>

<script>
var btn = document.getElementById('buy-btn');
btn.addEventListener('click', () => {
  var stripe = Stripe(btn.dataset.key);
  stripe.redirectToCheckout({
    lineItems: [{price: 'price_1YOUR_PRICE_ID', quantity: 1}],
    mode: 'subscription',          // or 'payment' for one‑time
    successUrl: 'https://example.com/success?session_id={CHECKOUT_SESSION_ID}',
    cancelUrl:  'https://example.com/cancel'
  }).then(r => { /* handle result */ });
});
</script>
</body></html>
```

Replace `data-key` and `price` with your values. After payment, Stripe redirects to your success URL where you can generate and email a license file.

## 3. Backend (optional) – Issue a License Key

If you want to enforce license checks in physis-core:

1. Your server receives the `checkout.session.completed` webhook (or the user is redirected to a success page).
2. Store a license JSON on the customer's side, e.g. `license.json`:

```json
{
  "license_key": "physis-pro-license-1234",
  "expires_at": "2026-09-01T00:00:00Z",
  "max_seats": 5
}
```

3. In the physis-core CLI, load the license and gate features:

```rust
let license: License = serde_json::from_str(&std::fs::read_to_string("license.json")?)?;
if license.is_valid() { /* unlock ONNX, extra packs, etc. */ }
```

## 4. Testing

- Use Stripe test card `4242 4242 4242 4242` with any future expiry and CMM `123`.
- Verify the redirect works and the license JSON is created.
- Run `physis-core classify ...` and confirm licensed features are active.

## 5. Checklist

- [ ] Stripe product & price created.
- [ ] HTML checkout page prepared (or embed in studio).
- [ ] Success URL handles session ID and creates `license.json`.
- [ ] License loading code added to CLI.
- [ ] Test end‑to‑end with Stripe test mode.
- [ ] Document the purchase flow for customers.