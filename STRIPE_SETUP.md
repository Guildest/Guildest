# Stripe Configuration Guide

## Overview

Guildest has a fully integrated Stripe subscription system for the Pro plan with the following features:

- **Pro plan subscription** with Stripe Checkout
- **Customer portal** for managing subscriptions (cancel, update payment, etc.)
- **Webhook processing** for real-time subscription updates
- **Automatic plan sync** - webhooks update user plans in database
- **Promo code support** during checkout

## Architecture

### Backend (FastAPI) - `backend/api/main.py`
- Stripe checkout session creation (line 305-341)
- Stripe billing portal access (line 344-364)
- Webhook handling for subscription events (line 501-608)
- Customer and subscription management

### Frontend (Next.js)
- Billing dashboard UI (`frontend/src/app/dashboard/billing/page.tsx`)
- Checkout/portal button components
- API integration (`frontend/src/lib/api.ts`)

### Webhook Events Handled
- `checkout.session.completed`
- `customer.subscription.created`
- `customer.subscription.updated`
- `customer.subscription.deleted`

---

## Configuration Steps for Stripe Sandbox (Test Mode)

### 1. Get Your Stripe API Keys

From your Stripe Dashboard (test mode):

1. Go to **Developers → API Keys**
2. Copy your **Secret key** (starts with `sk_test_`)
3. This is your `STRIPE_SECRET_KEY`

### 2. Create a Product and Price

1. Go to **Products** in Stripe Dashboard
2. Click **Add product**
3. Set name: "Guildest Pro" (or similar)
4. Set pricing: Recurring, monthly (or your preferred billing cycle)
5. Set amount (e.g., $9.99/month)
6. Click **Save product**
7. Copy the **Price ID** (starts with `price_`)
8. This is your `STRIPE_PRO_PRICE_ID`

### 3. Set Up Webhook Endpoint

#### Option A: Stripe CLI (Recommended for Local Dev)

```bash
# Install Stripe CLI
# https://stripe.com/docs/stripe-cli

# Login
stripe login

# Forward webhooks to your local API
stripe listen --forward-to http://localhost:8000/webhooks/stripe
```

The CLI will output a webhook signing secret (starts with `whsec_`). This is your `STRIPE_WEBHOOK_SECRET`.

#### Option B: Production/Deployed Environment

1. In Stripe Dashboard, go to **Developers → Webhooks**
2. Click **Add endpoint**
3. Set URL: `https://yourdomain.com/webhooks/stripe` (or `/subscriptions/stripe`)
4. Select events to listen to:
   - `checkout.session.completed`
   - `customer.subscription.created`
   - `customer.subscription.updated`
   - `customer.subscription.deleted`
5. Copy the **Signing secret** (starts with `whsec_`)
6. This is your `STRIPE_WEBHOOK_SECRET`

### 4. Configure Environment Variables

Add these to your `.env` file:

```bash
# Stripe Configuration (Test Mode)
STRIPE_SECRET_KEY=sk_test_your_secret_key_here
STRIPE_WEBHOOK_SECRET=whsec_your_webhook_secret_here
STRIPE_PRO_PRICE_ID=price_your_price_id_here
```

### 5. Restart Your Services

```bash
docker compose down
docker compose up --build
```

---

## Testing the Integration

### Test the Checkout Flow

1. Log in to your dashboard at `http://localhost:3000/dashboard`
2. Go to `/dashboard/billing`
3. Click "Upgrade to Pro" or similar button
4. You'll be redirected to Stripe Checkout
5. Use Stripe test card: `4242 4242 4242 4242`
   - Any future expiry date
   - Any 3-digit CVC
   - Any ZIP code
6. Complete checkout
7. You should be redirected back to `/dashboard/billing?checkout=success`

### Test Webhooks

Watch your API logs - you should see:
```
Received Stripe webhook checkout.session.completed
```

Your user's plan in the database should update from "free" to "pro".

### Test Billing Portal

1. From `/dashboard/billing`, click "Manage Subscription"
2. You'll be redirected to Stripe's customer portal
3. Try canceling or updating the subscription
4. Webhooks should fire and update your database

### Verify Database Updates

Check the `subscriptions` table in your database after webhook events:

```sql
SELECT * FROM subscriptions WHERE user_id = 'your_user_id';
```

You should see:
- `plan = 'pro'` when subscription is active
- `status` matching Stripe subscription status
- `current_period_end` timestamp
- `stripe_subscription_id` and `stripe_price_id`

---

## Implementation Details

### Webhook Security
The implementation properly verifies webhook signatures using `stripe.Webhook.construct_event()` (line 516), which prevents webhook spoofing.

### Customer Mapping
When a user checks out for the first time, a Stripe Customer is created and linked to their `user_id` via the `users` table `stripe_customer_id` column (lines 318-322).

### Subscription Plan Logic
Plan is set to "pro" only if:
- Subscription status is `active` or `trialing`
- The price_id matches your configured `STRIPE_PRO_PRICE_ID`

(See backend/api/main.py:562-564)

### Frontend Flow
1. `createBillingCheckoutUrl()` → Creates Stripe Checkout session
2. Redirects to Stripe hosted checkout page
3. User completes payment
4. Stripe redirects to `success_url`
5. Webhook fires → Backend updates database
6. User has Pro access

---

## Optional: Dev Mode Plan Setter

For testing without Stripe, use the dev endpoint (requires `DEV_ADMIN_TOKEN` in `.env`):

```bash
# Set a user to Pro without Stripe
curl -X POST http://localhost:8000/subscriptions/dev/set-plan \
  -H "Content-Type: application/json" \
  -H "X-Dev-Admin-Token: your_dev_token" \
  -H "Authorization: Bearer your_session_token" \
  -d '{"plan": "pro"}'
```

---

## Stripe Test Cards

- **Success**: `4242 4242 4242 4242`
- **Decline**: `4000 0000 0000 0002`
- **3D Secure Required**: `4000 0025 0000 3155`

Use any future expiry date, any 3-digit CVC, and any ZIP code.

More test cards: https://stripe.com/docs/testing

---

## Troubleshooting

### Webhooks not firing
- Ensure Stripe CLI is running: `stripe listen --forward-to http://localhost:8000/webhooks/stripe`
- Check webhook endpoint is publicly accessible in production
- Verify webhook secret is correct

### Plan not updating after checkout
- Check API logs for webhook errors
- Verify `STRIPE_PRO_PRICE_ID` matches the price from the checkout
- Check database subscriptions table

### Customer not created
- Ensure user is authenticated before checkout
- Check API logs for errors during customer creation
- Verify Stripe API key has proper permissions

### Checkout redirects to wrong URL
- Verify `FRONTEND_BASE_URL` in `.env`
- Check success_url and cancel_url in checkout session creation (lines 332-333)
